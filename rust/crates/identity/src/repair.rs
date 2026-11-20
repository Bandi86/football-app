//! S41 — identitás-javítás: crosswalk-újraépítés és meccs-hozzárendelés
//! javítása a capture-scope-olt `team_identity` megfigyelésekből.
//!
//! Kiinduló hiba: a soccerway forrás csapat-azonosításai (2026-11-16..20
//! batch-approve) részben rossz csapatokra mutatnak — az U17-es válogatott
//! kódok a felnőtt csapatra, illetve ellenfelek felcserélve. A megbízható
//! igazság a capture-höz tartozó `team_identity` observation: ugyanabban a
//! capture-ben a forrás ugyanazt a kódot ugyanazzal a névvel és kontextussal
//! látta, mint a `match_core` payload.
//!
//! Két nyilvános belépés:
//! - [`rebuild_source_refs`]: a forrás összes team-kódjára a legfrissebb
//!   `team_identity` observation alapján újraépíti az aktív crosswalk-refet;
//! - [`repair_soccerway_matches`]: a megadott kickoff-tartomány meccseit a
//!   saját capture-jük team-nevei alapján átállítja (és szükség esetén
//!   duplikátumot olvaszt be, vagy a megjelenítést visszatartja).
//!
//! Minden írás `ops.audit_event`-be kerül (`identity_repair_source_ref`,
//! `identity_repair_match`); `dry_run` módban egyetlen írás sem történik.
//! Nem hívunk automatikus fuzzy entitás-összevonást: a csapatfeloldás
//! fail-closed (egzakt vagy ≥ 0.85 hasonlóság, egyértelmű jelölt), a
//! meccssorok javítása pedig mindig auditált.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, NaiveDate, Utc};
use football_domain::identity::{
    IdentityContext, canonical_country_code, context_matches, fold_name, merge_name_markers,
    names_likely_same_ignoring_age, normalize_country_code, normalize_name, strip_age_markers,
    token_overlap,
};
use football_storage::repositories::dedupe;
use football_storage::repositories::identity as repo;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::IdentityError;

/// A napi feed és a worker gate ugyanezt a zónát használja
/// (`football-worker` `tick.rs` `PUBLISH_TIMEZONE`); a publish_date a
/// helyi naptári nap, ezért a visszatartásnak is ezt kell számolnia.
const PUBLISH_TIMEZONE: &str = "Europe/Budapest";

/// A gate szabálykészletének fallback értéke, ha még egyetlen gate-sor sincs
/// a környezetben (a worker `RULE_SET_VERSION` konstansával egyezik).
const FALLBACK_RULE_SET_VERSION: &str = "verification-1.2";

/// Egy jelentésben legfeljebb ennyi változás-részletet adunk vissza.
const MAX_REPORT_CHANGES: usize = 50;

/// Crosswalk-újraépítés eredménye.
#[derive(Debug, Clone, Default)]
pub struct RebuildReport {
    /// A feldolgozott forrás kódja.
    pub source: String,
    /// Az érintett kódok száma (obs + meglévő team-ref unió).
    pub codes_seen: usize,
    /// A kód aktív refje már a helyes csapatra mutat.
    pub unchanged: usize,
    /// Új vagy átállított ref-írások száma (dry_run: előrejelzés).
    pub relinked: usize,
    /// Újonnan létrehozott csapatok (dry_run: előrejelzés).
    pub teams_created: usize,
    /// Nem oldható fel egyértelműen (nem írtunk).
    pub ambiguous: usize,
    /// A kódra nincs `team_identity` observation (békén hagytuk).
    pub no_obs: usize,
    /// Legfeljebb [`MAX_REPORT_CHANGES`] változás: (kód, régi csapatnév, új
    /// csapatnév). Új csapatnál a régi oldal `None`.
    pub changes: Vec<(String, Option<String>, String)>,
}

/// Meccs-javítás eredménye.
#[derive(Debug, Clone, Default)]
pub struct MatchRepairReport {
    pub matches_scanned: usize,
    pub already_correct: usize,
    pub repaired: usize,
    pub merged: usize,
    /// `withhold_unresolved` + apply esetén visszatartott meccsek száma.
    pub withheld: usize,
    pub unresolved: usize,
    /// (match_id, magyar hibaüzenet) — a javítás folytatódott a többi meccsen.
    pub errors: Vec<(Uuid, String)>,
}

// ---------------------------------------------------------------------------
// Csapatfeloldás (közös a két belépéshez)
// ---------------------------------------------------------------------------

/// A csapatfeloldás kimenete. `Missing` esetén a hívó dönti el, létrehozza-e
/// az új csapatot (dry_run: csak előrejelez).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lookup {
    Found(Uuid),
    Ambiguous,
    Missing,
}

fn db_err(e: anyhow::Error) -> IdentityError {
    IdentityError::Db(format!("{e:#}"))
}

fn sqlx_err(e: sqlx::Error) -> IdentityError {
    IdentityError::Db(e.to_string())
}

/// A `team_identity` payload kontextusa; az üres/whitespace érték hiány.
fn context_from_payload(payload: &serde_json::Value) -> IdentityContext {
    let field = |k: &str| {
        payload
            .get(k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    IdentityContext {
        country_code: field("country_code"),
        gender: field("gender"),
        age_group: field("age_group"),
        branch: field("branch"),
    }
}

/// A payload kontextusát a névből levezethető jelölőkkel egészíti ki; a
/// `branch` hiányát a szerződés szerint `senior`-ra egészíti ki.
fn derived_context(name: &str, payload: &serde_json::Value) -> IdentityContext {
    let base = context_from_payload(payload);
    let (mut ctx, _notes) = merge_name_markers(name, &base);
    if ctx.branch.is_none() {
        ctx.branch = Some("senior".to_string());
    }
    ctx
}

/// Egzakt név + kontextus egyezés, majd országon belüli, magas bizalmú
/// fuzzy (≥ 0.85, egyértelmű legjobb). Nem dönt véletlenül: bizonytalanság
/// esetén `Missing` (a hívó új csapatot hoz létre), több egzakt találatnál
/// `Ambiguous`.
fn lookup_team(name: &str, ctx: &IdentityContext, entities: &[repo::TeamNameRow]) -> Lookup {
    let normalized = normalize_name(name);
    let folded = fold_name(name);

    let name_matches = |e: &repo::TeamNameRow| {
        normalize_name(&e.canonical_name) == normalized
            || fold_name(&e.canonical_name) == folded
            || normalize_name(&e.team_name) == normalized
            || fold_name(&e.team_name) == folded
    };

    let exact: Vec<&repo::TeamNameRow> = entities
        .iter()
        .filter(|e| name_matches(e) && context_matches(ctx, &e.context()))
        .collect();
    match exact.len() {
        1 => return Lookup::Found(exact[0].entity_id),
        n if n > 1 => return Lookup::Ambiguous,
        _ => {}
    }

    // Országon belüli fuzzy: csak kontextus-egyező, azonos országú jelölt
    // jöhet szóba; a pontszám a korosztály-jelölés nélküli token-átfedés.
    let wanted_country = ctx
        .country_code
        .as_deref()
        .map(normalize_country_code)
        .filter(|c| !c.is_empty());
    let wanted_age = age_token(name);
    let mut scored: Vec<(f64, Uuid)> = Vec::new();
    for entity in entities {
        if !context_matches(ctx, &entity.context()) {
            continue;
        }
        // Az U17/U19/U23 jelölés külön csapatot jelent: a név szerinti
        // hasonlóság (a korosztály levágása után) ne mosson össze két
        // korosztályt. Ha mindkét névben van korosztály-jelölő, egyezniük kell.
        let candidate_age =
            age_token(&entity.canonical_name).or_else(|| age_token(&entity.team_name));
        if let (Some(wanted), Some(candidate)) = (wanted_age.as_deref(), candidate_age.as_deref())
            && wanted != candidate
        {
            continue;
        }
        if let Some(wanted) = wanted_country.as_deref() {
            let Some(code) = entity.country_code.as_deref() else {
                continue;
            };
            if normalize_country_code(code) != wanted {
                continue;
            }
        }
        let likely = fold_name(&entity.canonical_name) == folded
            || fold_name(&entity.team_name) == folded
            || names_likely_same_ignoring_age(name, &entity.canonical_name)
            || names_likely_same_ignoring_age(name, &entity.team_name);
        if !likely {
            continue;
        }
        let score =
            fuzzy_score(name, &entity.canonical_name).max(fuzzy_score(name, &entity.team_name));
        if score >= 0.85 {
            scored.push((score, entity.entity_id));
        }
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
    match scored.as_slice() {
        [(_, id)] => Lookup::Found(*id),
        [(best, _), (second, _), ..] if best > second => Lookup::Found(scored[0].1),
        _ => Lookup::Missing,
    }
}

/// A névből kiolvasott korosztály-jelölő (`U17`, `U-17`, `U 17` → `u17`).
/// `None`, ha a név nem tartalmaz korosztályt (felnőtt csapat).
fn age_token(name: &str) -> Option<String> {
    let cleaned = name.to_lowercase().replace('-', " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        if *token == "u" {
            if let Some(next) = tokens.get(index + 1)
                && next.len() == 2
                && next.chars().all(|c| c.is_ascii_digit())
            {
                return Some(format!("u{next}"));
            }
            continue;
        }
        if let Some(rest) = token.strip_prefix('u')
            && rest.len() == 2
            && rest.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("u{rest}"));
        }
    }
    None
}

/// Fuzzy pontszám: a korosztály-/női jelölés nélküli nevek azonossága 1.0,
/// egyébként token-átfedés.
fn fuzzy_score(name: &str, candidate: &str) -> f64 {
    let (a, b) = (strip_age_markers(name), strip_age_markers(candidate));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    token_overlap(&a, &b)
}

/// Új kanonikus csapat (`core.entity` + `core.team`) a levezetett
/// kontextussal. Az országot a payload kódjából kanonizálva keressük/pótoljuk
/// (S41: ISO3 → ISO2, hogy ne keletkezzen párhuzamos országsor); ismeretlen
/// kódnál a kód lesz az ideiglenes név, ahogy az approve-minta is teszi.
async fn create_team(
    pool: &PgPool,
    name: &str,
    ctx: &IdentityContext,
) -> Result<Uuid, IdentityError> {
    let mut tx = pool.begin().await.map_err(sqlx_err)?;
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err)?;

    let country_id: Option<Uuid> = match ctx.country_code.as_deref().map(str::trim) {
        Some(code) if !code.is_empty() => {
            let code = canonical_country_code(code);
            let existing: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM core.country WHERE code=$1")
                    .bind(&code)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(sqlx_err)?;
            Some(match existing {
                Some(id) => id,
                None => sqlx::query_scalar(
                    "INSERT INTO core.country (id, code, name) VALUES ($1,$2,$2) RETURNING id",
                )
                .bind(Uuid::now_v7())
                .bind(&code)
                .fetch_one(&mut *tx)
                .await
                .map_err(sqlx_err)?,
            })
        }
        _ => None,
    };

    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,$4,$5,$6)",
    )
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .bind(ctx.gender.as_deref().unwrap_or("male"))
    .bind(ctx.age_group.as_deref().unwrap_or("senior"))
    .bind(ctx.branch.as_deref().unwrap_or("senior"))
    .execute(&mut *tx)
    .await
    .map_err(sqlx_err)?;

    // Minden írás auditálva: az új csapat létrejötte önálló esemény, mert a
    // hívó (ref-átállítás vagy meccsjavítás) ettől függetlenül is auditál.
    sqlx::query(
        "INSERT INTO ops.audit_event
         (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,'repair','identity_repair_team_create','team',$2,$3)",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .bind(json!({
        "name": name,
        "country_code": ctx.country_code,
        "gender": ctx.gender,
        "age_group": ctx.age_group,
        "branch": ctx.branch,
    }))
    .execute(&mut *tx)
    .await
    .map_err(sqlx_err)?;

    tx.commit().await.map_err(sqlx_err)?;
    Ok(entity_id)
}

/// A frissen létrehozott csapatot a memóriabeli entitáslistára is felveszi,
/// hogy ugyanabban a futásban egy másik kód/meccs már megtalálja.
fn remember_created(
    entities: &mut Vec<repo::TeamNameRow>,
    entity_id: Uuid,
    name: &str,
    ctx: &IdentityContext,
) {
    entities.push(repo::TeamNameRow {
        entity_id,
        canonical_name: name.to_string(),
        team_name: name.to_string(),
        country_code: ctx.country_code.clone(),
        gender: ctx.gender.clone(),
        age_group: ctx.age_group.clone(),
        branch: ctx.branch.clone(),
    });
}

// ---------------------------------------------------------------------------
// 1) Crosswalk-újraépítés
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct LatestTeamObservation {
    code: String,
    name: Option<String>,
    payload: serde_json::Value,
    observation_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct ActiveTeamRef {
    code: String,
    entity_id: Uuid,
}

/// A forrás team-crosswalkjének újraépítése a legfrissebb `team_identity`
/// observation alapján.
///
/// Kód → a legfrissebb observation (`DISTINCT ON source_entity_id`, a
/// legutóbbi `observed_at` nyer). A kontextus a payloadból jön, a hiányzó
/// age_group/gender/branch a névből vezethető le (ugyanaz a helper, mint a
/// review-javításnál), a branch hiánya `senior`.
///
/// Feloldás: egzakt normalizált/foldolt név a kanonikus vagy a `core.team`
/// névre, kontextus-szűrve; pontosan 1 találat → az. 0 találat → országon
/// belüli fuzzy (≥ 0.85, egyértelmű) → az; különben új csapat. Több egzakt
/// találat → nem írunk (ambiguous).
///
/// Ha a kód aktív refje már a helyes csapatra mutat → skip; különben a régi
/// aktív ref superseded, és új aktív `manual_review` ref íródik a capture
/// observation bizonyítékával (`reviewed_by = 'repair'`). A kódra
/// observation nélkül nincs teendő (`no_obs`). `dry_run` esetén egyetlen
/// írás sem történik, a jelentés csak előrejelez.
pub async fn rebuild_source_refs(
    pool: &PgPool,
    source_code: &str,
    dry_run: bool,
) -> Result<RebuildReport, IdentityError> {
    let source_id = repo::source_id_by_code(pool, source_code)
        .await
        .map_err(db_err)?
        .ok_or_else(|| IdentityError::InvalidInput(format!("ismeretlen forrás: {source_code}")))?;

    let observations: Vec<LatestTeamObservation> = sqlx::query_as(
        "SELECT DISTINCT ON (o.normalized_payload->>'source_entity_id')
                o.normalized_payload->>'source_entity_id' AS code,
                o.normalized_payload->>'name' AS name,
                o.normalized_payload AS payload,
                o.id AS observation_id
           FROM stage.observation o
          WHERE o.source_id = $1
            AND o.entity_kind = 'team'
            AND o.aspect = 'team_identity'
            AND o.normalized_payload->>'source_entity_id' IS NOT NULL
          ORDER BY o.normalized_payload->>'source_entity_id',
                   o.observed_at DESC, o.created_at DESC",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await
    .map_err(sqlx_err)?;

    let refs: Vec<ActiveTeamRef> = sqlx::query_as(
        "SELECT source_entity_id AS code, entity_id
           FROM identity.source_entity_ref
          WHERE source_id = $1 AND entity_kind = 'team' AND status = 'active'",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await
    .map_err(sqlx_err)?;

    let mut entities = repo::all_team_name_rows(pool).await.map_err(db_err)?;
    let entity_names: HashMap<Uuid, String> = entities
        .iter()
        .map(|e| (e.entity_id, e.canonical_name.clone()))
        .collect();
    let ref_by_code: HashMap<String, Uuid> =
        refs.into_iter().map(|r| (r.code, r.entity_id)).collect();
    let obs_by_code: HashMap<String, LatestTeamObservation> = observations
        .into_iter()
        .map(|o| (o.code.clone(), o))
        .collect();

    // A kódhalmaz az observationök és a meglévő aktív refek uniója: a
    // ref-only kódokat is látni akarjuk (no_obs), az obs-only kódok új
    // refet kaphatnak.
    let codes: BTreeSet<String> = obs_by_code
        .keys()
        .chain(ref_by_code.keys())
        .cloned()
        .collect();

    let mut report = RebuildReport {
        source: source_code.to_string(),
        ..RebuildReport::default()
    };

    for code in codes {
        report.codes_seen += 1;
        let Some(obs) = obs_by_code.get(&code) else {
            // A kódra nincs team_identity observation: a meglévő refet békén
            // hagyjuk (a capture-scope-olt igazság hiányzik).
            report.no_obs += 1;
            continue;
        };
        let Some(name) = obs.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) else {
            report.ambiguous += 1;
            continue;
        };
        let ctx = derived_context(name, &obs.payload);
        let old_entity = ref_by_code.get(&code).copied();
        let old_name = old_entity.and_then(|id| entity_names.get(&id).cloned());

        let mut created_team = false;
        let target = match lookup_team(name, &ctx, &entities) {
            Lookup::Ambiguous => {
                report.ambiguous += 1;
                continue;
            }
            Lookup::Found(entity_id) => entity_id,
            Lookup::Missing => {
                if dry_run {
                    // Előrejelzés: a csapat létrejönne, a ref átállna.
                    report.teams_created += 1;
                    report.relinked += 1;
                    push_change(&mut report, &code, old_name.as_deref(), name);
                    continue;
                }
                let entity_id = create_team(pool, name, &ctx).await?;
                remember_created(&mut entities, entity_id, name, &ctx);
                report.teams_created += 1;
                created_team = true;
                entity_id
            }
        };

        if old_entity == Some(target) {
            report.unchanged += 1;
            continue;
        }
        report.relinked += 1;
        push_change(&mut report, &code, old_name.as_deref(), name);
        if dry_run {
            continue;
        }
        write_source_ref(
            pool,
            source_id,
            source_code,
            &code,
            target,
            obs.observation_id,
            old_entity,
            old_name.as_deref(),
            name,
            created_team,
        )
        .await?;
    }

    Ok(report)
}

fn push_change(report: &mut RebuildReport, code: &str, old: Option<&str>, new: &str) {
    if report.changes.len() >= MAX_REPORT_CHANGES {
        return;
    }
    report
        .changes
        .push((code.to_string(), old.map(String::from), new.to_string()));
}

/// A ref átállítása egy tranzakcióban: a régi aktív sor superseded, majd az
/// új aktív `manual_review` ref a bizonyíték-observationnel. A
/// `UNIQUE(source_id, entity_kind, source_entity_id)` miatt a superseded sor
/// helyére lép az új (a régi cél az auditban marad meg).
#[allow(clippy::too_many_arguments)]
async fn write_source_ref(
    pool: &PgPool,
    source_id: Uuid,
    source_code: &str,
    code: &str,
    entity_id: Uuid,
    observation_id: Uuid,
    old_entity: Option<Uuid>,
    old_team_name: Option<&str>,
    new_team_name: &str,
    created_team: bool,
) -> Result<(), IdentityError> {
    let mut tx = pool.begin().await.map_err(sqlx_err)?;
    sqlx::query(
        "UPDATE identity.source_entity_ref
            SET status = 'superseded'
          WHERE source_id = $1 AND entity_kind = 'team'
            AND source_entity_id = $2 AND status = 'active'",
    )
    .bind(source_id)
    .bind(code)
    .execute(&mut *tx)
    .await
    .map_err(sqlx_err)?;

    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method,
          confidence, status, reviewed_by, evidence_observation_id)
         VALUES ($1,$2,'team',$3,$4,'manual_review',1.0,'active','repair',$5)
         ON CONFLICT (source_id, entity_kind, source_entity_id) DO UPDATE SET
           entity_id = EXCLUDED.entity_id,
           method = 'manual_review',
           confidence = 1.0,
           status = 'active',
           reviewed_by = 'repair',
           evidence_observation_id = EXCLUDED.evidence_observation_id",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(code)
    .bind(entity_id)
    .bind(observation_id)
    .execute(&mut *tx)
    .await
    .map_err(sqlx_err)?;

    sqlx::query(
        "INSERT INTO ops.audit_event
         (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,'repair','identity_repair_source_ref','source_entity_ref',$2,$3)",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .bind(json!({
        "source": source_code,
        "code": code,
        "old_entity_id": old_entity,
        "old_team_name": old_team_name,
        "new_entity_id": entity_id,
        "new_team_name": new_team_name,
        "observation_id": observation_id,
        "created_team": created_team,
    }))
    .execute(&mut *tx)
    .await
    .map_err(sqlx_err)?;

    tx.commit().await.map_err(sqlx_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 2) Meccs-hozzárendelés javítása
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct SoccerwayMatchRow {
    id: Uuid,
    competition_id: Uuid,
    season_id: Uuid,
    round_label: Option<String>,
    kickoff_at: DateTime<Utc>,
    home_team_id: Uuid,
    away_team_id: Uuid,
    observation_id: Uuid,
    capture_id: Uuid,
    source_home_id: String,
    source_away_id: String,
}

#[derive(Debug, sqlx::FromRow)]
struct TeamIdentityObservation {
    code: String,
    payload: serde_json::Value,
}

/// A kiválasztott soccerway `match_core` provenance-szal rendelkező, a
/// `[from, to)` UTC kickoff-tartományba eső meccsek csapatait a saját
/// capture-jük `team_identity` megfigyelései alapján javítja.
///
/// `dry_run`: csak jelentés, egyetlen írás sem. `withhold_unresolved`:
/// apply módban a feloldhatatlan meccsek `ops.publish_gate` sora
/// `state='withheld'` lesz, `IDENTITY_UNRESOLVED` blokkolóval, a napi feed
/// által használt `Europe/Budapest` helyi publish_date-tel.
pub async fn repair_soccerway_matches(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
    dry_run: bool,
    withhold_unresolved: bool,
) -> Result<MatchRepairReport, IdentityError> {
    if from > to {
        return Err(IdentityError::InvalidInput(
            "A kezdő dátum nem lehet későbbi a záró dátumnál.".to_string(),
        ));
    }
    let from_ts = from
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| IdentityError::InvalidInput("érvénytelen kezdő dátum".to_string()))?
        .and_utc();
    let to_ts = to
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| IdentityError::InvalidInput("érvénytelen záró dátum".to_string()))?
        .and_utc();

    let matches: Vec<SoccerwayMatchRow> = sqlx::query_as(
        "SELECT DISTINCT ON (m.id)
                m.id, m.competition_id, m.season_id, m.round_label,
                m.kickoff_at, m.home_team_id, m.away_team_id,
                o.id AS observation_id, o.capture_id,
                o.normalized_payload->>'source_home_id' AS source_home_id,
                o.normalized_payload->>'source_away_id' AS source_away_id
           FROM core.match m
           JOIN fact.provenance p
             ON p.subject_kind = 'match' AND p.subject_id = m.id AND p.is_selected
           JOIN stage.observation o ON o.id = p.observation_id
           JOIN ops.source s ON s.id = o.source_id
          WHERE s.code = 'soccerway'
            AND o.entity_kind = 'match'
            AND o.aspect = 'match_core'
            AND o.capture_id IS NOT NULL
            AND o.normalized_payload->>'source_home_id' IS NOT NULL
            AND o.normalized_payload->>'source_away_id' IS NOT NULL
            AND m.kickoff_at >= $1
            AND m.kickoff_at < $2
          ORDER BY m.id, o.observed_at DESC",
    )
    .bind(from_ts)
    .bind(to_ts)
    .fetch_all(pool)
    .await
    .map_err(sqlx_err)?;

    let mut entities = repo::all_team_name_rows(pool).await.map_err(db_err)?;
    let mut report = MatchRepairReport::default();

    for row in matches {
        report.matches_scanned += 1;
        match repair_one_match(
            pool,
            &row,
            &mut entities,
            dry_run,
            withhold_unresolved,
            &mut report,
        )
        .await
        {
            Ok(()) => {}
            Err(err) => report.errors.push((row.id, err.to_string())),
        }
    }

    Ok(report)
}

async fn repair_one_match(
    pool: &PgPool,
    row: &SoccerwayMatchRow,
    entities: &mut Vec<repo::TeamNameRow>,
    dry_run: bool,
    withhold_unresolved: bool,
    report: &mut MatchRepairReport,
) -> Result<(), IdentityError> {
    // Oldalanként oldjuk fel a capture-höz tartozó csapatneveket. Ha az egyik
    // oldalhoz nincs observation, a másik oldal attól még javítható: a
    // részleges javítás csak a feloldott oldalt írja át.
    let [home_side, away_side] = resolved_sides(pool, row, entities, dry_run).await?;
    let (Some(home), Some(away)) = (home_side, away_side) else {
        return repair_partial_side(
            pool,
            row,
            home_side,
            away_side,
            dry_run,
            withhold_unresolved,
            report,
        )
        .await;
    };

    // A capture szerinti pár már a helyén van.
    if home == row.home_team_id && away == row.away_team_id {
        report.already_correct += 1;
        return Ok(());
    }

    // A két oldal ugyanaz a csapat: a `core.match` CHECK miatt nem írható.
    if home == away {
        report.unresolved += 1;
        if !dry_run && withhold_unresolved {
            withhold_match(
                pool,
                row,
                "a feloldott csapatpár mindkét oldalán ugyanaz a csapat áll",
            )
            .await?;
            report.withheld += 1;
        }
        return Ok(());
    }

    // Dry-run: a hiányzó csapat miatt nem tudunk id-t összevetni, de a
    // javítás ténye előrejelezhető (új csapat + átállítás).
    if home.is_nil() || away.is_nil() {
        report.repaired += 1;
        return Ok(());
    }

    // Dry-run: innentől már írás következne (UPDATE vagy merge), ezért itt
    // megállunk. Csak előrejelezzük, hogy merge vagy egyszerű átállítás jönne.
    if dry_run {
        if find_conflict(pool, row, home, away).await?.is_some() {
            report.merged += 1;
        } else {
            report.repaired += 1;
        }
        return Ok(());
    }

    // Azonos természetes kulccsal (verseny, szezon, kör, kickoff) létező
    // meccs ugyanezzel a csapatpárral, bármely orientációban → merge.
    if let Some((other_id, other_home, other_away)) = find_conflict(pool, row, home, away).await? {
        let other_is_capture_orientation = other_home == home && other_away == away;
        if other_is_capture_orientation {
            // A helyes orientációjú sor a túlélő; a jelenlegi beleolvad.
            merge_matches(
                pool,
                other_id,
                row.id,
                "S41 identitás-javítás: a capture szerinti orientációjú sor a túlélő",
            )
            .await?;
            write_repair_audit(
                pool,
                row.id,
                row,
                row.home_team_id,
                row.away_team_id,
                home,
                away,
                Some(other_id),
            )
            .await?;
            report.merged += 1;
        } else {
            // A másik sor fordított: a capture szerinti orientáció a jelenlegi
            // soré lesz; előbb beolvasztjuk a fordított duplikátumot.
            merge_matches(
                pool,
                row.id,
                other_id,
                "S41 identitás-javítás: a fordított orientációjú duplikátum beolvasztva",
            )
            .await?;
            report.merged += 1;
            match update_match_teams(pool, row, home, away).await? {
                true => {
                    report.repaired += 1;
                }
                false => {
                    report.errors.push((
                        row.id,
                        "a javítás után is egyedi kulcs ütközést kapunk".to_string(),
                    ));
                }
            }
        }
        return Ok(());
    }

    match update_match_teams(pool, row, home, away).await? {
        true => {
            report.repaired += 1;
        }
        false => {
            // Időközben keletkezett ütközés: a meglévő helyes sorba olvasztunk.
            let Some((other_id, other_home, other_away)) =
                find_conflict(pool, row, home, away).await?
            else {
                report.errors.push((
                    row.id,
                    "a meccs frissítése egyedi kulcsot sértett, de az ütköző meccs nem található"
                        .to_string(),
                ));
                return Ok(());
            };
            if other_home == home && other_away == away {
                merge_matches(
                    pool,
                    other_id,
                    row.id,
                    "S41 identitás-javítás: a capture szerinti orientációjú sor a túlélő",
                )
                .await?;
                write_repair_audit(
                    pool,
                    row.id,
                    row,
                    row.home_team_id,
                    row.away_team_id,
                    home,
                    away,
                    Some(other_id),
                )
                .await?;
                report.merged += 1;
            } else {
                merge_matches(
                    pool,
                    row.id,
                    other_id,
                    "S41 identitás-javítás: a fordított orientációjú duplikátum beolvasztva",
                )
                .await?;
                report.merged += 1;
                if update_match_teams(pool, row, home, away).await? {
                    report.repaired += 1;
                } else {
                    report.errors.push((
                        row.id,
                        "a javítás után is egyedi kulcs ütközést kapunk".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// A meccs capture-jéhez tartozó két team-kód feloldása oldalanként.
/// `None` az adott oldalon: nincs observation vagy a név kétértelmű (ilyenkor
/// nem hozunk létre csapatot). Dry-runban a létrehozandó csapat helyét
/// `Uuid::nil()` jelöli.
async fn resolved_sides(
    pool: &PgPool,
    row: &SoccerwayMatchRow,
    entities: &mut Vec<repo::TeamNameRow>,
    dry_run: bool,
) -> Result<[Option<Uuid>; 2], IdentityError> {
    let observations: Vec<TeamIdentityObservation> = sqlx::query_as(
        "SELECT DISTINCT ON (o.normalized_payload->>'source_entity_id')
                o.normalized_payload->>'source_entity_id' AS code,
                o.normalized_payload AS payload
           FROM stage.observation o
          WHERE o.capture_id = $1
            AND o.entity_kind = 'team'
            AND o.aspect = 'team_identity'
            AND o.normalized_payload->>'source_entity_id' = ANY($2::text[])
          ORDER BY o.normalized_payload->>'source_entity_id', o.observed_at DESC",
    )
    .bind(row.capture_id)
    .bind(vec![row.source_home_id.clone(), row.source_away_id.clone()])
    .fetch_all(pool)
    .await
    .map_err(sqlx_err)?;

    let mut by_code: HashMap<String, (String, IdentityContext)> = HashMap::new();
    for obs in observations {
        let Some(name) = obs
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|n| !n.is_empty())
        else {
            continue;
        };
        let ctx = derived_context(name, &obs.payload);
        by_code.insert(obs.code, (name.to_string(), ctx));
    }

    let mut ids: [Option<Uuid>; 2] = [None, None];
    for (index, code) in [&row.source_home_id, &row.source_away_id]
        .into_iter()
        .enumerate()
    {
        let Some((name, ctx)) = by_code.get(code) else {
            continue;
        };
        ids[index] = match lookup_team(name, ctx, entities) {
            Lookup::Found(entity_id) => Some(entity_id),
            // Kétértelmű névnél nem hozunk létre csapatot (nem keletkezik
            // árva entitás), és az adott oldalt nem javítjuk.
            Lookup::Ambiguous => None,
            Lookup::Missing if dry_run => Some(Uuid::nil()),
            Lookup::Missing => {
                let entity_id = create_team(pool, name, ctx).await?;
                remember_created(entities, entity_id, name, ctx);
                Some(entity_id)
            }
        };
    }
    Ok(ids)
}

/// Részleges javítás: csak a feloldott oldal cserélődik, a másik oldal a
/// jelenlegi meccs csapata marad. Természetes-kulcs ütközés esetén nem
/// merge-elünk (a másik oldal identitása ismeretlen), hanem feloldhatatlannak
/// számítjuk a sort.
async fn repair_partial_side(
    pool: &PgPool,
    row: &SoccerwayMatchRow,
    home_side: Option<Uuid>,
    away_side: Option<Uuid>,
    dry_run: bool,
    withhold_unresolved: bool,
    report: &mut MatchRepairReport,
) -> Result<(), IdentityError> {
    let (home, away) = match (home_side, away_side) {
        (Some(home), None) => (home, row.away_team_id),
        (None, Some(away)) => (row.home_team_id, away),
        _ => {
            report.unresolved += 1;
            if !dry_run && withhold_unresolved {
                withhold_match(
                    pool,
                    row,
                    "a capture-höz tartozó csapatnevek nem oldhatók fel egyértelműen",
                )
                .await?;
                report.withheld += 1;
            }
            return Ok(());
        }
    };

    if home == row.home_team_id && away == row.away_team_id {
        report.already_correct += 1;
        return Ok(());
    }
    if home == away {
        report.unresolved += 1;
        return Ok(());
    }
    // Dry-run vagy létrehozandó csapat: az írás tényét előrejelezzük.
    if dry_run || home.is_nil() || away.is_nil() {
        report.repaired += 1;
        return Ok(());
    }
    if find_conflict(pool, row, home, away).await?.is_some() {
        report.unresolved += 1;
        return Ok(());
    }
    match update_match_teams(pool, row, home, away).await? {
        true => report.repaired += 1,
        false => report.unresolved += 1,
    }
    Ok(())
}

/// Azonos természetes kulcsú, ugyanezt a csapatpárt (bármely orientációban)
/// tartalmazó másik meccs.
async fn find_conflict(
    pool: &PgPool,
    row: &SoccerwayMatchRow,
    home: Uuid,
    away: Uuid,
) -> Result<Option<(Uuid, Uuid, Uuid)>, IdentityError> {
    let conflict: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, home_team_id, away_team_id
           FROM core.match
          WHERE id <> $1
            AND competition_id = $2
            AND season_id = $3
            AND round_label IS NOT DISTINCT FROM $4
            AND kickoff_at = $5
            AND ((home_team_id = $6 AND away_team_id = $7)
              OR (home_team_id = $7 AND away_team_id = $6))
          LIMIT 1",
    )
    .bind(row.id)
    .bind(row.competition_id)
    .bind(row.season_id)
    .bind(&row.round_label)
    .bind(row.kickoff_at)
    .bind(home)
    .bind(away)
    .fetch_optional(pool)
    .await
    .map_err(sqlx_err)?;
    Ok(conflict)
}

/// Meccs-átállítás + audit egy tranzakcióban. `false` → egyedi kulcs
/// ütközés (23505), a tranzakció rollbackelt, a hívó merge-el.
async fn update_match_teams(
    pool: &PgPool,
    row: &SoccerwayMatchRow,
    home: Uuid,
    away: Uuid,
) -> Result<bool, IdentityError> {
    let mut tx = pool.begin().await.map_err(sqlx_err)?;
    let updated = sqlx::query(
        "UPDATE core.match
            SET home_team_id = $2, away_team_id = $3, updated_at = now()
          WHERE id = $1",
    )
    .bind(row.id)
    .bind(home)
    .bind(away)
    .execute(&mut *tx)
    .await;
    match updated {
        Ok(_) => {
            insert_repair_audit(
                &mut tx,
                row.id,
                row,
                row.home_team_id,
                row.away_team_id,
                home,
                away,
                None,
            )
            .await?;
            tx.commit().await.map_err(sqlx_err)?;
            Ok(true)
        }
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
            tx.rollback().await.map_err(sqlx_err)?;
            Ok(false)
        }
        Err(e) => Err(sqlx_err(e)),
    }
}

async fn merge_matches(
    pool: &PgPool,
    survivor: Uuid,
    duplicate: Uuid,
    note: &str,
) -> Result<(), IdentityError> {
    dedupe::merge_duplicate(pool, survivor, duplicate, note)
        .await
        .map_err(db_err)?;
    Ok(())
}

/// Külön audit a merge-ágakhoz (a `merge_duplicate` saját
/// `match_dedupe_merge` eseménye mellé): minden identitás-javítási
/// módosítás `identity_repair_match` eseményt kap.
#[allow(clippy::too_many_arguments)]
async fn write_repair_audit(
    pool: &PgPool,
    match_id: Uuid,
    row: &SoccerwayMatchRow,
    old_home: Uuid,
    old_away: Uuid,
    new_home: Uuid,
    new_away: Uuid,
    merged_into: Option<Uuid>,
) -> Result<(), IdentityError> {
    let mut tx = pool.begin().await.map_err(sqlx_err)?;
    insert_repair_audit(
        &mut tx,
        match_id,
        row,
        old_home,
        old_away,
        new_home,
        new_away,
        merged_into,
    )
    .await?;
    tx.commit().await.map_err(sqlx_err)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_repair_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    row: &SoccerwayMatchRow,
    old_home: Uuid,
    old_away: Uuid,
    new_home: Uuid,
    new_away: Uuid,
    merged_into: Option<Uuid>,
) -> Result<(), IdentityError> {
    sqlx::query(
        "INSERT INTO ops.audit_event
         (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,'repair','identity_repair_match','match',$2,$3)",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(json!({
        "match_id": match_id,
        "old_home": old_home,
        "old_away": old_away,
        "new_home": new_home,
        "new_away": new_away,
        "capture_id": row.capture_id,
        "observation_id": row.observation_id,
        "merged_into": merged_into,
    }))
    .execute(&mut **tx)
    .await
    .map_err(sqlx_err)?;
    Ok(())
}

/// A feloldhatatlan identitású meccs visszatartása: `ops.publish_gate`
/// upsert `withheld` állapotra, a napi feeddel egyező helyi publish_date-tel;
/// a gate-írás és az audit ugyanabban a tranzakcióban történik.
async fn withhold_match(
    pool: &PgPool,
    row: &SoccerwayMatchRow,
    reason: &str,
) -> Result<(), IdentityError> {
    let mut tx = pool.begin().await.map_err(sqlx_err)?;
    let latest: Option<String> = sqlx::query_scalar(
        "SELECT rule_set_version FROM ops.publish_gate
          ORDER BY evaluated_at DESC LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(sqlx_err)?;
    let rule_set_version = latest.unwrap_or_else(|| FALLBACK_RULE_SET_VERSION.to_string());
    let blockers = json!([{
        "rule": "IDENTITY_UNRESOLVED",
        "message": "A meccs csapatidentitása nem oldható fel egyértelműen; a megjelenítés visszatartva.",
    }]);

    let sql = format!(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         SELECT $1, m.id, (m.kickoff_at AT TIME ZONE '{PUBLISH_TIMEZONE}')::date,
                'withheld', $2, $3, now()
           FROM core.match m
          WHERE m.id = $4
         ON CONFLICT (match_id, publish_date) DO UPDATE SET
           state = 'withheld',
           rule_set_version = EXCLUDED.rule_set_version,
           blockers = EXCLUDED.blockers,
           evaluated_at = EXCLUDED.evaluated_at"
    );
    let inserted = sqlx::query(&sql)
        .bind(Uuid::now_v7())
        .bind(&rule_set_version)
        .bind(blockers)
        .bind(row.id)
        .execute(&mut *tx)
        .await
        .map_err(sqlx_err)?;
    if inserted.rows_affected() == 0 {
        tx.rollback().await.map_err(sqlx_err)?;
        return Err(IdentityError::InvalidInput(format!(
            "a visszatartandó meccs nem található: {}",
            row.id
        )));
    }

    sqlx::query(
        "INSERT INTO ops.audit_event
         (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,'repair','identity_repair_match','match',$2,$3)",
    )
    .bind(Uuid::now_v7())
    .bind(row.id)
    .bind(json!({
        "match_id": row.id,
        "state": "withheld",
        "rule": "IDENTITY_UNRESOLVED",
        "reason": reason,
        "capture_id": row.capture_id,
        "observation_id": row.observation_id,
        "rule_set_version": rule_set_version,
    }))
    .execute(&mut *tx)
    .await
    .map_err(sqlx_err)?;

    tx.commit().await.map_err(sqlx_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::identity::IdentityContext;

    fn team_row(name: &str, country: &str, age: &str) -> repo::TeamNameRow {
        repo::TeamNameRow {
            entity_id: Uuid::now_v7(),
            canonical_name: name.to_string(),
            team_name: name.to_string(),
            country_code: Some(country.to_string()),
            gender: Some("male".to_string()),
            age_group: Some(age.to_string()),
            branch: Some(if age == "youth" { "youth" } else { "senior" }.to_string()),
        }
    }

    fn youth_context(country: &str) -> IdentityContext {
        IdentityContext {
            country_code: Some(country.to_string()),
            gender: Some("male".to_string()),
            age_group: Some("youth".to_string()),
            branch: Some("youth".to_string()),
        }
    }

    #[test]
    fn age_token_reads_the_common_forms() {
        assert_eq!(age_token("Iran U17").as_deref(), Some("u17"));
        assert_eq!(age_token("Iran U-19").as_deref(), Some("u19"));
        assert_eq!(age_token("Iran U 23").as_deref(), Some("u23"));
        assert_eq!(age_token("Iran"), None);
    }

    #[test]
    fn lookup_never_merges_different_age_groups() {
        // U17 és U23 azonos országban, azonos kontextussal: a korosztály-
        // jelölő levágása után a nevek azonosak, mégsem ugyanaz a csapat.
        let entities = vec![team_row("Lebanon U23", "LB", "youth")];
        assert_eq!(
            lookup_team("Lebanon U17", &youth_context("LB"), &entities),
            Lookup::Missing
        );
        assert!(matches!(
            lookup_team("Lebanon U23", &youth_context("LB"), &entities),
            Lookup::Found(_)
        ));
    }
}
