//! S2b-1 identitás-réteg storage repository-k.
//!
//! A fail-closed linker és a review-életciklus adatbázis-műveletei: forrás- és
//! ref-feloldás, crosswalk (mindkét irány), reviewed aliasok, csapat-entitások,
//! review-sor upsert/döntés, kézi ref, audit. A három író függvény
//! (`decide_review`, `insert_manual_ref`, `record_identity_decision`)
//! Executor-generikus, hogy a review-életciklus tranzakcióban is hívja őket
//! (Task 5) — SQL-duplikáció nélkül.
//!
//! Dokumentált korlát: az `all_team_entities` teljes szkennelést ad a fuzzy
//! jelölt-képzéshez; S2b-1 szintetikus skálán elfogadható, S3-ban szűkítendő.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_domain::identity::IdentityContext;
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

/// A crosswalk másik oldala egy aktív összerendelésben. A `right_*` mezők
/// iránytól függően a bal vagy a jobb oldalt jelölik (lásd `active_crosswalk`).
#[derive(Debug, Clone)]
pub struct CrosswalkRow {
    pub right_source: String,
    pub right_external_id: String,
    pub rung: String,
}

/// Egy alias sor a kontextusmezőivel; a `context()` a domain kontextus-típusát
/// adja a fail-closed kontextus-szűrőhöz.
#[derive(Debug, Clone)]
pub struct AliasRow {
    pub entity_id: Uuid,
    pub alias: String,
    pub lang: String,
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
    pub status: String,
}

impl AliasRow {
    pub fn context(&self) -> IdentityContext {
        IdentityContext {
            country_code: self.country_code.clone(),
            gender: self.gender.clone(),
            age_group: self.age_group.clone(),
            branch: self.branch.clone(),
        }
    }
}

/// Egy csapat-entitás a kanonikus nevével és kontextusával (core.team +
/// core.country). A `context()` a domain kontextus-típusát adja.
#[derive(Debug, Clone)]
pub struct EntityRow {
    pub entity_id: Uuid,
    pub canonical_name: String,
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
}

impl EntityRow {
    pub fn context(&self) -> IdentityContext {
        IdentityContext {
            country_code: self.country_code.clone(),
            gender: self.gender.clone(),
            age_group: self.age_group.clone(),
            branch: self.branch.clone(),
        }
    }
}

/// Új pending review-sor adatai. A `source_id` lehet NULL (forrás nélküli
/// párosítás); a részleges unique index NULLS NOT DISTINCT kezeli.
#[derive(Debug, Clone)]
pub struct NewReview {
    pub source_id: Option<Uuid>,
    pub entity_kind: String,
    pub source_entity_id: String,
    pub candidate_entity_ids: serde_json::Value,
    pub reason: String,
    pub proposed_name: String,
    pub context: serde_json::Value,
}

/// Egy review-sor olvasási nézete.
#[derive(Debug, Clone)]
pub struct ReviewRow {
    pub id: Uuid,
    pub source_id: Option<Uuid>,
    pub entity_kind: String,
    pub source_entity_id: String,
    pub candidate_entity_ids: serde_json::Value,
    pub reason: String,
    pub proposed_name: String,
    pub context: serde_json::Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Forrásazonosító a hiteles forrás-kód alapján (megszemélyesítés-védelem:
/// a hívó a payload kódját NEM használhatja forrásazonosításra).
pub async fn source_id_by_code(pool: &PgPool, code: &str) -> Result<Option<Uuid>> {
    sqlx::query_scalar("SELECT id FROM ops.source WHERE code = $1")
        .bind(code)
        .fetch_optional(pool)
        .await
        .context("forrás azonosító lekérés")
}

/// Összes forrás (id, kód) párokban, kód szerint rendezve — a review-lista
/// forrás-megjelenítéséhez (a CLI itt kapja a hiteles kódokat).
pub async fn list_sources(pool: &PgPool) -> Result<Vec<(Uuid, String)>> {
    sqlx::query_as("SELECT id, code FROM ops.source ORDER BY code")
        .fetch_all(pool)
        .await
        .context("forrás lista")
}

/// Aktív source→entity ref (entity_id, method). Csak `status='active'` sor;
/// superseded/rejected ref nem old fel.
pub async fn active_ref(
    pool: &PgPool,
    source_id: Uuid,
    entity_kind: &str,
    source_entity_id: &str,
) -> Result<Option<(Uuid, String)>> {
    sqlx::query_as(
        // S2: az összevont entitás soha nem oldódhat fel. A merge a refeket
        // átirányítja, de a védelem itt is kell (kézi/újraírt ref, crosswalk
        // lánc ugyanezen a függvényen megy át).
        "SELECT r.entity_id, r.method FROM identity.source_entity_ref r
         JOIN core.entity e ON e.id = r.entity_id AND e.merged_into_id IS NULL
         WHERE r.source_id = $1 AND r.entity_kind = $2 AND r.source_entity_id = $3
           AND r.status = 'active'",
    )
    .bind(source_id)
    .bind(entity_kind)
    .bind(source_entity_id)
    .fetch_optional(pool)
    .await
    .context("aktív ref lekérés")
}

/// Aktív crosswalk-összerendelések mindkét irányban (left→right ÉS right→left).
/// A visszaadott `right_*` mezők a másik oldalt jelölik: forward irányban a
/// jobb oldalt, reverse irányban a bal oldalt. Csak `status='active'`.
pub async fn active_crosswalk(
    pool: &PgPool,
    source_slug: &str,
    external_id: &str,
) -> Result<Vec<CrosswalkRow>> {
    let rows = sqlx::query(
        "SELECT right_source, right_external_id, rung FROM identity.crosswalk
         WHERE status='active' AND left_source=$1 AND left_external_id=$2
         UNION ALL
         SELECT left_source, left_external_id, rung FROM identity.crosswalk
         WHERE status='active' AND right_source=$1 AND right_external_id=$2",
    )
    .bind(source_slug)
    .bind(external_id)
    .fetch_all(pool)
    .await
    .context("crosswalk lekérdezés")?;
    rows.iter().map(crosswalk_from_row).collect()
}

fn crosswalk_from_row(row: &PgRow) -> Result<CrosswalkRow> {
    Ok(CrosswalkRow {
        right_source: row
            .try_get("right_source")
            .context("crosswalk right_source oszlop")?,
        right_external_id: row
            .try_get("right_external_id")
            .context("crosswalk right_external_id oszlop")?,
        rung: row.try_get("rung").context("crosswalk rung oszlop")?,
    })
}

const ALIAS_COLUMNS: &str =
    "entity_id, alias, lang, country_code, gender, age_group, branch, status";

fn alias_from_row(row: &PgRow) -> Result<AliasRow> {
    Ok(AliasRow {
        entity_id: row.try_get("entity_id").context("alias entity_id oszlop")?,
        alias: row.try_get("alias").context("alias alias oszlop")?,
        lang: row.try_get("lang").context("alias lang oszlop")?,
        country_code: row
            .try_get("country_code")
            .context("alias country_code oszlop")?,
        gender: row.try_get("gender").context("alias gender oszlop")?,
        age_group: row.try_get("age_group").context("alias age_group oszlop")?,
        branch: row.try_get("branch").context("alias branch oszlop")?,
        status: row.try_get("status").context("alias status oszlop")?,
    })
}

/// Reviewed aliasok foldolt név szerint (S1: `identity.fold_name(alias)`).
/// Csak `status='reviewed'`; pending/rejected alias nem old fel, és az
/// összevont (`core.entity.merged_into_id` nem NULL) entitás aliasa sem —
/// a merged entitás soha nem oldódhat fel (S2).
pub async fn reviewed_aliases_by_name(pool: &PgPool, folded_alias: &str) -> Result<Vec<AliasRow>> {
    let sql = format!(
        "SELECT {ALIAS_COLUMNS} FROM identity.alias a
         JOIN core.entity e ON e.id = a.entity_id AND e.merged_into_id IS NULL
         WHERE a.status = 'reviewed' AND identity.fold_name(a.alias) = identity.fold_name($1)
         ORDER BY a.created_at, a.alias"
    );
    let rows = sqlx::query(&sql)
        .bind(folded_alias)
        .fetch_all(pool)
        .await
        .context("reviewed alias lekérés")?;
    rows.iter().map(alias_from_row).collect()
}

/// Összes alias, opcionálisan egyetlen entitásra szűrve. `None` → minden alias.
pub async fn list_aliases(pool: &PgPool, entity_id: Option<Uuid>) -> Result<Vec<AliasRow>> {
    let sql = format!(
        "SELECT {ALIAS_COLUMNS} FROM identity.alias
         WHERE ($1::uuid IS NULL OR entity_id = $1)
         ORDER BY created_at, alias"
    );
    let rows = sqlx::query(&sql)
        .bind(entity_id)
        .fetch_all(pool)
        .await
        .context("alias lista")?;
    rows.iter().map(alias_from_row).collect()
}

/// Crosswalk-seed beszúrás (v1 import): reviewed alias sorok, ütközésnél
/// kihagyás. Visszaadja a ténylegesen beszúrt sorok számát.
pub async fn insert_alias_seeds(
    pool: &PgPool,
    seeds: &[football_domain::identity::AliasSeed],
) -> Result<usize> {
    let mut inserted = 0usize;
    for seed in seeds {
        let affected = sqlx::query(
            "INSERT INTO identity.alias
             (id, entity_id, alias, lang, country_code, gender, age_group, branch,
              status, method, reviewed_by, reviewed_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'reviewed','import','v1-seed',now())
             ON CONFLICT (entity_id, alias, lang, country_code, gender, age_group, branch)
             DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(seed.entity_id)
        .bind(&seed.alias)
        .bind(&seed.lang)
        .bind(&seed.country_code)
        .bind(&seed.gender)
        .bind(&seed.age_group)
        .bind(&seed.branch)
        .execute(pool)
        .await
        .context("alias seed beszúrás")?
        .rows_affected();
        inserted += affected as usize;
    }
    Ok(inserted)
}

const ENTITY_COLUMNS: &str =
    "e.id AS entity_id, e.canonical_name, c.code AS country_code, t.gender, t.age_group, t.branch";

const ENTITY_JOIN: &str = "FROM core.entity e
         JOIN core.team t ON t.entity_id = e.id
         LEFT JOIN core.country c ON c.id = t.country_id";

fn entity_from_row(row: &PgRow) -> Result<EntityRow> {
    Ok(EntityRow {
        entity_id: row
            .try_get("entity_id")
            .context("entity entity_id oszlop")?,
        canonical_name: row
            .try_get("canonical_name")
            .context("entity canonical_name oszlop")?,
        country_code: row
            .try_get("country_code")
            .context("entity country_code oszlop")?,
        gender: row.try_get("gender").context("entity gender oszlop")?,
        age_group: row
            .try_get("age_group")
            .context("entity age_group oszlop")?,
        branch: row.try_get("branch").context("entity branch oszlop")?,
    })
}

/// Csapat-entitások foldolt név szerint (S1: `identity.fold_name` mindkét
/// oldalon; core.entity canonical_name vagy core.team name). Az összevont
/// (`merged_into_id` nem NULL) entitásokat kihagyja. A LEFT JOIN miatt az
/// ország nélküli csapat is megjelenik (country_code = None), nem vesz el adat.
pub async fn team_entities_by_normalized_name(
    pool: &PgPool,
    folded_name: &str,
) -> Result<Vec<EntityRow>> {
    let sql = format!(
        "SELECT {ENTITY_COLUMNS} {ENTITY_JOIN}
         WHERE e.kind = 'team' AND e.merged_into_id IS NULL
           AND (identity.fold_name(e.canonical_name) = identity.fold_name($1)
                OR identity.fold_name(t.name) = identity.fold_name($1))
         ORDER BY e.canonical_name"
    );
    let rows = sqlx::query(&sql)
        .bind(folded_name)
        .fetch_all(pool)
        .await
        .context("csapat entitás lekérés név szerint")?;
    rows.iter().map(entity_from_row).collect()
}

/// Összes csapat-entitás a fuzzy jelölt-képzéshez.
///
/// S2: az összevont entitás nem lehet jelölt sem, ezért `merged_into_id IS
/// NULL` szűrő van. Dokumentált korlát: teljes szkennelés (nincs más szűrés);
/// S2b-1 szintetikus skálán elfogadható, S3-ban szűkítendő (pl. verseny/ország
/// szerinti halmaz).
pub async fn all_team_entities(pool: &PgPool) -> Result<Vec<EntityRow>> {
    let sql = format!(
        "SELECT {ENTITY_COLUMNS} {ENTITY_JOIN}
         WHERE e.kind = 'team' AND e.merged_into_id IS NULL
         ORDER BY e.canonical_name"
    );
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .context("összes csapat entitás lekérés")?;
    rows.iter().map(entity_from_row).collect()
}

/// Csapat-entitás a kanonikus névvel és a `core.team.name`-mel együtt (S41
/// identitás-javítás). A crosswalk-újraépítés mindkét tárolt nevet figyeli:
/// a régi importokban a kettő eltérhet, és a forrás neve bármelyikhez
/// illeszkedhet. Az archivált (`merged_into_id` nem NULL) entitásokat kihagyja.
#[derive(Debug, Clone)]
pub struct TeamNameRow {
    pub entity_id: Uuid,
    pub canonical_name: String,
    pub team_name: String,
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
}

impl TeamNameRow {
    pub fn context(&self) -> IdentityContext {
        IdentityContext {
            country_code: self.country_code.clone(),
            gender: self.gender.clone(),
            age_group: self.age_group.clone(),
            branch: self.branch.clone(),
        }
    }
}

/// Összes élő csapat-entitás mindkét névvel és a kontextusával (S41
/// crosswalk-újraépítés). Dokumentált korlát: teljes szkennelés; a javító
/// parancs egyszer tölti be és a memóriában oldja fel a kódokat.
pub async fn all_team_name_rows(pool: &PgPool) -> Result<Vec<TeamNameRow>> {
    let rows = sqlx::query(
        "SELECT e.id AS entity_id, e.canonical_name, t.name AS team_name,
                c.code AS country_code, t.gender, t.age_group, t.branch
           FROM core.entity e
           JOIN core.team t ON t.entity_id = e.id
           LEFT JOIN core.country c ON c.id = t.country_id
          WHERE e.kind = 'team' AND e.merged_into_id IS NULL
          ORDER BY e.canonical_name",
    )
    .fetch_all(pool)
    .await
    .context("csapat entitás lekérés mindkét névvel")?;
    rows.iter().map(team_name_from_row).collect()
}

fn team_name_from_row(row: &PgRow) -> Result<TeamNameRow> {
    Ok(TeamNameRow {
        entity_id: row
            .try_get("entity_id")
            .context("team_name entity_id oszlop")?,
        canonical_name: row
            .try_get("canonical_name")
            .context("team_name canonical_name oszlop")?,
        team_name: row
            .try_get("team_name")
            .context("team_name team_name oszlop")?,
        country_code: row
            .try_get("country_code")
            .context("team_name country_code oszlop")?,
        gender: row.try_get("gender").context("team_name gender oszlop")?,
        age_group: row
            .try_get("age_group")
            .context("team_name age_group oszlop")?,
        branch: row.try_get("branch").context("team_name branch oszlop")?,
    })
}

/// Pending review-sor upsert a részleges unique indexre. A kulcs
/// (source_id, entity_kind, source_entity_id, reason) WHERE status='pending';
/// a NULL source_id is egyenértékű kulcs (NULLS NOT DISTINCT). Ütközéskor a
/// jelöltlista/név/kontextus frissül, az id marad. Visszatérés: a sor id-ja.
///
/// FONTOS: az ON CONFLICT target megismétli a `WHERE status = 'pending'`
/// predikátumot, különben a részleges index inferencia nem sikerül.
pub async fn upsert_pending_review(pool: &PgPool, r: &NewReview) -> Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO identity.review_queue
         (id, source_id, entity_kind, source_entity_id, candidate_entity_ids,
          reason, proposed_name, context)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         ON CONFLICT (source_id, entity_kind, source_entity_id, reason)
           WHERE status = 'pending'
         DO UPDATE SET candidate_entity_ids = EXCLUDED.candidate_entity_ids,
           proposed_name = EXCLUDED.proposed_name, context = EXCLUDED.context
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(r.source_id)
    .bind(&r.entity_kind)
    .bind(&r.source_entity_id)
    .bind(&r.candidate_entity_ids)
    .bind(&r.reason)
    .bind(&r.proposed_name)
    .bind(&r.context)
    .fetch_one(pool)
    .await
    .context("review-sor upsert")
}

const REVIEW_COLUMNS: &str = "id, source_id, entity_kind, source_entity_id, candidate_entity_ids, \
     reason, proposed_name, context, status, created_at";

fn review_from_row(row: &PgRow) -> Result<ReviewRow> {
    Ok(ReviewRow {
        id: row.try_get("id").context("review id oszlop")?,
        source_id: row
            .try_get("source_id")
            .context("review source_id oszlop")?,
        entity_kind: row
            .try_get("entity_kind")
            .context("review entity_kind oszlop")?,
        source_entity_id: row
            .try_get("source_entity_id")
            .context("review source_entity_id oszlop")?,
        candidate_entity_ids: row
            .try_get("candidate_entity_ids")
            .context("review candidate_entity_ids oszlop")?,
        reason: row.try_get("reason").context("review reason oszlop")?,
        proposed_name: row
            .try_get("proposed_name")
            .context("review proposed_name oszlop")?,
        context: row.try_get("context").context("review context oszlop")?,
        status: row.try_get("status").context("review status oszlop")?,
        created_at: row
            .try_get("created_at")
            .context("review created_at oszlop")?,
    })
}

/// Egy review-sor lekérése id alapján.
pub async fn get_review(pool: &PgPool, id: Uuid) -> Result<Option<ReviewRow>> {
    let sql = format!("SELECT {REVIEW_COLUMNS} FROM identity.review_queue WHERE id = $1");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("review lekérés")?;
    row.as_ref().map(review_from_row).transpose()
}

/// Review-sorok listája, opcionális státusz-szűrővel, created_at DESC szerint.
pub async fn list_reviews(pool: &PgPool, status: Option<&str>) -> Result<Vec<ReviewRow>> {
    let sql = format!(
        "SELECT {REVIEW_COLUMNS} FROM identity.review_queue
         WHERE ($1::text IS NULL OR status = $1)
         ORDER BY created_at DESC"
    );
    let rows = sqlx::query(&sql)
        .bind(status)
        .fetch_all(pool)
        .await
        .context("review lista")?;
    rows.iter().map(review_from_row).collect()
}

/// Review-sor döntése: csak `pending` sor módosul (státusz, decided_by,
/// decided_at, decision_note). Visszatérés: érintett sorok száma (0 = már
/// döntött vagy nem létező). Executor-generikus: tranzakcióban is hívható.
pub async fn decide_review<'e, E>(
    executor: E,
    id: Uuid,
    new_status: &str,
    decided_by: &str,
    note: Option<&str>,
) -> Result<u64>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let res = sqlx::query(
        "UPDATE identity.review_queue
         SET status = $2, decided_by = $3, decided_at = now(), decision_note = $4
         WHERE id = $1 AND status = 'pending'",
    )
    .bind(id)
    .bind(new_status)
    .bind(decided_by)
    .bind(note)
    .execute(executor)
    .await
    .context("review döntés")?;
    Ok(res.rows_affected())
}

/// Kézi review-ref upsert: a (source_id, entity_kind, source_entity_id) kulcsra
/// beszúr vagy frissít, method='manual_review', status='active'. Executor-
/// generikus: tranzakcióban is hívható.
pub async fn insert_manual_ref<'e, E>(
    executor: E,
    source_id: Uuid,
    entity_kind: &str,
    source_entity_id: &str,
    entity_id: Uuid,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method, status)
         VALUES ($1,$2,$3,$4,$5,'manual_review','active')
         ON CONFLICT (source_id, entity_kind, source_entity_id)
         DO UPDATE SET entity_id = EXCLUDED.entity_id,
           method = 'manual_review', status = 'active'",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(entity_kind)
    .bind(source_entity_id)
    .bind(entity_id)
    .execute(executor)
    .await
    .context("kézi ref beszúrás")?;
    Ok(())
}

/// Létezik-e a kanonikus entitás.
pub async fn entity_exists(pool: &PgPool, entity_id: Uuid) -> Result<bool> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM core.entity WHERE id = $1)")
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .context("entitás létezés")
}

/// Egy csapat-entitás kontextusa (core.team + core.country). `None`, ha nincs
/// team sor az entitáshoz; ország nélküli csapatnál country_code = None.
pub async fn entity_context(pool: &PgPool, entity_id: Uuid) -> Result<Option<IdentityContext>> {
    let row = sqlx::query(
        "SELECT c.code AS country_code, t.gender, t.age_group, t.branch
         FROM core.team t LEFT JOIN core.country c ON c.id = t.country_id
         WHERE t.entity_id = $1",
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await
    .context("entitás kontextus")?;
    let Some(row) = row else { return Ok(None) };
    Ok(Some(IdentityContext {
        country_code: row
            .try_get("country_code")
            .context("kontextus country_code oszlop")?,
        gender: row.try_get("gender").context("kontextus gender oszlop")?,
        age_group: row
            .try_get("age_group")
            .context("kontextus age_group oszlop")?,
        branch: row.try_get("branch").context("kontextus branch oszlop")?,
    }))
}

/// Append-only identitás-döntés audit (ops.audit_event). A subject a review-sor
/// (subject_kind='review_queue', subject_id=queue_id); a detail jsonb tartalmazza
/// a döntés körülményeit. Executor-generikus: tranzakcióban is hívható.
pub async fn record_identity_decision<'e, E>(
    executor: E,
    actor: &str,
    action: &str,
    queue_id: Uuid,
    detail: serde_json::Value,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,$2,$3,'review_queue',$4,$5)",
    )
    .bind(Uuid::now_v7())
    .bind(actor)
    .bind(action)
    .bind(queue_id)
    .bind(detail)
    .execute(executor)
    .await
    .context("identitás döntés audit")?;
    Ok(())
}

/// Entitás kind-ja (approve cross-kind ellenőrzéshez).
pub async fn entity_kind(pool: &PgPool, entity_id: Uuid) -> Result<Option<String>> {
    sqlx::query_scalar("SELECT kind FROM core.entity WHERE id = $1")
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .context("entitás kind lekérés")
}

/// Pending review-k összesítése a triage-hez: nem/korosztály/ország és a
/// legutóbbi megfigyelt verseny szerint (a team-observation nem hordoz
/// versenyt, ezért a match_core payloadból kötjük).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ReviewSummaryRow {
    pub gender: Option<String>,
    pub branch: Option<String>,
    pub competition: Option<String>,
    pub count: i64,
    pub countries: Option<String>,
    pub sample_names: Option<String>,
}

pub async fn review_summary(pool: &PgPool) -> Result<Vec<ReviewSummaryRow>> {
    sqlx::query_as(
        "SELECT r.context->>'gender' AS gender,
                r.context->>'branch' AS branch,
                comp.name AS competition,
                count(*)::bigint AS count,
                string_agg(DISTINCT coalesce(r.context->>'country_code', '?'), ',') AS countries,
                (array_agg(r.proposed_name ORDER BY r.proposed_name))[1:5]::text AS sample_names
         FROM identity.review_queue r
         LEFT JOIN LATERAL (
             SELECT o.normalized_payload->'competition'->>'name' AS name
             FROM stage.observation o
             WHERE r.source_id IS NOT NULL
               AND o.source_id = r.source_id
               AND o.aspect = 'match_core'
               AND (o.normalized_payload->>'source_home_id' = r.source_entity_id
                 OR o.normalized_payload->>'source_away_id' = r.source_entity_id)
             ORDER BY o.observed_at DESC
             LIMIT 1
         ) comp ON true
         WHERE r.status = 'pending'
         GROUP BY 1, 2, 3
         ORDER BY count(*) DESC, competition NULLS LAST, sample_names",
    )
    .fetch_all(pool)
    .await
    .context("review összesítés")
}
