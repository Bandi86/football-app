//! S42: verseny-ország javítás (`footballctl competitions repair-countries`).
//!
//! A források néha rossz országú versenyre teszik a meccset (kínai FA Kupa a GB
//! „FA Cup" alatt, chilei/bolíviai Primera División az ES „La Liga" alatt), a
//! brit hazai nemzeteket (Anglia/Skócia/Wales/Észak-Írország) pedig GB kód alatt
//! adják meg. Ez a parancs a **kiválasztott kickoff-provenance** megfigyelésekből
//! szavaz, és két szinten javít:
//!
//! 1. verseny: az országot csak erős dominancia esetén írja át (≥60%, ≥2
//!    szavazat), a nevet a domináns kanonikus névre (≥60%);
//! 2. meccs: ha a meccs saját forrás-országa eltér a versenye országától, a
//!    cél-ország azonos nevű versenyébe (és annak szezonjába) sorolja át.
//!
//! Alapból dry-run: csak riportot ír. `--apply` egyetlen tranzakcióban ír, és
//! minden verseny-ország-, verseny-név- és meccs-változásról `ops.audit_event`
//! sort készít (`competition_country_repair` / actor `cli`).

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use football_domain::identity::{
    canonical_country_code, canonical_country_code_with_name, fold_name,
};
use football_ingest::competitions::{canonicalize, country_specific_name};
use football_ingest::normalized::NormalizedCompetition;
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// A verseny-ország döntéshez legalább ennyi (valós országú) szavazat kell.
const MIN_COUNTRY_VOTES: usize = 2;
/// A domináns ország/név minimális szavazataránya (%).
const MIN_SHARE_PERCENT: usize = 60;
const AUDIT_ACTOR: &str = "cli";
const AUDIT_ACTION: &str = "competition_country_repair";

/// Egy megfigyelés a kiválasztott kickoff-provenance-ból.
#[derive(Debug, Clone, sqlx::FromRow)]
struct VoteRow {
    match_id: Uuid,
    competition_id: Uuid,
    season_id: Uuid,
    season_label: String,
    competition_name: String,
    competition_country_code: Option<String>,
    home_team_id: Uuid,
    away_team_id: Uuid,
    round_label: Option<String>,
    kickoff_at: DateTime<Utc>,
    observed_name: String,
    observed_country_code: String,
    observed_country_name: String,
    source_code: String,
}

/// A megfigyelésből számított kanonikus (ország, név) pár.
#[derive(Debug, Clone)]
struct Observation {
    match_id: Uuid,
    competition_id: Uuid,
    season_id: Uuid,
    season_label: String,
    competition_name: String,
    /// A jelenlegi `core.competition.country_id` kanonikus kódja ("" = nincs).
    current_country: String,
    canonical_country: String,
    canonical_name: String,
    source_code: String,
    home_team_id: Uuid,
    away_team_id: Uuid,
    round_label: Option<String>,
    kickoff_at: DateTime<Utc>,
}

impl Observation {
    fn from_row(row: &VoteRow) -> Self {
        let canonical_country = canonical_country_code_with_name(
            &row.observed_country_code,
            &row.observed_country_name,
        );
        let canonical_name = canonical_competition_name(
            &canonical_country,
            &row.observed_name,
            &row.observed_country_name,
        );
        Observation {
            match_id: row.match_id,
            competition_id: row.competition_id,
            season_id: row.season_id,
            season_label: row.season_label.clone(),
            competition_name: row.competition_name.clone(),
            current_country: canonical_country_code(
                row.competition_country_code.as_deref().unwrap_or(""),
            ),
            canonical_country,
            canonical_name,
            source_code: row.source_code.clone(),
            home_team_id: row.home_team_id,
            away_team_id: row.away_team_id,
            round_label: row.round_label.clone(),
            kickoff_at: row.kickoff_at,
        }
    }
}

/// Megfigyelt verseny-név kanonizálása az országgal együtt.
///
/// Először az ország-specifikus felülírást próbálja (S42), különben a globális
/// versenyszótárt futtatja a megfigyelt országkóddal (így a szótári ország nem
/// írja felül a forrás országát, lásd `competitions::canonicalize`).
fn canonical_competition_name(
    canonical_country: &str,
    observed_name: &str,
    observed_country_name: &str,
) -> String {
    if observed_name.trim().is_empty() {
        return String::new();
    }
    if let Some(name) = country_specific_name(canonical_country, observed_name) {
        return name.to_string();
    }
    let competition = NormalizedCompetition {
        id: None,
        source_entity_id: None,
        name: observed_name.to_string(),
        country_code: canonical_country.to_string(),
        country_name: observed_country_name.to_string(),
        tier: 0,
        priority: 0,
    };
    canonicalize(&competition).name
}

/// Valós ország-e (az üres és az „INT"/„XX" nem hordoz ország-információt).
fn is_real_country(code: &str) -> bool {
    let code = code.trim();
    !code.is_empty() && code != "XX"
}

/// Brit hazai nemzet-e a kód (a GB család bármely tagja)?
fn is_british_home_nation(code: &str) -> bool {
    matches!(
        canonical_country_code(code).as_str(),
        "GB" | "GB-ENG" | "GB-SCT" | "GB-WLS" | "GB-NIR"
    )
}

/// A verseny nevéből egyértelmű hazai nemzet: a walesi/skót/észak-írországi
/// ligák forrásai néha UK-szintű kódot adnak (pl. tipplap „Scotland" a Cymru
/// Premierre). Ilyenkor a név a mérvadó, nem a szavazat.
fn forced_home_nation(competition_name: &str) -> Option<&'static str> {
    let folded = fold_name(competition_name);
    let has = |needle: &str| folded.split(' ').any(|token| token == needle);
    if has("cymru") {
        return Some("GB-WLS");
    }
    if has("nifl") {
        return Some("GB-NIR");
    }
    if has("scottish") || has("swpl") || has("lowland") {
        return Some("GB-SCT");
    }
    None
}

/// Szavazatok rendezett listája (darab szerint csökkenő, majd kulcs szerint).
fn count_votes(items: impl Iterator<Item = String>) -> Vec<(String, usize)> {
    let mut map: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        if item.trim().is_empty() {
            continue;
        }
        *map.entry(item).or_default() += 1;
    }
    let mut votes: Vec<(String, usize)> = map.into_iter().collect();
    votes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    votes
}

/// Domináns szavazat: `(kulcs, darab, összes)`. Döntetlennél a lexikálisan
/// kisebb kulcs nyer, hogy a döntés determinisztikus legyen.
fn dominant_vote(votes: &[(String, usize)]) -> Option<(String, usize, usize)> {
    let total = votes.iter().map(|(_, count)| *count).sum();
    votes
        .iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(key, count)| (key.clone(), *count, total))
}

/// Eléri-e a szavazatarány a küszöböt (legalább [`MIN_SHARE_PERCENT`] %)?
fn share_at_least(count: usize, total: usize) -> bool {
    total > 0 && count * 100 >= total * MIN_SHARE_PERCENT
}

/// Verseny-szintű változás (ország és/vagy név).
#[derive(Debug, Clone)]
pub struct CompetitionChange {
    pub competition_id: Uuid,
    pub old_name: String,
    pub new_name: String,
    pub old_country_code: String,
    pub new_country_code: String,
    pub country_changed: bool,
    pub name_changed: bool,
    /// Hány meccs tartozik jelenleg a versenyhez.
    pub match_count: usize,
    pub country_votes: Vec<(String, usize)>,
    pub name_votes: Vec<(String, usize)>,
    pub source_votes: Vec<(String, usize)>,
}

/// Meccs-átsorolás (verseny + szezon).
#[derive(Debug, Clone)]
pub struct MatchMove {
    pub match_id: Uuid,
    pub old_competition_id: Uuid,
    pub old_competition_name: String,
    pub old_country_code: String,
    pub old_season_id: Uuid,
    pub old_season_label: String,
    pub target_country_code: String,
    pub target_name: String,
    pub own_country_code: String,
    pub own_country_votes: Vec<(String, usize)>,
    pub source_votes: Vec<(String, usize)>,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub round_label: Option<String>,
    pub kickoff_at: DateTime<Utc>,
}

/// A futás terve/riportja (dry-runnál nem íródik semmi).
#[derive(Debug, Default)]
pub struct RepairReport {
    pub observations: usize,
    pub matches: usize,
    pub competitions: usize,
    pub competition_changes: Vec<CompetitionChange>,
    pub match_moves: Vec<MatchMove>,
    /// Kihagyott versenyek indok szerint.
    pub skipped_competitions: BTreeMap<&'static str, usize>,
    /// Kihagyott meccsek indok szerint.
    pub skipped_matches: BTreeMap<&'static str, usize>,
}

impl RepairReport {
    fn country_changes(&self) -> usize {
        self.competition_changes
            .iter()
            .filter(|change| change.country_changed)
            .count()
    }

    fn name_changes(&self) -> usize {
        self.competition_changes
            .iter()
            .filter(|change| change.name_changed)
            .count()
    }
}

/// Dry-run előzetes ellenőrzés: milyen ország-sor és cél-verseny hiányzik még
/// (ezeket az `--apply` hozná létre / kódolná át).
#[derive(Debug, Default)]
struct Preflight {
    country_rows_created: Vec<String>,
    country_rows_renamed: Vec<(String, String)>,
    new_competitions: BTreeMap<(String, String), usize>,
}

/// A teljes terv kiszámítása a megfigyelésekből — tiszta függvény, DB nélkül
/// tesztelhető.
fn plan(rows: &[VoteRow]) -> RepairReport {
    let observations: Vec<Observation> = rows.iter().map(Observation::from_row).collect();
    let mut competitions: BTreeMap<Uuid, Vec<&Observation>> = BTreeMap::new();
    let mut matches: BTreeMap<Uuid, Vec<&Observation>> = BTreeMap::new();
    for observation in &observations {
        competitions
            .entry(observation.competition_id)
            .or_default()
            .push(observation);
        matches
            .entry(observation.match_id)
            .or_default()
            .push(observation);
    }
    let mut report = RepairReport {
        observations: observations.len(),
        matches: matches.len(),
        competitions: competitions.len(),
        ..Default::default()
    };
    // A versenyek 3. lépés utáni országa (a meccs-átsoroláshoz).
    let mut competition_country: BTreeMap<Uuid, String> = BTreeMap::new();

    // 1) Verseny-szintű döntés.
    for (competition_id, list) in &competitions {
        let current_name = list[0].competition_name.clone();
        let current_country = list[0].current_country.clone();
        let country_votes: Vec<(String, usize)> = count_votes(
            list.iter()
                .filter(|observation| is_real_country(&observation.canonical_country))
                .map(|observation| observation.canonical_country.clone()),
        );
        let name_votes: Vec<(String, usize)> = count_votes(
            list.iter()
                .map(|observation| observation.canonical_name.clone()),
        );
        let source_votes: Vec<(String, usize)> = count_votes(
            list.iter()
                .map(|observation| observation.source_code.clone()),
        );

        let country_decision = dominant_vote(&country_votes);
        let forced = forced_home_nation(&current_name);
        let (new_country, country_changed, country_skip) = if let Some(forced) = forced {
            let changed = canonical_country_code(&current_country) != forced;
            (forced.to_string(), changed, None)
        } else {
            match &country_decision {
                None => (
                    current_country.clone(),
                    false,
                    Some("nincs valós ország-szavazat"),
                ),
                Some((country, count, total)) => {
                    if *count < MIN_COUNTRY_VOTES {
                        (
                            current_country.clone(),
                            false,
                            Some("kevés ország-szavazat (<2)"),
                        )
                    } else if !share_at_least(*count, *total) {
                        (
                            current_country.clone(),
                            false,
                            Some("gyenge dominancia (<60%)"),
                        )
                    } else if *country == current_country {
                        (current_country.clone(), false, Some("már helyes ország"))
                    } else {
                        (country.clone(), true, None)
                    }
                }
            }
        };

        let (new_name, name_changed) = match dominant_vote(&name_votes) {
            Some((name, count, total)) if share_at_least(count, total) && name != current_name => {
                (name, true)
            }
            _ => (current_name.clone(), false),
        };

        competition_country.insert(*competition_id, new_country.clone());
        if country_changed || name_changed {
            report.competition_changes.push(CompetitionChange {
                competition_id: *competition_id,
                old_name: current_name,
                new_name,
                old_country_code: current_country,
                new_country_code: new_country,
                country_changed,
                name_changed,
                match_count: list
                    .iter()
                    .map(|observation| observation.match_id)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                country_votes,
                name_votes,
                source_votes,
            });
        } else {
            let reason = country_skip.unwrap_or("már helyes ország");
            *report.skipped_competitions.entry(reason).or_default() += 1;
        }
    }

    // 2) Meccs-szintű átsorolás.
    for (match_id, list) in &matches {
        let competition_id = list[0].competition_id;
        let target_country = competition_country
            .get(&competition_id)
            .cloned()
            .unwrap_or_else(|| list[0].current_country.clone());
        let own_votes: Vec<(String, usize)> = count_votes(
            list.iter()
                .filter(|observation| is_real_country(&observation.canonical_country))
                .map(|observation| observation.canonical_country.clone()),
        );
        let Some((own_country, _, _)) = dominant_vote(&own_votes) else {
            *report
                .skipped_matches
                .entry("nincs valós forrás-ország")
                .or_default() += 1;
            continue;
        };
        if !is_real_country(&target_country) {
            *report
                .skipped_matches
                .entry("a verseny országa ismeretlen")
                .or_default() += 1;
            continue;
        }
        if own_country == target_country {
            *report
                .skipped_matches
                .entry("már a jó versenyben van")
                .or_default() += 1;
            continue;
        }
        // Brit hazai nemzetek között nem sorolunk át meccset: a források
        // rendszeresen összekeverik (pl. walesi liga „Scotland" névvel), a
        // cross-border klubok pedig helyesen a szomszéd ligában játszanak.
        if is_british_home_nation(&own_country) && is_british_home_nation(&target_country) {
            *report
                .skipped_matches
                .entry("brit hazai nemzet-ellentmondás")
                .or_default() += 1;
            continue;
        }
        // A cél-név a saját országú megfigyelések domináns kanonikus neve;
        // ha ilyen nincs, bármelyik megfigyelés neve.
        let own_names: Vec<(String, usize)> = count_votes(
            list.iter()
                .filter(|observation| observation.canonical_country == own_country)
                .map(|observation| observation.canonical_name.clone()),
        );
        let fallback_names: Vec<(String, usize)> = count_votes(
            list.iter()
                .map(|observation| observation.canonical_name.clone()),
        );
        let target_name = dominant_vote(&own_names)
            .or_else(|| dominant_vote(&fallback_names))
            .map(|(name, _, _)| name);
        let Some(target_name) = target_name else {
            *report
                .skipped_matches
                .entry("nincs kanonikus verseny-név")
                .or_default() += 1;
            continue;
        };
        report.match_moves.push(MatchMove {
            match_id: *match_id,
            old_competition_id: competition_id,
            old_competition_name: list[0].competition_name.clone(),
            old_country_code: target_country,
            old_season_id: list[0].season_id,
            old_season_label: list[0].season_label.clone(),
            target_country_code: own_country.clone(),
            target_name,
            own_country_code: own_country,
            own_country_votes: own_votes,
            source_votes: count_votes(
                list.iter()
                    .map(|observation| observation.source_code.clone()),
            ),
            home_team_id: list[0].home_team_id,
            away_team_id: list[0].away_team_id,
            round_label: list[0].round_label.clone(),
            kickoff_at: list[0].kickoff_at,
        });
    }

    report
        .competition_changes
        .sort_by(|a, b| a.old_name.cmp(&b.old_name));
    report.match_moves.sort_by(|a, b| {
        (
            a.target_country_code.as_str(),
            a.target_name.as_str(),
            a.match_id,
        )
            .cmp(&(
                b.target_country_code.as_str(),
                b.target_name.as_str(),
                b.match_id,
            ))
    });
    report
}

/// A megfigyelések betöltése a [from,to] helyi napokra (Europe/Budapest).
async fn load_vote_rows(pool: &PgPool, from: NaiveDate, to: NaiveDate) -> Result<Vec<VoteRow>> {
    // A nap-határok a felhasználó időzónájában értendők; a DST-t a PostgreSQL
    // oldja fel, így nincs szükség extra függőségre.
    let rows = sqlx::query_as::<_, VoteRow>(
        r#"
        SELECT p.subject_id                                    AS match_id,
               m.competition_id                                AS competition_id,
               m.season_id                                     AS season_id,
               s.label                                         AS season_label,
               c.name                                          AS competition_name,
               cc.code                                         AS competition_country_code,
               m.home_team_id                                  AS home_team_id,
               m.away_team_id                                  AS away_team_id,
               m.round_label                                   AS round_label,
               m.kickoff_at                                    AS kickoff_at,
               coalesce(o.normalized_payload->'competition'->>'name', '')          AS observed_name,
               coalesce(o.normalized_payload->'competition'->>'country_code', '')  AS observed_country_code,
               coalesce(o.normalized_payload->'competition'->>'country_name', '')  AS observed_country_name,
               coalesce(src.code, '')                          AS source_code
          FROM fact.provenance p
          JOIN stage.observation o ON o.id = p.observation_id
          JOIN core.match m ON m.id = p.subject_id
          JOIN core.competition c ON c.id = m.competition_id
          JOIN core.season s ON s.id = m.season_id
          LEFT JOIN core.country cc ON cc.id = c.country_id
          LEFT JOIN ops.source src ON src.id = o.source_id
         WHERE p.subject_kind = 'match'
           AND p.field_key = 'kickoff_at'
           AND p.is_selected
           AND m.kickoff_at >= ($1::date::timestamp AT TIME ZONE 'Europe/Budapest')
           AND m.kickoff_at < (($2::date + 1)::timestamp AT TIME ZONE 'Europe/Budapest')
         ORDER BY m.competition_id, p.subject_id
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .context("a kickoff-megfigyelések betöltése")?;
    Ok(rows)
}

/// A parancs belépési pontja: terv + opcionális írás + riport.
pub async fn run_repair_countries(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
    apply: bool,
) -> Result<()> {
    let rows = load_vote_rows(pool, from, to).await?;
    let report = plan(&rows);
    let preflight = preflight(pool, &report).await?;
    let mut apply_warnings = Vec::new();
    if apply {
        let mut tx = pool.begin().await.context("tranzakció indítása")?;
        apply_plan(&mut tx, &report, &mut apply_warnings).await?;
        tx.commit().await.context("tranzakció commit")?;
    }
    print_report(&report, from, to, apply, &preflight, &apply_warnings);
    Ok(())
}

/// A terv végrehajtása egyetlen tranzakción belül.
async fn apply_plan(
    tx: &mut Transaction<'_, Postgres>,
    report: &RepairReport,
    warnings: &mut Vec<String>,
) -> Result<()> {
    for change in &report.competition_changes {
        let new_name = if change.name_changed {
            change.new_name.clone()
        } else {
            change.old_name.clone()
        };
        // Ország nélküli versenyen (country_id IS NULL) a név-változás nem
        // hozhat létre üres kódú ország-sort: ilyenkor csak a nevet írjuk.
        if change.country_changed {
            let (country_id, rename) = resolve_country_id(tx, &change.new_country_code).await?;
            if let Some(rename) = rename {
                sqlx::query(
                    "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
                     VALUES ($1, $2, $3, 'country', $4, $5)",
                )
                .bind(Uuid::now_v7())
                .bind(AUDIT_ACTOR)
                .bind(AUDIT_ACTION)
                .bind(rename.country_id)
                .bind(json!({
                    "country_id": rename.country_id,
                    "old_country_code": rename.old_code,
                    "new_country_code": rename.new_code,
                    "reason": "kanonikus GB-ENG kód a meglévő ENG soron",
                }))
                .execute(&mut **tx)
                .await?;
            }
            sqlx::query("UPDATE core.competition SET country_id = $1, name = $2 WHERE id = $3")
                .bind(country_id)
                .bind(&new_name)
                .bind(change.competition_id)
                .execute(&mut **tx)
                .await
                .context("verseny frissítése")?;
        } else if change.name_changed {
            sqlx::query("UPDATE core.competition SET name = $1 WHERE id = $2")
                .bind(&new_name)
                .bind(change.competition_id)
                .execute(&mut **tx)
                .await
                .context("verseny nevének frissítése")?;
        }
        if change.name_changed {
            sqlx::query(
                "UPDATE core.entity SET canonical_name = $1
                  WHERE id = (SELECT entity_id FROM core.competition WHERE id = $2)",
            )
            .bind(&new_name)
            .bind(change.competition_id)
            .execute(&mut **tx)
            .await
            .context("verseny-entitás név frissítése")?;
        }
        sqlx::query(
            "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
             VALUES ($1, $2, $3, 'competition', $4, $5)",
        )
        .bind(Uuid::now_v7())
        .bind(AUDIT_ACTOR)
        .bind(AUDIT_ACTION)
        .bind(change.competition_id)
        .bind(json!({
            "competition_id": change.competition_id,
            "old_country_code": change.old_country_code,
            "new_country_code": change.new_country_code,
            "old_competition_name": change.old_name,
            "new_competition_name": new_name,
            "country_changed": change.country_changed,
            "name_changed": change.name_changed,
            "source_votes": votes_json(&change.source_votes),
            "country_votes": votes_json(&change.country_votes),
            "name_votes": votes_json(&change.name_votes),
        }))
        .execute(&mut **tx)
        .await?;
    }

    for mv in &report.match_moves {
        let (country_id, _) = resolve_country_id(tx, &mv.target_country_code).await?;
        let (target_competition_id, created) =
            find_or_create_competition(tx, country_id, &mv.target_name).await?;
        let target_season_id =
            find_or_create_season(tx, target_competition_id, &mv.old_season_label).await?;
        if natural_key_taken(tx, target_competition_id, target_season_id, mv).await? {
            warnings.push(format!(
                "meccs kihagyva (a cél-versenyben már létezik ugyanaz a natural key): {} → {} ({})",
                mv.match_id, mv.target_name, mv.target_country_code
            ));
            continue;
        }
        sqlx::query(
            "UPDATE core.match SET competition_id = $1, season_id = $2, updated_at = now()
              WHERE id = $3",
        )
        .bind(target_competition_id)
        .bind(target_season_id)
        .bind(mv.match_id)
        .execute(&mut **tx)
        .await
        .context("meccs átsorolása")?;
        sqlx::query(
            "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
             VALUES ($1, $2, $3, 'match', $4, $5)",
        )
        .bind(Uuid::now_v7())
        .bind(AUDIT_ACTOR)
        .bind(AUDIT_ACTION)
        .bind(mv.match_id)
        .bind(json!({
            "match_id": mv.match_id,
            "old_competition_id": mv.old_competition_id,
            "new_competition_id": target_competition_id,
            "old_competition_name": mv.old_competition_name,
            "new_competition_name": mv.target_name,
            "old_country_code": mv.old_country_code,
            "new_country_code": mv.target_country_code,
            "old_season_id": mv.old_season_id,
            "new_season_id": target_season_id,
            "season_label": mv.old_season_label,
            "source_country_code": mv.own_country_code,
            "source_country_votes": votes_json(&mv.own_country_votes),
            "source_votes": votes_json(&mv.source_votes),
            "competition_created": created,
        }))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn votes_json(votes: &[(String, usize)]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, count) in votes {
        map.insert(key.clone(), json!(count));
    }
    serde_json::Value::Object(map)
}

struct CountryRename {
    country_id: Uuid,
    old_code: String,
    new_code: String,
}

/// Ország-sor find-or-create a kanonikus kódra.
///
/// - pontos kód-egyezés → kész;
/// - `GB-ENG` és csak `ENG` sor van → annak a kódját írjuk át (a név marad);
/// - különben új sor a beépített magyar névtáblából (ismeretlen kód → a kód).
async fn resolve_country_id(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<(Uuid, Option<CountryRename>)> {
    if let Some(id) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM core.country WHERE code = $1")
        .bind(code)
        .fetch_optional(&mut **tx)
        .await?
    {
        return Ok((id, None));
    }
    if code == "GB-ENG"
        && let Some(id) =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM core.country WHERE code = 'ENG'")
                .fetch_optional(&mut **tx)
                .await?
    {
        sqlx::query("UPDATE core.country SET code = 'GB-ENG' WHERE id = $1")
            .bind(id)
            .execute(&mut **tx)
            .await
            .context("az ENG ország-sor átkódolása GB-ENG-re")?;
        return Ok((
            id,
            Some(CountryRename {
                country_id: id,
                old_code: "ENG".into(),
                new_code: "GB-ENG".into(),
            }),
        ));
    }
    let name = hungarian_country_name(code).unwrap_or(code);
    let id = Uuid::now_v7();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name) VALUES ($1, $2, $3)
         ON CONFLICT (code) DO UPDATE SET code = EXCLUDED.code
         RETURNING id",
    )
    .bind(id)
    .bind(code)
    .bind(name)
    .fetch_one(&mut **tx)
    .await
    .with_context(|| format!("ország-sor létrehozása: {code}"))?;
    Ok((id, None))
}

/// Beépített magyar ország-névtábla az új sorokhoz (S42).
fn hungarian_country_name(code: &str) -> Option<&'static str> {
    match code {
        "GB" => Some("Egyesült Királyság"),
        "GB-ENG" => Some("Anglia"),
        "GB-SCT" => Some("Skócia"),
        "GB-WLS" => Some("Wales"),
        "GB-NIR" => Some("Észak-Írország"),
        "CN" => Some("Kína"),
        "SK" => Some("Szlovákia"),
        "CL" => Some("Chile"),
        "BO" => Some("Bolívia"),
        _ => None,
    }
}

/// Cél-verseny find-or-create a közös storage-helperen át (S81).
///
/// A verseny-identitás szabálya (source-ref → fold alias → természetes kulcs →
/// create) a `football_storage::repositories::competition_identity` modulban él,
/// így a javító parancs és az ingest ugyanazt a döntést hozza. Itt nincs forrás-
/// kontextus, ezért source-ref írás nem történik; a meglévő tier/priority
/// értékeket nem írjuk felül.
async fn find_or_create_competition(
    tx: &mut Transaction<'_, Postgres>,
    country_id: Uuid,
    name: &str,
) -> Result<(Uuid, bool)> {
    let resolution =
        football_storage::repositories::competition_identity::find_or_create_competition(
            tx,
            &football_storage::repositories::competition_identity::CompetitionIdentity {
                id_hint: None,
                name,
                country_id: Some(country_id),
                country_code: None,
                tier: None,
                priority: None,
                source_id: None,
                source_entity_id: None,
            },
        )
        .await?;
    Ok((resolution.competition_id, resolution.created))
}

/// Cél-szezon find-or-create ugyanazzal a címkével, mint a meccs jelenlegi
/// szezonja (`UNIQUE (competition_id, label)`).
async fn find_or_create_season(
    tx: &mut Transaction<'_, Postgres>,
    competition_id: Uuid,
    label: &str,
) -> Result<Uuid> {
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM core.season WHERE competition_id = $1 AND label = $2",
    )
    .bind(competition_id)
    .bind(label)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(id);
    }
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(competition_id)
        .bind(label)
        .execute(&mut **tx)
        .await
        .context("szezon létrehozása")?;
    Ok(id)
}

/// Ütközne-e az átsorolás a cél-verseny natural key-ével?
async fn natural_key_taken(
    tx: &mut Transaction<'_, Postgres>,
    competition_id: Uuid,
    season_id: Uuid,
    mv: &MatchMove,
) -> Result<bool> {
    let taken: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM core.match
          WHERE competition_id = $1 AND season_id = $2
            AND round_label IS NOT DISTINCT FROM $3
            AND home_team_id = $4 AND away_team_id = $5
            AND kickoff_at = $6 AND id <> $7",
    )
    .bind(competition_id)
    .bind(season_id)
    .bind(&mv.round_label)
    .bind(mv.home_team_id)
    .bind(mv.away_team_id)
    .bind(mv.kickoff_at)
    .bind(mv.match_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(taken.is_some())
}

/// Read-only előzetes ellenőrzés a tervhez: hiányzó ország-sorok, átkódolandó
/// ország-sor és létrehozandó cél-versenyek (a jelentés döntési támpontja).
async fn preflight(pool: &PgPool, report: &RepairReport) -> Result<Preflight> {
    let mut needed_countries: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for change in &report.competition_changes {
        if change.country_changed && !change.new_country_code.is_empty() {
            needed_countries.insert(change.new_country_code.clone());
        }
    }
    for mv in &report.match_moves {
        needed_countries.insert(mv.target_country_code.clone());
    }
    let mut preflight = Preflight::default();
    for code in &needed_countries {
        let exists: Option<String> =
            sqlx::query_scalar("SELECT code FROM core.country WHERE code = $1")
                .bind(code)
                .fetch_optional(pool)
                .await?;
        if exists.is_some() {
            continue;
        }
        if code == "GB-ENG"
            && sqlx::query_scalar::<_, String>("SELECT code FROM core.country WHERE code = 'ENG'")
                .fetch_optional(pool)
                .await?
                .is_some()
        {
            preflight
                .country_rows_renamed
                .push(("ENG".into(), "GB-ENG".into()));
        } else {
            preflight.country_rows_created.push(code.clone());
        }
    }
    let mut targets: BTreeMap<(String, String), usize> = BTreeMap::new();
    for mv in &report.match_moves {
        *targets
            .entry((mv.target_country_code.clone(), mv.target_name.clone()))
            .or_default() += 1;
    }
    for ((code, name), count) in targets {
        let country_id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM core.country WHERE code = $1")
                .bind(&code)
                .fetch_optional(pool)
                .await?;
        let country_id = match country_id {
            Some(id) => Some(id),
            None if code == "GB-ENG" => {
                sqlx::query_scalar("SELECT id FROM core.country WHERE code = 'ENG'")
                    .fetch_optional(pool)
                    .await?
            }
            None => None,
        };
        let exists = if let Some(country_id) = country_id {
            let names: Vec<String> =
                sqlx::query_scalar("SELECT name FROM core.competition WHERE country_id = $1")
                    .bind(country_id)
                    .fetch_all(pool)
                    .await?;
            let key = fold_name(&name);
            names.iter().any(|candidate| fold_name(candidate) == key)
        } else {
            false
        };
        if !exists {
            preflight.new_competitions.insert((code, name), count);
        }
    }
    Ok(preflight)
}

fn print_report(
    report: &RepairReport,
    from: NaiveDate,
    to: NaiveDate,
    apply: bool,
    preflight: &Preflight,
    warnings: &[String],
) {
    println!(
        "verseny-ország javítás [{from}..{to}]{}{}",
        if apply { "" } else { " (dry-run)" },
        if apply { ", tranzakció lezárva" } else { "" }
    );
    println!(
        "  megfigyelés: {} | meccs: {} | verseny: {}",
        report.observations, report.matches, report.competitions
    );
    println!(
        "  verseny-ország változna: {} | verseny-név változna: {} (ebből ország nélkül: {}) | átsorolódó meccs: {}",
        report.country_changes(),
        report.name_changes(),
        report.name_changes()
            - report
                .competition_changes
                .iter()
                .filter(|change| change.name_changed && change.country_changed)
                .count(),
        report.match_moves.len()
    );

    if !report.competition_changes.is_empty() {
        println!("  érintett versenyek:");
        let mut changes = report.competition_changes.clone();
        changes.sort_by(|a, b| {
            b.country_votes
                .first()
                .map(|(_, count)| *count)
                .unwrap_or(0)
                .cmp(
                    &a.country_votes
                        .first()
                        .map(|(_, count)| *count)
                        .unwrap_or(0),
                )
                .then_with(|| a.old_name.cmp(&b.old_name))
        });
        for change in changes.iter().take(15) {
            let votes = change
                .country_votes
                .first()
                .map(|(_, count)| {
                    let total: usize = change.country_votes.iter().map(|(_, n)| n).sum();
                    format!("{count}/{total} szavazat")
                })
                .unwrap_or_else(|| "nincs ország-szavazat".into());
            let mut line = format!(
                "    - {}: {} → {} ({votes}, {} meccs)",
                change.old_name,
                if change.old_country_code.is_empty() {
                    "(nincs)"
                } else {
                    &change.old_country_code
                },
                change.new_country_code,
                change.match_count
            );
            if change.name_changed {
                line.push_str(&format!(
                    "; név: „{}” → „{}”",
                    change.old_name, change.new_name
                ));
            }
            println!("{line}");
        }
        if changes.len() > 15 {
            println!("    … és további {} verseny", changes.len() - 15);
        }
    }

    if !report.match_moves.is_empty() {
        println!("  átsorolások cél szerint:");
        let mut targets: BTreeMap<(String, String), usize> = BTreeMap::new();
        for mv in &report.match_moves {
            *targets
                .entry((mv.target_country_code.clone(), mv.target_name.clone()))
                .or_default() += 1;
        }
        for ((country, name), count) in targets.iter().take(15) {
            println!("    - {country} / {name}: {count} meccs");
        }
        if targets.len() > 15 {
            println!("    … és további {} cél-verseny", targets.len() - 15);
        }
        println!("  példák:");
        for mv in report.match_moves.iter().take(10) {
            println!(
                "    - {}: {} ({}) → {} ({})",
                mv.match_id,
                mv.old_competition_name,
                if mv.old_country_code.is_empty() {
                    "(nincs)"
                } else {
                    &mv.old_country_code
                },
                mv.target_name,
                mv.target_country_code
            );
        }
        if report.match_moves.len() > 10 {
            println!("    … és további {} meccs", report.match_moves.len() - 10);
        }
    }

    if !preflight.country_rows_created.is_empty()
        || !preflight.country_rows_renamed.is_empty()
        || !preflight.new_competitions.is_empty()
    {
        println!("  előzetes ellenőrzés (mi hiányzik még a DB-ből):");
        for code in &preflight.country_rows_created {
            println!("    új ország-sor jönne létre: {code}");
        }
        for (old, new) in &preflight.country_rows_renamed {
            println!("    ország-sor átkódolása: {old} → {new} (a név marad)");
        }
        for ((code, name), count) in &preflight.new_competitions {
            println!("    új cél-verseny jönne létre: {code} / {name} ({count} meccs)");
        }
    }

    if report.skipped_competitions.is_empty() && report.skipped_matches.is_empty() {
        println!("  kihagyottak: nincs");
    } else {
        println!("  kihagyottak:");
        for (reason, count) in &report.skipped_competitions {
            println!("    verseny: {reason}: {count}");
        }
        for (reason, count) in &report.skipped_matches {
            println!("    meccs: {reason}: {count}");
        }
    }
    for warning in warnings {
        println!("  FIGYELEM: {warning}");
    }
    if !apply {
        println!("  (nem történt írás; futtasd --apply kapcsolóval)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vote(key: &str, count: usize) -> (String, usize) {
        (key.to_string(), count)
    }

    /// Megfigyelt verseny-meta a teszt-sorokhoz.
    struct Obs<'a> {
        name: &'a str,
        country_code: &'a str,
        country_name: &'a str,
    }

    fn row(
        match_id: Uuid,
        competition_id: Uuid,
        competition: (&str, Option<&str>),
        observed: Obs<'_>,
        source_code: &str,
    ) -> VoteRow {
        VoteRow {
            match_id,
            competition_id,
            season_id: Uuid::nil(),
            season_label: "2026/2027".into(),
            competition_name: competition.0.into(),
            competition_country_code: competition.1.map(str::to_string),
            home_team_id: Uuid::now_v7(),
            away_team_id: Uuid::now_v7(),
            round_label: None,
            kickoff_at: Utc::now(),
            observed_name: observed.name.into(),
            observed_country_code: observed.country_code.into(),
            observed_country_name: observed.country_name.into(),
            source_code: source_code.into(),
        }
    }

    #[test]
    fn dominant_vote_prefers_count_then_smallest_key() {
        let votes = vec![vote("GB", 3), vote("CN", 2)];
        assert_eq!(
            dominant_vote(&votes),
            Some(("GB".to_string(), 3, 5)),
            "a nagyobb darabszám nyer"
        );
        let tie = vec![vote("SK", 2), vote("CN", 2), vote("BO", 1)];
        assert_eq!(
            dominant_vote(&tie),
            Some(("CN".to_string(), 2, 5)),
            "döntetlennél a lexikálisan kisebb kulcs nyer"
        );
        assert_eq!(dominant_vote(&[]), None);
    }

    #[test]
    fn share_threshold_is_inclusive_sixty_percent() {
        assert!(share_at_least(3, 5));
        assert!(share_at_least(6, 10));
        assert!(!share_at_least(2, 5)); // 40%
        assert!(!share_at_least(0, 0));
    }

    #[test]
    fn competition_country_and_match_move_from_british_home_nations() {
        // Premier League (GB) alatt két angol és egy kínai megfigyelés: a verseny
        // országa GB-ENG lesz, a kínai meccs a kínai FA Kupába sorolódik.
        let competition_id = Uuid::now_v7();
        let eng = Uuid::now_v7();
        let cn = Uuid::now_v7();
        let rows = vec![
            row(
                eng,
                competition_id,
                ("Premier League", Some("GB")),
                Obs {
                    name: "Premier League",
                    country_code: "GB",
                    country_name: "England",
                },
                "soccerway",
            ),
            row(
                Uuid::now_v7(),
                competition_id,
                ("Premier League", Some("GB")),
                Obs {
                    name: "Premier League",
                    country_code: "GB",
                    country_name: "England",
                },
                "research",
            ),
            row(
                cn,
                competition_id,
                ("FA Cup", Some("GB")),
                Obs {
                    name: "FA Cup",
                    country_code: "CN",
                    country_name: "China",
                },
                "soccerway",
            ),
        ];
        let report = plan(&rows);
        assert_eq!(report.competition_changes.len(), 1);
        let change = &report.competition_changes[0];
        assert!(change.country_changed);
        assert_eq!(change.new_country_code, "GB-ENG");
        assert!(!change.name_changed, "a domináns név a jelenlegi név");
        assert_eq!(report.match_moves.len(), 1, "csak a kínai meccs mozdul");
        let mv = &report.match_moves[0];
        assert_eq!(mv.match_id, cn);
        assert_eq!(mv.own_country_code, "CN");
        assert_eq!(mv.target_country_code, "CN");
        assert_eq!(mv.target_name, "Chinese FA Cup");
        assert_eq!(mv.old_competition_id, competition_id);
        // Az angol meccsek a helyükön maradnak.
        assert!(!report.match_moves.iter().any(|mv| mv.match_id == eng));
    }

    #[test]
    fn international_competition_is_skipped() {
        let rows = vec![row(
            Uuid::now_v7(),
            Uuid::now_v7(),
            ("Friendly International", Some("INT")),
            Obs {
                name: "Friendly International",
                country_code: "",
                country_name: "",
            },
            "soccerway",
        )];
        let report = plan(&rows);
        assert!(report.competition_changes.is_empty());
        assert!(report.match_moves.is_empty());
        assert_eq!(
            report
                .skipped_competitions
                .get("nincs valós ország-szavazat"),
            Some(&1)
        );
        assert_eq!(
            report.skipped_matches.get("nincs valós forrás-ország"),
            Some(&1)
        );
    }

    #[test]
    fn single_observation_does_not_change_country_but_renames_competition() {
        // Egyetlen chilei megfigyelés az ES La Liga alatt: a verseny országa nem
        // változik (kevés szavazat), de a kanonikus név 100%-os és eltér, ezért
        // a név frissül; a meccs a chilei Primera Divisiónba sorolódik.
        let competition_id = Uuid::now_v7();
        let match_id = Uuid::now_v7();
        let rows = vec![row(
            match_id,
            competition_id,
            ("La Liga", Some("ES")),
            Obs {
                name: "Primera Division",
                country_code: "CHI",
                country_name: "Chile",
            },
            "soccerway",
        )];
        let report = plan(&rows);
        let change = &report.competition_changes[0];
        assert!(!change.country_changed);
        assert!(change.name_changed);
        assert_eq!(change.new_name, "Primera División");
        assert_eq!(change.new_country_code, "ES");
        assert_eq!(report.match_moves.len(), 1);
        assert_eq!(report.match_moves[0].target_country_code, "CL");
        assert_eq!(report.match_moves[0].target_name, "Primera División");
    }

    #[test]
    fn already_correct_competition_is_not_reported() {
        let competition_id = Uuid::now_v7();
        let rows = vec![
            row(
                Uuid::now_v7(),
                competition_id,
                ("La Liga", Some("ES")),
                Obs {
                    name: "La Liga",
                    country_code: "ES",
                    country_name: "Spain",
                },
                "soccerway",
            ),
            row(
                Uuid::now_v7(),
                competition_id,
                ("La Liga", Some("ES")),
                Obs {
                    name: "La Liga",
                    country_code: "ES",
                    country_name: "Spain",
                },
                "research",
            ),
        ];
        let report = plan(&rows);
        assert!(report.competition_changes.is_empty());
        assert!(report.match_moves.is_empty());
        assert_eq!(
            report.skipped_competitions.get("már helyes ország"),
            Some(&1)
        );
        assert_eq!(
            report.skipped_matches.get("már a jó versenyben van"),
            Some(&2)
        );
    }
}
