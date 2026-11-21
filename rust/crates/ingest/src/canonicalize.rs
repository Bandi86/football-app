use anyhow::{Context, Result};
use chrono::Utc;
use football_domain::{MatchResultSnapshot, MatchSnapshot, MatchStatus};
use sqlx::PgPool;
use uuid::Uuid;

use crate::normalized::{NormalizedDay, NormalizedMatch, NormalizedTeam};

pub struct Canonicalizer;

#[derive(Debug, Clone)]
pub struct MaterializedMatch {
    pub match_id: Uuid,
    /// A materializált meccshez mindkét csapat feloldott (fixture-seed ref
    /// vagy linker Exact) — konstrukció szerint igaz.
    pub identity_exact: bool,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub snapshot: MatchSnapshot,
    pub observation_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct MaterializedDay {
    pub matches: Vec<MaterializedMatch>,
}

/// A forrás-sor melyik kanonikus meccshez kerül (S37b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchLink {
    /// Pontos natural-key találat vagy új meccs: a kanonikus sor írható.
    Exact { id: Uuid, reversed: bool },
    /// Név-variáns alapján linkelt sor: a kanonikus meccs adatai
    /// érintetlenek, csak a forrás-hozzájárulás és az odds kerül rá.
    Variant { id: Uuid, reversed: bool },
}

impl MatchLink {
    fn id(self) -> Uuid {
        match self {
            MatchLink::Exact { id, .. } | MatchLink::Variant { id, .. } => id,
        }
    }

    fn reversed(self) -> bool {
        match self {
            MatchLink::Exact { reversed, .. } | MatchLink::Variant { reversed, .. } => reversed,
        }
    }

    fn writes_canonical(self) -> bool {
        matches!(self, MatchLink::Exact { .. })
    }
}

/// A meccs-identitás keresésének bemenete (natural-key + név-variáns).
struct MatchLookup<'a> {
    competition_id: Uuid,
    season_id: Uuid,
    fixture: &'a NormalizedMatch,
    home: Uuid,
    away: Uuid,
    home_name: &'a str,
    away_name: &'a str,
}

impl Canonicalizer {
    /// clubelo-pillanatkép upsertje `fact.club_elo`-ba (as-of + kötelező
    /// provenance; a klub-entitás kapcsolás a crosswalk későbbi lépése).
    pub async fn materialize_elo(
        pool: &PgPool,
        day: &crate::normalized::ClubEloDay,
    ) -> Result<usize> {
        let mut tx = pool.begin().await?;
        let mut written = 0usize;
        for row in &day.rows {
            sqlx::query(
                "INSERT INTO fact.club_elo
                 (country_code, club_name, as_of, elo, provenance, source, captured_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7)
                 ON CONFLICT (country_code, club_name, as_of) DO UPDATE SET
                   elo = EXCLUDED.elo,
                   provenance = EXCLUDED.provenance,
                   source = EXCLUDED.source,
                   captured_at = EXCLUDED.captured_at",
            )
            .bind(&row.country_code)
            .bind(&row.club_name)
            .bind(day.as_of)
            .bind(row.elo)
            .bind(&row.provenance)
            .bind(&day.source_code)
            .bind(day.captured_at)
            .execute(&mut *tx)
            .await?;
            written += 1;
        }
        tx.commit().await?;
        Ok(written)
    }

    pub async fn materialize(pool: &PgPool, day: &NormalizedDay) -> Result<MaterializedDay> {
        let mut tx = pool.begin().await?;

        // S21: a megfigyelt verseny-cím kanonizálása fix szótárral, hogy a
        // lokalizált/név-változat címek ne hozzanak létre új entitást.
        let competition = crate::competitions::canonicalize(&day.competition);

        // S9: a verseny országa ismeretlen lehet (kupa/nemzetközi) — ilyenkor
        // NULL; csak ismert kódnál hozunk létre ország-sort.
        // S42: a brit hazai nemzeteket a forrás `GB` kóddal, de a hazai névvel
        // adja („Anglia", „Skócia", „Wales"); a kódot a névvel együtt oldjuk
        // fel, hogy ne „Egyesült Királyság" alá kerüljön az angol liga.
        // Ha a forrás csak az ország nevét adja (pl. openfoot: "Iran", üres kóddal),
        // a névből oldjuk fel a kódot, hogy ne NULL országú verseny keletkezzen.
        let mut canonical_country = football_domain::identity::canonical_country_code_with_name(
            &competition.country_code,
            &competition.country_name,
        );
        if canonical_country.is_empty()
            && !competition.country_name.trim().is_empty()
            && let Some((code, _)) = crate::countries::by_english_name(&competition.country_name)
                .or_else(|| crate::countries::by_localized_title(&competition.country_name))
        {
            canonical_country = football_domain::identity::canonical_country_code(code);
        }
        let country_id: Option<Uuid> = if canonical_country.is_empty() {
            None
        } else {
            Some(upsert_country(&mut tx, &canonical_country, &competition.country_name).await?)
        };
        let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
            .bind(&day.source_code)
            .fetch_one(&mut *tx)
            .await
            .with_context(|| format!("ismeretlen forrás: {}", day.source_code))?;
        // S38: a research forrás create-on-miss módban új kanonikus csapatot
        // hozhat létre (név+ország+forrás-ID); ütközésnél (fuzzy jelölt)
        // marad a review. Más forrásokon a szigorú fail-closed marad.
        let identity_mode: String =
            sqlx::query_scalar("SELECT identity_mode FROM ops.source WHERE id=$1")
                .bind(source_id)
                .fetch_one(&mut *tx)
                .await
                .context("forrás identity_mode")?;
        let create_on_miss = identity_mode == "create_on_miss";

        // S81: a verseny-identitás egyetlen find-or-create szabálya a storage
        // helperben él (source-ref → fold alias → természetes kulcs → create);
        // id-hint is csak lookup után számít, vak create nincs. A forrás saját
        // verseny-azonosítóját is átadjuk, hogy az első létrehozáskor
        // source_entity_ref íródjon.
        let competition_resolution =
            football_storage::repositories::competition_identity::find_or_create_competition(
                &mut tx,
                &football_storage::repositories::competition_identity::CompetitionIdentity {
                    id_hint: competition.id,
                    name: &competition.name,
                    country_id,
                    country_code: if canonical_country.is_empty() {
                        None
                    } else {
                        Some(canonical_country.as_str())
                    },
                    tier: Some(competition.tier),
                    priority: Some(competition.priority),
                    source_id: Some(source_id),
                    source_entity_id: competition.source_entity_id.as_deref(),
                },
            )
            .await?;
        let competition_id = competition_resolution.competition_id;

        let season_id = match day.season.id {
            Some(id) => id,
            None => sqlx::query_scalar(
                "SELECT id FROM core.season WHERE competition_id=$1 AND label=$2 LIMIT 1",
            )
            .bind(competition_id)
            .bind(&day.season.label)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_else(Uuid::now_v7),
        };

        sqlx::query(
            "INSERT INTO core.season (id, competition_id, label)
             VALUES ($1,$2,$3)
             ON CONFLICT (id) DO UPDATE SET label = EXCLUDED.label",
        )
        .bind(season_id)
        .bind(competition_id)
        .bind(&day.season.label)
        .execute(&mut *tx)
        .await?;

        let mut team_ids: std::collections::HashMap<String, Uuid> =
            std::collections::HashMap::new();
        for team in &day.teams {
            match team.id {
                // Fixture-út: az id-hint kanonikus; a seed-ref a demóhoz tartozik.
                Some(_) => {
                    let team_country = team_country_id(&mut tx, team, country_id).await?;
                    upsert_team(&mut tx, team, team_country, day.source_code.as_str()).await?;
                    team_ids.insert(
                        team.source_entity_id.clone(),
                        team.id.expect("hint jelen van"),
                    );
                }
                // Élő út: a fail-closed linker oldja fel; nem egzakt → kimarad.
                None => {
                    // Nemzetközi (INT/XX) versenyeknél a csapat országa a kanonikus
                    // névből jön — különben minden nemzeti csapat XX-re esne.
                    let team_code = inferred_team_country_code(&mut tx, team).await?;
                    let country_team = NormalizedTeam {
                        country_code: team_code.clone(),
                        ..team.clone()
                    };
                    let team_country = team_country_id(&mut tx, &country_team, country_id).await?;
                    let ctx = football_domain::identity::IdentityContext {
                        country_code: if team_code.trim().is_empty() {
                            None
                        } else {
                            Some(team_code)
                        },
                        gender: Some(team.gender.clone()),
                        age_group: Some(team.age_group.clone()),
                        branch: Some(team.branch.clone()),
                    };
                    match football_identity::resolve(
                        pool,
                        source_id,
                        &day.source_code,
                        "team",
                        &team.source_entity_id,
                        &team.name,
                        &ctx,
                    )
                    .await
                    {
                        Ok(football_domain::identity::Resolution::Exact { entity_id, .. }) => {
                            ensure_live_team(&mut tx, entity_id, &team.name, team_country).await?;
                            team_ids.insert(team.source_entity_id.clone(), entity_id);
                        }
                        Ok(football_domain::identity::Resolution::NotFound { queue_id })
                            if create_on_miss =>
                        {
                            let created = create_team_on_miss(
                                &mut tx,
                                &day.source_code,
                                &country_team,
                                team_country,
                                queue_id,
                            )
                            .await?;
                            team_ids.insert(team.source_entity_id.clone(), created);
                        }
                        Ok(_) => {
                            tracing::warn!(
                                source = %day.source_code,
                                team = %team.name,
                                "ismeretlen csapat: review-sor nyílt, a meccsek kimaradnak"
                            );
                        }
                        Err(err) => {
                            anyhow::bail!(
                                "identitás-feloldás hiba (fail-closed): {err} — {}",
                                team.name
                            );
                        }
                    }
                }
            }
        }

        let mut matches = Vec::new();
        let team_names: std::collections::HashMap<&str, &str> = day
            .teams
            .iter()
            .map(|t| (t.source_entity_id.as_str(), t.name.as_str()))
            .collect();

        for fixture_match in &day.matches {
            let (Some(&home), Some(&away)) = (
                team_ids.get(&fixture_match.home_source_entity_id),
                team_ids.get(&fixture_match.away_source_entity_id),
            ) else {
                tracing::warn!(
                    source = %day.source_code,
                    home = %fixture_match.home_source_entity_id,
                    away = %fixture_match.away_source_entity_id,
                    "feloldatlan csapat: a meccs kimarad"
                );
                continue;
            };
            if home == away {
                tracing::warn!(
                    source = %day.source_code,
                    home = %fixture_match.home_source_entity_id,
                    away = %fixture_match.away_source_entity_id,
                    "azonos csapat mindkét oldalon: a meccs kimarad"
                );
                continue;
            }

            // `reversed`: a forrás a meccset fordított hazai/vendég oldallal
            // listázza; ilyenkor a meglévő kanonikus sor az igazság, az
            // eredmény/statisztika nem írható, az odds oldalt cserélve megy.
            let link = match fixture_match.id {
                Some(id) => MatchLink::Exact {
                    id,
                    reversed: false,
                },
                None => {
                    find_or_insert_match_id(
                        &mut tx,
                        &MatchLookup {
                            competition_id,
                            season_id,
                            fixture: fixture_match,
                            home,
                            away,
                            home_name: team_names
                                .get(fixture_match.home_source_entity_id.as_str())
                                .copied()
                                .unwrap_or_default(),
                            away_name: team_names
                                .get(fixture_match.away_source_entity_id.as_str())
                                .copied()
                                .unwrap_or_default(),
                        },
                    )
                    .await?
                }
            };
            let match_id = link.id();
            let reversed = link.reversed();
            let status = parse_status(&fixture_match.status)?;

            if reversed {
                tracing::warn!(
                    source = %day.source_code,
                    match_id = %match_id,
                    home = %fixture_match.home_source_entity_id,
                    away = %fixture_match.away_source_entity_id,
                    "fordított oldalú forrás-sor: a kanonikus meccs adatai érintetlenek"
                );
            } else if !link.writes_canonical() {
                tracing::warn!(
                    source = %day.source_code,
                    match_id = %match_id,
                    home = %fixture_match.home_source_entity_id,
                    away = %fixture_match.away_source_entity_id,
                    "név-variáns forrás-sor: a kanonikus meccshez linkelve, az adatai érintetlenek (S37b)"
                );
            } else {
                sqlx::query(
                    "INSERT INTO core.match
                 (id, competition_id, season_id, round_label, home_team_id, away_team_id,
                  kickoff_at, kickoff_precision, competition_timezone, status)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                 ON CONFLICT (id) DO UPDATE SET
                   round_label = EXCLUDED.round_label,
                   home_team_id = EXCLUDED.home_team_id,
                   away_team_id = EXCLUDED.away_team_id,
                   kickoff_at = CASE
                     WHEN EXCLUDED.kickoff_precision = 'exact'
                       OR core.match.kickoff_precision = 'date_only'
                     THEN EXCLUDED.kickoff_at ELSE core.match.kickoff_at END,
                   kickoff_precision = CASE
                     WHEN EXCLUDED.kickoff_precision = 'exact'
                       OR core.match.kickoff_precision = 'exact'
                     THEN 'exact' ELSE 'date_only' END,
                   status = EXCLUDED.status,
                   updated_at = now()",
                )
                .bind(match_id)
                .bind(competition_id)
                .bind(season_id)
                .bind(&fixture_match.round_label)
                .bind(home)
                .bind(away)
                .bind(fixture_match.kickoff_at)
                .bind(&fixture_match.kickoff_precision)
                .bind(&fixture_match.competition_timezone)
                .bind(status_db(status))
                .execute(&mut *tx)
                .await?;
            }

            // S37b: meccs-szintű forrás-híd (eddig csak csapat-szintű volt).
            // Név-variáns linknél alacsonyabb bizalmi szint, hogy a
            // későbbi audit lássa, nem ID-egyezés hozta létre.
            let ref_confidence = if link.writes_canonical() { 1.0 } else { 0.9 };
            sqlx::query(
                "INSERT INTO identity.source_entity_ref
                 (id, source_id, entity_kind, source_entity_id, entity_id, method, confidence)
                 VALUES ($1,$2,'match',$3,$4,'exact_external_id',$5)
                 ON CONFLICT (source_id, entity_kind, source_entity_id)
                 DO UPDATE SET entity_id = EXCLUDED.entity_id,
                               status = 'active',
                               confidence = EXCLUDED.confidence",
            )
            .bind(Uuid::now_v7())
            .bind(source_id)
            .bind(&fixture_match.source_match_id)
            .bind(match_id)
            .bind(ref_confidence)
            .execute(&mut *tx)
            .await?;

            let result = fixture_match.result.as_ref().map(|r| MatchResultSnapshot {
                home_ft: r.home_ft,
                away_ft: r.away_ft,
                home_ht: r.home_ht,
                away_ht: r.away_ht,
                home_et: r.home_et,
                away_et: r.away_et,
                home_pens: r.home_pens,
                away_pens: r.away_pens,
            });

            if !reversed {
                persist_result(
                    &mut tx,
                    match_id,
                    result.as_ref(),
                    fixture_match
                        .result
                        .as_ref()
                        .and_then(|r| r.decided_by.as_deref()),
                )
                .await?;
            } else if result.is_some() {
                tracing::warn!(
                    source = %day.source_code,
                    match_id = %match_id,
                    "fordított oldalú eredmény: nem írjuk (kézi döntés kell)"
                );
            }

            let observation_ids =
                observations_for_match(&mut tx, &day.source_code, &fixture_match.source_match_id)
                    .await?;

            for field in [
                "home_team_id",
                "away_team_id",
                "kickoff_at",
                "status",
                "result",
            ]
            .into_iter()
            .filter(|_| !reversed)
            {
                if let Some(observation_id) = observation_ids.first() {
                    sqlx::query(
                        "INSERT INTO fact.provenance
                         (subject_kind, subject_id, field_key, observation_id,
                          is_selected, policy_version, valid_from)
                         VALUES (
                           'match',$1,$2,$3,
                           NOT EXISTS (
                             SELECT 1 FROM fact.provenance p
                             WHERE p.subject_kind='match' AND p.subject_id=$1
                               AND p.field_key=$2 AND p.is_selected
                           ),
                           'fixture-1.0',$4
                         )
                         ON CONFLICT (subject_kind, subject_id, field_key, observation_id)
                         DO NOTHING",
                    )
                    .bind(match_id)
                    .bind(field)
                    .bind(observation_id)
                    .bind(Utc::now())
                    .execute(&mut *tx)
                    .await?;
                }
            }

            matches.push(MaterializedMatch {
                match_id,
                identity_exact: true,
                home_team_id: home,
                away_team_id: away,
                snapshot: MatchSnapshot {
                    id: match_id,
                    home_team_id: home,
                    away_team_id: away,
                    kickoff_at: fixture_match.kickoff_at,
                    status,
                    result,
                },
                observation_ids,
            });

            // Odds csak a materializált meccshez (a kimaradóknak nincs kanonikus
            // sora → FK-sértés lenne).
            if let Some(odds) = &fixture_match.odds {
                write_odds(
                    &mut tx,
                    match_id,
                    source_id,
                    day.captured_at,
                    odds,
                    reversed,
                )
                .await?;
            }
            if !reversed {
                if let Some(stats) = &fixture_match.stats {
                    write_stats(&mut tx, match_id, source_id, day.captured_at, stats).await?;
                }
                // S82: a research eseményei; fordított oldalú forrás-sornál a
                // kanonikus oldalak nem írhatók át (a stats/eredmény mintájára).
                if !fixture_match.incidents.is_empty() {
                    write_incidents(
                        &mut tx,
                        match_id,
                        source_id,
                        day.captured_at,
                        &fixture_match.incidents,
                    )
                    .await?;
                }
                if let Some(name) = &fixture_match.referee {
                    link_referee(pool, &mut tx, source_id, &day.source_code, match_id, name)
                        .await?;
                }
            }
        }

        if let Some(standings) = &day.standings {
            for standing in standings {
                let Some(&team_id) = team_ids.get(&standing.team_source_entity_id) else {
                    continue;
                };
                sqlx::query(
                    "INSERT INTO core.standing (season_id, team_id, position, source_id, captured_at)
                     VALUES ($1,$2,$3,$4,$5)
                     ON CONFLICT (season_id, team_id) DO UPDATE SET
                       position = EXCLUDED.position,
                       source_id = EXCLUDED.source_id,
                       captured_at = EXCLUDED.captured_at",
                )
                .bind(season_id)
                .bind(team_id)
                .bind(standing.position)
                .bind(source_id)
                .bind(day.captured_at)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(MaterializedDay { matches })
    }
}

async fn upsert_country(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    code: &str,
    name: &str,
) -> Result<Uuid> {
    // S41: a tárolt ország-kód kanonikus alakja (ISO3 → ISO2, brit alrégiók),
    // hogy ne keletkezzen `ESP` sor az `ES` mellett.
    let code = football_domain::identity::canonical_country_code(code);
    let name = name.trim();
    // Ha a forrás a nevet kódként adja (pl. "NED"), ne írjuk felül vele a
    // rendes megjelenített nevet.
    let code_like =
        !name.is_empty() && name.len() <= 3 && name.chars().all(|c| c.is_ascii_uppercase());
    let name = if name.is_empty() || code_like {
        code.as_str()
    } else {
        name
    };
    sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name)
         VALUES ($1,$2,$3)
         ON CONFLICT (code) DO UPDATE SET name = CASE
             WHEN core.country.name IS NULL
               OR core.country.name = ''
               OR core.country.name = core.country.code
             THEN EXCLUDED.name
             ELSE core.country.name
         END
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(&code)
    .bind(name)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn upsert_entity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    kind: &str,
    name: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name)
         VALUES ($1,$2,$3)
         ON CONFLICT (id) DO UPDATE SET canonical_name = EXCLUDED.canonical_name",
    )
    .bind(id)
    .bind(kind)
    .bind(name)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// S38 create-on-miss: a forrás új kanonikus csapatot hoz létre. Csak akkor
/// fut, ha a linker NEM talált jelöltet (`NotFound`); fuzzy jelöltnél a
/// review-sor marad (nincs automatikus összevonás).
async fn create_team_on_miss(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_code: &str,
    team: &NormalizedTeam,
    country_id: Option<Uuid>,
    queue_id: Uuid,
) -> Result<Uuid> {
    let team_id = Uuid::now_v7();
    let normalized = NormalizedTeam {
        id: Some(team_id),
        source_entity_id: team.source_entity_id.clone(),
        name: team.name.clone(),
        country_code: team.country_code.clone(),
        gender: team.gender.clone(),
        age_group: team.age_group.clone(),
        branch: team.branch.clone(),
    };
    upsert_team(tx, &normalized, country_id, source_code).await?;

    // S1.6: az alias országa a kanonikus ISO2-kód (a `country_team` már a
    // kikövetkeztetett országot hordozza), semleges kódnál (XX/INT/UNK/ZZ/üres)
    // NULL — soha nem nyers INT/XX/üres. A `lang` semleges 'und' a hamis 'hu'
    // helyett; a unique kulcsot ez nem zavarja, mert a lang a kulcs része és
    // az INSERT `ON CONFLICT DO NOTHING`.
    sqlx::query(
        "INSERT INTO identity.alias
         (id, entity_id, alias, lang, country_code, gender, age_group, branch,
          status, method)
         VALUES ($1,$2,$3,'und',$4,$5,$6,$7,'reviewed','import')
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(team_id)
    .bind(&team.name)
    .bind(football_domain::identity::alias_country_code(Some(
        team.country_code.as_str(),
    )))
    .bind(&team.gender)
    .bind(&team.age_group)
    .bind(&team.branch)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        "UPDATE identity.review_queue
         SET status='approved', decided_by='create_on_miss', decided_at=now(),
             candidate_entity_ids=$2,
             decision_note='S38: új csapat a forrásból (create-on-miss)'
         WHERE id=$1",
    )
    .bind(queue_id)
    .bind(serde_json::json!([team_id]))
    .execute(&mut **tx)
    .await?;

    tracing::info!(
        source = %source_code,
        team = %team.name,
        %team_id,
        "új kanonikus csapat a forrásból (create-on-miss)"
    );
    Ok(team_id)
}

async fn upsert_team(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team: &NormalizedTeam,
    country_id: Option<Uuid>,
    source_code: &str,
) -> Result<()> {
    let team_id = team.id.unwrap_or_else(Uuid::now_v7);
    upsert_entity(tx, team_id, "team", &team.name).await?;

    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,$4,$5,$6)
         ON CONFLICT (id) DO UPDATE SET
           name = EXCLUDED.name,
           country_id = COALESCE(EXCLUDED.country_id, core.team.country_id),
           gender = EXCLUDED.gender,
           age_group = EXCLUDED.age_group,
           branch = EXCLUDED.branch",
    )
    .bind(team_id)
    .bind(country_id)
    .bind(&team.name)
    .bind(&team.gender)
    .bind(&team.age_group)
    .bind(&team.branch)
    .execute(&mut **tx)
    .await?;

    let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
        .bind(source_code)
        .fetch_one(&mut **tx)
        .await?;

    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'team',$3,$4,'fixture_seed')
         ON CONFLICT (source_id, entity_kind, source_entity_id)
         DO UPDATE SET entity_id = EXCLUDED.entity_id, status = 'active'",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(&team.source_entity_id)
    .bind(team_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn persist_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    result: Option<&MatchResultSnapshot>,
    decided_by: Option<&str>,
) -> Result<()> {
    let Some(result) = result else {
        sqlx::query("DELETE FROM core.match_result WHERE match_id=$1")
            .bind(match_id)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    };

    sqlx::query(
        "INSERT INTO core.match_result
         (match_id, home_ft, away_ft, home_ht, away_ht, home_et, away_et,
          home_pens, away_pens, decided_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         ON CONFLICT (match_id) DO UPDATE SET
           home_ft=EXCLUDED.home_ft, away_ft=EXCLUDED.away_ft,
           home_ht=EXCLUDED.home_ht, away_ht=EXCLUDED.away_ht,
           home_et=EXCLUDED.home_et, away_et=EXCLUDED.away_et,
           home_pens=EXCLUDED.home_pens, away_pens=EXCLUDED.away_pens,
           decided_by=EXCLUDED.decided_by",
    )
    .bind(match_id)
    .bind(result.home_ft)
    .bind(result.away_ft)
    .bind(result.home_ht)
    .bind(result.away_ht)
    .bind(result.home_et)
    .bind(result.away_et)
    .bind(result.home_pens)
    .bind(result.away_pens)
    .bind(decided_by)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn observations_for_match(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_code: &str,
    source_match_id: &str,
) -> Result<Vec<Uuid>> {
    let rows = sqlx::query_scalar::<_, Uuid>(
        "SELECT o.id
         FROM stage.observation o
         JOIN ops.source s ON s.id = o.source_id
         WHERE s.code = $1 AND o.entity_kind = 'match'
           AND o.aspect = 'match_core' AND o.source_record_key = $2
         ORDER BY o.observed_at DESC",
    )
    .bind(source_code)
    .bind(source_match_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows)
}

/// Nemzetközi versenynél a forrás `INT`/`XX` országot ad minden csapatra
/// (pl. World Cup U17). Ilyenkor a kanonikus csapat nevére rákeresünk, és ha
/// pontosan egy, nem-XX országú entitás illeszkedik (azonos nem/kor/ággal),
/// annak kódját használjuk. Így a linker a meglévő nemzeti csapatot találja
/// meg, és nem születik párhuzamos XX-országú entitás. Több találatnál vagy
/// találat hiányában az eredeti (XX) kód marad — nincs találgatás.
async fn inferred_team_country_code(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team: &NormalizedTeam,
) -> Result<String> {
    let canonical = football_domain::identity::canonical_country_code(&team.country_code);
    if !canonical.is_empty() && canonical != "XX" {
        return Ok(canonical);
    }
    // S2.9: a név-egyezés foldolt kulcson megy (ékezet/írásjel/kisbetű
    // független), nem `lower()`-on: "Epinal" így megtalálja az "Épinal" sort.
    let codes: Vec<String> = sqlx::query_scalar(
        "SELECT co.code FROM core.team t
         JOIN core.country co ON co.id=t.country_id
         WHERE identity.fold_name(t.name) = identity.fold_name($1)
           AND t.gender=$2 AND t.age_group=$3 AND t.branch=$4
           AND co.code IS NOT NULL AND co.code <> 'XX'",
    )
    .bind(&team.name)
    .bind(&team.gender)
    .bind(&team.age_group)
    .bind(&team.branch)
    .fetch_all(&mut **tx)
    .await?;
    let unique: std::collections::BTreeSet<String> = codes.into_iter().collect();
    if unique.len() == 1 {
        return Ok(unique.into_iter().next().expect("egyetlen elem"));
    }
    Ok(canonical)
}

/// A csapat országa: a megfigyelt kód, ha van; különben a verseny országa
/// (hazai liga), kupa/nemzetközi esetén NULL.
async fn team_country_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team: &NormalizedTeam,
    competition_country: Option<Uuid>,
) -> Result<Option<Uuid>> {
    if team.country_code.trim().is_empty() {
        return Ok(competition_country);
    }
    // S41: a keresés is a kanonikus kódon történik (ISO3 → ISO2), különben a
    // `ESP` megfigyelés kihagyná a meglévő `ES` sort.
    let code = football_domain::identity::canonical_country_code(&team.country_code);
    // Meglévő országot nem írunk felül a kóddal mint névvel; új kódnál a
    // kód az ideiglenes név (későbbi megfigyelés javíthatja).
    let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM core.country WHERE code=$1")
        .bind(&code)
        .fetch_optional(&mut **tx)
        .await?;
    match existing {
        Some(id) => Ok(Some(id)),
        None => Ok(Some(upsert_country(tx, &code, &code).await?)),
    }
}

/// Élő csapat: a linkeres Exact entitáshoz létezzen core.team sor
/// (az invariáns: core.team.id == entity.id); hiányzó sor a forrás nevével
/// jön létre.
async fn ensure_live_team(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: Uuid,
    name: &str,
    country_id: Option<Uuid>,
) -> Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM core.team WHERE id=$1)")
        .bind(entity_id)
        .fetch_one(&mut **tx)
        .await?;
    if exists {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,'male','senior','senior')",
    )
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Élő meccs natural-key keresése, különben új id. A `round_label` NEM
/// része az azonosságnak (két forrás eltérő forduló-jelöléssel ugyanazt a
/// meccset írja le); megjelenítési adatként frissül.
async fn find_or_insert_match_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    lookup: &MatchLookup<'_>,
) -> Result<MatchLink> {
    let (competition_id, season_id, m) = (lookup.competition_id, lookup.season_id, lookup.fixture);
    for (h, a, reversed) in [
        (lookup.home, lookup.away, false),
        (lookup.away, lookup.home, true),
    ] {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM core.match
             WHERE competition_id=$1 AND season_id=$2
               AND home_team_id=$3 AND away_team_id=$4
               AND (($6 = 'date_only' AND kickoff_at::date = $5::date)
                 OR ($6 = 'exact' AND kickoff_at = $5))
             LIMIT 1",
        )
        .bind(competition_id)
        .bind(season_id)
        .bind(h)
        .bind(a)
        .bind(m.kickoff_at)
        .bind(&m.kickoff_precision)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(id) = existing {
            return Ok(MatchLink::Exact { id, reversed });
        }
    }

    if let Some(link) =
        find_variant_match(tx, competition_id, m, lookup.home_name, lookup.away_name).await?
    {
        return Ok(link);
    }

    Ok(MatchLink::Exact {
        id: Uuid::now_v7(),
        reversed: false,
    })
}

/// Név-variáns meccs-link (S37b): a különböző források ugyanazt a klubot
/// más néven hozzák (`SV Darmstadt 98` / `Darmstadt`, `TNS` / `The New
/// Saints`), ezért a natural-key nem talál. Ilyenkor a verseny + nap
/// kontextusában, csapatnév-hasonlítással keresünk pontosan egy jelöltet;
/// ha több van, a legkorábbihoz linkelünk (a duplikátumok merge-elése
/// külön szelet, S37c), hogy ne keletkezzen harmadik sor ugyanarra a napra.
async fn find_variant_match(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    competition_id: Uuid,
    m: &NormalizedMatch,
    home_name: &str,
    away_name: &str,
) -> Result<Option<MatchLink>> {
    if home_name.trim().is_empty() || away_name.trim().is_empty() {
        return Ok(None);
    }
    let candidates: Vec<(Uuid, chrono::DateTime<Utc>, String, String)> = sqlx::query_as(
        "SELECT m.id, m.kickoff_at, th.name, ta.name
         FROM core.match m
         JOIN core.team th ON th.id = m.home_team_id
         JOIN core.team ta ON ta.id = m.away_team_id
         WHERE m.competition_id = $1
           AND m.kickoff_at BETWEEN $2 - interval '12 hours' AND $2 + interval '12 hours'
           AND m.status <> 'cancelled'
         ORDER BY m.kickoff_at, m.id",
    )
    .bind(competition_id)
    .bind(m.kickoff_at)
    .fetch_all(&mut **tx)
    .await?;

    let mut hits = Vec::new();
    for (id, kickoff_at, home, away) in candidates {
        if football_domain::identity::names_likely_same(home_name, &home)
            && football_domain::identity::names_likely_same(away_name, &away)
        {
            hits.push((kickoff_at, id, false));
        } else if football_domain::identity::names_likely_same(home_name, &away)
            && football_domain::identity::names_likely_same(away_name, &home)
        {
            hits.push((kickoff_at, id, true));
        }
    }
    hits.sort_by_key(|(kickoff_at, id, _)| (*kickoff_at, *id));

    match hits.first() {
        None => Ok(None),
        Some((_, id, reversed)) => {
            if hits.len() > 1 {
                tracing::warn!(
                    candidates = hits.len(),
                    match_id = %id,
                    "több név-variáns jelölt: a legkorábbihoz linkelünk (S37b; merge: S37c)"
                );
            }
            Ok(Some(MatchLink::Variant {
                id: *id,
                reversed: *reversed,
            }))
        }
    }
}

/// Odds-fejléc upsert + a forrás quote-blokkjának teljes cseréje
/// (a friss/új blokk mindig provisional — a gate utáni kiértékelés zárja).
async fn write_odds(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    source_id: Uuid,
    captured_at: chrono::DateTime<Utc>,
    odds: &crate::normalized::NormalizedOdds,
    swap_sides: bool,
) -> Result<()> {
    let market_count = odds
        .market_count
        .with_context(|| format!("hiányzó piacszám az odds mellé: {match_id}"))?;
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,$3,$4)
         ON CONFLICT (match_id, source_id) DO UPDATE SET
           captured_at = EXCLUDED.captured_at,
           market_count = EXCLUDED.market_count,
           trust_state = 'provisional'",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(captured_at)
    .bind(market_count)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM fact.odds_quote WHERE match_id = $1 AND source_id = $2")
        .bind(match_id)
        .bind(source_id)
        .execute(&mut **tx)
        .await?;
    for quote in &odds.quotes {
        // Fordított oldalú forrás-sornál az 1X2 hazai/vendég kimenetele
        // oldalt cserél (a döntetlen marad).
        let outcome = if swap_sides && quote.market == "1x2" {
            match quote.outcome.as_str() {
                "home" => "away".to_string(),
                "away" => "home".to_string(),
                other => other.to_string(),
            }
        } else {
            quote.outcome.clone()
        };
        sqlx::query(
            "INSERT INTO fact.odds_quote
             (match_id, source_id, bookmaker, market, outcome, line, phase, price)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(&quote.bookmaker)
        .bind(&quote.market)
        .bind(&outcome)
        .bind(quote.line)
        .bind(&quote.phase)
        .bind(quote.price)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Meccs-események teljes cseréje per match+source (S82): a forrás
/// sorrendje adja az `incident_no`-t 1..n; a hiányzó oldal NULL, a
/// `score_after` csak akkor megy, ha mindkét érték jelen van.
///
/// A DELETE+INSERT a write_odds mintáját követi: az új payload a forrás
/// pillanatképe, a régi sorok nem maradhatnak meg mellette.
async fn write_incidents(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    source_id: Uuid,
    captured_at: chrono::DateTime<Utc>,
    incidents: &[crate::normalized::NormalizedIncident],
) -> Result<()> {
    sqlx::query("DELETE FROM fact.match_incidents WHERE match_id = $1 AND source_id = $2")
        .bind(match_id)
        .bind(source_id)
        .execute(&mut **tx)
        .await?;
    for (index, incident) in incidents.iter().enumerate() {
        let Ok(incident_no) = i32::try_from(index + 1) else {
            anyhow::bail!("túl sok esemény egy meccshez: {match_id}");
        };
        // Az ismeretlen oldal NULL (nem tippelünk); a DB CHECK csak a két
        // ismert értéket engedi, a NULL-t nem vizsgálja.
        let side = match incident.side.as_deref() {
            Some("home") => Some("home"),
            Some("away") => Some("away"),
            _ => None,
        };
        let (score_after_home, score_after_away) =
            match (incident.score_after_home, incident.score_after_away) {
                (Some(home), Some(away)) => (Some(home), Some(away)),
                _ => (None, None),
            };
        sqlx::query(
            "INSERT INTO fact.match_incidents
             (match_id, source_id, captured_at, incident_no, side, incident_type,
              minute, detail, player_name, related_player_name,
              score_after_home, score_after_away)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(captured_at)
        .bind(incident_no)
        .bind(side)
        .bind(&incident.incident_type)
        .bind(incident.minute)
        .bind(&incident.detail)
        .bind(&incident.player_name)
        .bind(&incident.related_player_name)
        .bind(score_after_home)
        .bind(score_after_away)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Meccs-statisztika upsert (per match+source; hiányzó érték NULL).
///
/// S82 óta a research forrás is ezt használja (a `day.source_code` alapján
/// feloldott `source_id`-vel), a football-data mellett.
async fn write_stats(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    source_id: Uuid,
    captured_at: chrono::DateTime<Utc>,
    stats: &crate::normalized::NormalizedStats,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO fact.match_stats
         (match_id, source_id, captured_at, home_shots, away_shots,
          home_shots_on_target, away_shots_on_target, home_corners, away_corners,
          home_yellow, away_yellow, home_red, away_red, home_fouls, away_fouls,
          home_xg, away_xg)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
         ON CONFLICT (match_id, source_id) DO UPDATE SET
           captured_at = EXCLUDED.captured_at,
           home_shots = EXCLUDED.home_shots, away_shots = EXCLUDED.away_shots,
           home_shots_on_target = EXCLUDED.home_shots_on_target,
           away_shots_on_target = EXCLUDED.away_shots_on_target,
           home_corners = EXCLUDED.home_corners, away_corners = EXCLUDED.away_corners,
           home_yellow = EXCLUDED.home_yellow, away_yellow = EXCLUDED.away_yellow,
           home_red = EXCLUDED.home_red, away_red = EXCLUDED.away_red,
           home_fouls = EXCLUDED.home_fouls, away_fouls = EXCLUDED.away_fouls,
           home_xg = EXCLUDED.home_xg, away_xg = EXCLUDED.away_xg",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(captured_at)
    .bind(stats.home_shots)
    .bind(stats.away_shots)
    .bind(stats.home_shots_on_target)
    .bind(stats.away_shots_on_target)
    .bind(stats.home_corners)
    .bind(stats.away_corners)
    .bind(stats.home_yellow)
    .bind(stats.away_yellow)
    .bind(stats.home_red)
    .bind(stats.away_red)
    .bind(stats.home_fouls)
    .bind(stats.away_fouls)
    .bind(stats.home_xg)
    .bind(stats.away_xg)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Játékvezető kapcsolása a fail-closed linkeren át; nem egzakt → NULL
/// (a linker review-sort nyit; a meccs nem blokkol emiatt).
async fn link_referee(
    pool: &PgPool,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_id: Uuid,
    source_code: &str,
    match_id: Uuid,
    name: &str,
) -> Result<()> {
    // Slug: ismétlődő elválasztók összevonva ("M. Oliver" → "m-oliver").
    let source_entity_id = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let ctx = football_domain::identity::IdentityContext::default();
    match football_identity::resolve(
        pool,
        source_id,
        source_code,
        "referee",
        &source_entity_id,
        name,
        &ctx,
    )
    .await
    {
        Ok(football_domain::identity::Resolution::Exact { entity_id, .. }) => {
            sqlx::query("UPDATE core.match SET referee_entity_id=$2, updated_at=now() WHERE id=$1")
                .bind(match_id)
                .bind(entity_id)
                .execute(&mut **tx)
                .await?;
        }
        Ok(_) => {
            tracing::warn!(referee = %name, "ismeretlen játékvezető: review-sor, referee NULL");
        }
        Err(err) => {
            anyhow::bail!("játékvezető-feloldás hiba (fail-closed): {err} — {name}");
        }
    }
    Ok(())
}

fn parse_status(value: &str) -> Result<MatchStatus> {
    match value {
        "scheduled" => Ok(MatchStatus::Scheduled),
        "confirmed" => Ok(MatchStatus::Confirmed),
        "live" => Ok(MatchStatus::Live),
        "halftime" => Ok(MatchStatus::Halftime),
        "finished" => Ok(MatchStatus::Finished),
        "postponed" => Ok(MatchStatus::Postponed),
        "canceled" => Ok(MatchStatus::Canceled),
        "abandoned" => Ok(MatchStatus::Abandoned),
        "awarded" => Ok(MatchStatus::Awarded),
        other => anyhow::bail!("ismeretlen meccsstátusz: {other}"),
    }
}

fn status_db(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::Scheduled => "scheduled",
        MatchStatus::Confirmed => "confirmed",
        MatchStatus::Live => "live",
        MatchStatus::Halftime => "halftime",
        MatchStatus::Finished => "finished",
        MatchStatus::Postponed => "postponed",
        MatchStatus::Canceled => "canceled",
        MatchStatus::Abandoned => "abandoned",
        MatchStatus::Awarded => "awarded",
    }
}
