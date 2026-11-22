//! Verseny-entitások merge-elése (S81).
//!
//! A csapat-merge (`identity_merge.rs`) mintájára, de verseny-specifikus
//! szabályokkal: a beolvadó `core.competition` sor NEM törlődik, a meccsek és
//! seasonök a survivor versenyre kerülnek, az entitás `merged_into_id` jelölést
//! kap.
//!
//! Season-ütközés kezelése: ha a beolvadó versenynek ugyanolyan labelű
//! seasonje van, mint a survivornak, akkor a meccsek a survivor seasonjére
//! mozognak, és az üressé vált beolvadó season törlődik (a label adatai a
//! survivoron megmaradnak, az archívum pedig megőrzi az eredeti sort).
//!
//! `identity.crosswalk`-nak nincs entitás-oszlopa (külső id-párokat tárol),
//! ezért itt nincs mit átírni: a `source_entity_ref` átmozgatásával a
//! crosswalk-on keresztüli feloldás is a survivorra mutat.

use anyhow::{Context, Result, bail};
use serde_json::json;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Olvasás-only előnézet a CLI dry-runhoz.
#[derive(Debug, Clone)]
pub struct CompetitionMergePreview {
    pub survivor_entity_id: Uuid,
    pub duplicate_entity_id: Uuid,
    pub survivor_name: String,
    pub duplicate_name: String,
    pub survivor_matches: i64,
    pub duplicate_matches: i64,
    pub matches_to_move: i64,
    pub seasons_moved: i64,
    pub seasons_merged: i64,
    pub aliases: i64,
    pub refs: i64,
    /// Ha a beolvadó entitás már a survivorba van merge-elve, nincs teendő.
    pub already_merged: bool,
}

/// A lefutott merge eredménye.
#[derive(Debug, Clone)]
pub struct CompetitionMergeReport {
    pub survivor_entity_id: Uuid,
    pub duplicate_entity_id: Uuid,
    pub survivor_competition_id: Uuid,
    pub duplicate_competition_id: Uuid,
    pub survivor_name: String,
    pub duplicate_name: String,
    pub matches_moved: i64,
    pub seasons_moved: i64,
    pub seasons_merged: i64,
    pub aliases_moved: i64,
    pub aliases_dropped: i64,
    pub refs_moved: i64,
    pub duplicate_match_groups: i64,
}

/// A merge kimenete: megtörtént, vagy már korábban megtörtént (idempotencia).
#[derive(Debug, Clone)]
pub enum CompetitionMergeOutcome {
    Merged(Box<CompetitionMergeReport>),
    AlreadyMerged {
        survivor_entity_id: Uuid,
        duplicate_entity_id: Uuid,
    },
}

#[derive(Debug, Clone)]
struct EntityRow {
    name: String,
    kind: String,
    merged_into: Option<Uuid>,
}

#[derive(Debug, Clone)]
struct CompetitionRow {
    competition_id: Uuid,
    name: String,
    country_code: Option<String>,
}

#[derive(Debug, Clone)]
struct Pair {
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
    survivor_competition: Uuid,
    duplicate_competition: Uuid,
    survivor_name: String,
    duplicate_name: String,
    survivor_matches: i64,
    duplicate_matches: i64,
    /// (beolvadó season, survivor season, label) hármasok.
    season_collisions: Vec<(Uuid, Uuid, String)>,
    seasons_moved: i64,
    aliases: i64,
    refs: i64,
    already_merged: bool,
}

async fn load_entity(conn: &mut PgConnection, id: Uuid) -> Result<EntityRow> {
    let row: Option<(Uuid, String, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, canonical_name, kind, merged_into_id FROM core.entity WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await
    .context("entitás olvasása")?;
    let (_entity_id, name, kind, merged_into) =
        row.with_context(|| format!("nincs entitás: {id}"))?;
    Ok(EntityRow {
        name,
        kind,
        merged_into,
    })
}

async fn load_competition(conn: &mut PgConnection, entity_id: Uuid) -> Result<CompetitionRow> {
    let row: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT c.id, c.name, co.code
         FROM core.competition c
         LEFT JOIN core.country co ON co.id = c.country_id
         WHERE c.entity_id = $1",
    )
    .bind(entity_id)
    .fetch_optional(&mut *conn)
    .await
    .context("verseny olvasása")?;
    let (competition_id, name, country_code) =
        row.with_context(|| format!("az entitásnak nincs verseny-sora: {entity_id}"))?;
    Ok(CompetitionRow {
        competition_id,
        name,
        country_code,
    })
}

async fn count_matches(conn: &mut PgConnection, competition_id: Uuid) -> Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM core.match WHERE competition_id = $1")
        .bind(competition_id)
        .fetch_one(&mut *conn)
        .await
        .context("meccsszám olvasása")
}

async fn count_aliases(conn: &mut PgConnection, entity_id: Uuid) -> Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM identity.alias WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_one(&mut *conn)
        .await
        .context("alias-szám olvasása")
}

async fn count_refs(conn: &mut PgConnection, entity_id: Uuid) -> Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM identity.source_entity_ref WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_one(&mut *conn)
        .await
        .context("source-ref szám olvasása")
}

/// Betölti és őrzi a merge párost; már megtörtént merge esetén jelzi.
async fn load_pair(
    conn: &mut PgConnection,
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
    force: bool,
) -> Result<Pair> {
    if survivor_entity == duplicate_entity {
        bail!("a survivor és a beolvadó entitás nem lehet ugyanaz: {survivor_entity}");
    }

    let survivor = load_entity(conn, survivor_entity).await?;
    let duplicate = load_entity(conn, duplicate_entity).await?;

    if survivor.merged_into.is_some() {
        bail!("a survivor entitás már merge-ölt: {survivor_entity}");
    }
    if let Some(target) = duplicate.merged_into {
        if target == survivor_entity {
            return Ok(Pair {
                survivor_entity,
                duplicate_entity,
                survivor_competition: Uuid::nil(),
                duplicate_competition: Uuid::nil(),
                survivor_name: survivor.name,
                duplicate_name: duplicate.name,
                survivor_matches: 0,
                duplicate_matches: 0,
                season_collisions: Vec::new(),
                seasons_moved: 0,
                aliases: 0,
                refs: 0,
                already_merged: true,
            });
        }
        bail!("a beolvadó entitás már más entitásba merge-ölt: {duplicate_entity}");
    }

    if survivor.kind != "competition" || duplicate.kind != "competition" {
        bail!(
            "mindkét entitásnak competition kindúnak kell lennie (survivor: {}, beolvadó: {})",
            survivor.kind,
            duplicate.kind
        );
    }

    let survivor_comp = load_competition(conn, survivor_entity).await?;
    let duplicate_comp = load_competition(conn, duplicate_entity).await?;

    // A verseny megjelenített neve és az entitás kanonikus neve eltérhet
    // (pl. az egyik sor átnevezés után keletkezett); a merge akkor biztonságos,
    // ha a két verseny-sor vagy a két entitás neve fold-olva egyezik.
    let entity_fold_same = football_domain::identity::fold_name(&survivor.name)
        == football_domain::identity::fold_name(&duplicate.name);
    let competition_fold_same = football_domain::identity::fold_name(&survivor_comp.name)
        == football_domain::identity::fold_name(&duplicate_comp.name);
    if !force && !entity_fold_same && !competition_fold_same {
        bail!(
            "a két verseny fold-olt neve eltér: {:?}/{:?} != {:?}/{:?}",
            survivor.name,
            survivor_comp.name,
            duplicate.name,
            duplicate_comp.name
        );
    }

    match (&survivor_comp.country_code, &duplicate_comp.country_code) {
        (Some(left), Some(right)) if left != right => {
            bail!("a két verseny országa eltér: {left} != {right}");
        }
        _ => {}
    }

    let survivor_matches = count_matches(conn, survivor_comp.competition_id).await?;
    let duplicate_matches = count_matches(conn, duplicate_comp.competition_id).await?;
    if !force && survivor_matches < duplicate_matches {
        bail!(
            "a survivor versenynek kevesebb meccse van ({survivor_matches}), mint a beolvadónak \
             ({duplicate_matches}); ez adatvesztés lenne, ezért csak --force esetén engedett"
        );
    }

    let collisions: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT d.id, s.id, d.label
         FROM core.season d
         JOIN core.season s ON s.label = d.label AND s.competition_id = $2
         WHERE d.competition_id = $1
         ORDER BY d.label",
    )
    .bind(duplicate_comp.competition_id)
    .bind(survivor_comp.competition_id)
    .fetch_all(&mut *conn)
    .await
    .context("ütköző seasonök olvasása")?;

    let seasons_moved: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM core.season d
         WHERE d.competition_id = $1
           AND NOT EXISTS (
               SELECT 1 FROM core.season s
               WHERE s.competition_id = $2 AND s.label = d.label
           )",
    )
    .bind(duplicate_comp.competition_id)
    .bind(survivor_comp.competition_id)
    .fetch_one(&mut *conn)
    .await
    .context("átmozgatandó seasonök számlálása")?;

    Ok(Pair {
        survivor_entity,
        duplicate_entity,
        survivor_competition: survivor_comp.competition_id,
        duplicate_competition: duplicate_comp.competition_id,
        survivor_name: survivor.name,
        duplicate_name: duplicate.name,
        survivor_matches,
        duplicate_matches,
        season_collisions: collisions,
        seasons_moved,
        aliases: count_aliases(conn, duplicate_entity).await?,
        refs: count_refs(conn, duplicate_entity).await?,
        already_merged: false,
    })
}

/// Olvasás-only előnézet; a guardok itt is lefutnak, hogy a dry-run őszinte legyen.
pub async fn competition_merge_preview(
    pool: &PgPool,
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
    force: bool,
) -> Result<CompetitionMergePreview> {
    let mut conn = pool.acquire().await.context("adatbázis-kapcsolat")?;
    let pair = load_pair(&mut conn, survivor_entity, duplicate_entity, force).await?;
    Ok(CompetitionMergePreview {
        survivor_entity_id: pair.survivor_entity,
        duplicate_entity_id: pair.duplicate_entity,
        survivor_name: pair.survivor_name,
        duplicate_name: pair.duplicate_name,
        survivor_matches: pair.survivor_matches,
        duplicate_matches: pair.duplicate_matches,
        matches_to_move: pair.duplicate_matches,
        seasons_moved: pair.seasons_moved,
        seasons_merged: i64::try_from(pair.season_collisions.len()).unwrap_or(i64::MAX),
        aliases: pair.aliases,
        refs: pair.refs,
        already_merged: pair.already_merged,
    })
}

/// A merge végrehajtása egy tranzakcióban; ismételt hívás nem ír.
pub async fn merge_competition(
    pool: &PgPool,
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
    note: &str,
    force: bool,
) -> Result<CompetitionMergeOutcome> {
    let mut tx = pool.begin().await.context("merge tranzakció indítása")?;
    let pair = load_pair(&mut tx, survivor_entity, duplicate_entity, force).await?;
    if pair.already_merged {
        tx.rollback()
            .await
            .context("merge tranzakció visszagörgetése")?;
        return Ok(CompetitionMergeOutcome::AlreadyMerged {
            survivor_entity_id: survivor_entity,
            duplicate_entity_id: duplicate_entity,
        });
    }

    let archive_id = Uuid::now_v7();
    insert_archive(&mut tx, archive_id, &pair, note).await?;

    let mut matches_moved = 0_i64;
    let mut seasons_merged = 0_i64;
    for (duplicate_season, survivor_season, _label) in &pair.season_collisions {
        let moved = sqlx::query(
            "UPDATE core.match
             SET competition_id = $1, season_id = $2
             WHERE competition_id = $3 AND season_id = $4",
        )
        .bind(pair.survivor_competition)
        .bind(survivor_season)
        .bind(pair.duplicate_competition)
        .bind(duplicate_season)
        .execute(&mut *tx)
        .await
        .context("ütköző season meccseinek átmozgatása")?;
        matches_moved += moved.rows_affected() as i64;

        let deleted = sqlx::query("DELETE FROM core.season WHERE id = $1")
            .bind(duplicate_season)
            .execute(&mut *tx)
            .await
            .context("üressé vált beolvadó season törlése")?;
        if deleted.rows_affected() != 1 {
            bail!("a beolvadó season nem törölhető: {duplicate_season}");
        }
        seasons_merged += 1;
    }

    let seasons_moved =
        sqlx::query("UPDATE core.season SET competition_id = $1 WHERE competition_id = $2")
            .bind(pair.survivor_competition)
            .bind(pair.duplicate_competition)
            .execute(&mut *tx)
            .await
            .context("seasonök átmozgatása")?;
    let seasons_moved = seasons_moved.rows_affected() as i64;

    let moved = sqlx::query("UPDATE core.match SET competition_id = $1 WHERE competition_id = $2")
        .bind(pair.survivor_competition)
        .bind(pair.duplicate_competition)
        .execute(&mut *tx)
        .await
        .context("meccsek átmozgatása")?;
    matches_moved += moved.rows_affected() as i64;

    let aliases_dropped = sqlx::query(
        "DELETE FROM identity.alias d
         WHERE d.entity_id = $1
           AND EXISTS (
               SELECT 1 FROM identity.alias s
               WHERE s.entity_id = $2
                 AND s.alias = d.alias
                 AND s.lang = d.lang
                 AND s.country_code IS NOT DISTINCT FROM d.country_code
                 AND s.gender IS NOT DISTINCT FROM d.gender
                 AND s.age_group IS NOT DISTINCT FROM d.age_group
                 AND s.branch IS NOT DISTINCT FROM d.branch
           )",
    )
    .bind(pair.duplicate_entity)
    .bind(pair.survivor_entity)
    .execute(&mut *tx)
    .await
    .context("ütköző aliasok törlése")?;
    let aliases_dropped = aliases_dropped.rows_affected() as i64;

    let aliases_moved =
        sqlx::query("UPDATE identity.alias SET entity_id = $1 WHERE entity_id = $2")
            .bind(pair.survivor_entity)
            .bind(pair.duplicate_entity)
            .execute(&mut *tx)
            .await
            .context("aliasok átmozgatása")?;
    let aliases_moved = aliases_moved.rows_affected() as i64;

    // S81: a merge után a túlélő entitásra írjuk a két verseny megjelenített
    // nevét és a két entitás kanonikus nevét aliasként, ország-kontextussal —
    // enélkül a következő ingest ugyanazt a nevet új versenyként hozná létre.
    let (survivor_comp_name, survivor_country): (String, Option<String>) = sqlx::query_as(
        "SELECT c.name, co.code
         FROM core.competition c
         LEFT JOIN core.country co ON co.id = c.country_id
         WHERE c.id = $1",
    )
    .bind(pair.survivor_competition)
    .fetch_one(&mut *tx)
    .await
    .context("túlélő verseny nevének olvasása")?;
    let (duplicate_comp_name, _duplicate_country): (String, Option<String>) = sqlx::query_as(
        "SELECT c.name, co.code
         FROM core.competition c
         LEFT JOIN core.country co ON co.id = c.country_id
         WHERE c.id = $1",
    )
    .bind(pair.duplicate_competition)
    .fetch_one(&mut *tx)
    .await
    .context("beolvadó verseny nevének olvasása")?;
    for name in [
        survivor_comp_name.as_str(),
        pair.survivor_name.as_str(),
        duplicate_comp_name.as_str(),
        pair.duplicate_name.as_str(),
    ] {
        sqlx::query(
            "INSERT INTO identity.alias
                 (id, entity_id, alias, lang, country_code, status, method)
             VALUES ($1, $2, $3, 'und', $4, 'reviewed', 'import')
             ON CONFLICT DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(pair.survivor_entity)
        .bind(name)
        .bind(survivor_country.as_deref())
        .execute(&mut *tx)
        .await
        .context("merge utáni alias írása")?;
    }

    let refs_moved =
        sqlx::query("UPDATE identity.source_entity_ref SET entity_id = $1 WHERE entity_id = $2")
            .bind(pair.survivor_entity)
            .bind(pair.duplicate_entity)
            .execute(&mut *tx)
            .await
            .context("source_entity_ref átmozgatása")?;
    let refs_moved = refs_moved.rows_affected() as i64;

    sqlx::query("UPDATE core.entity SET merged_into_id = $1 WHERE id = $2")
        .bind(pair.survivor_entity)
        .bind(pair.duplicate_entity)
        .execute(&mut *tx)
        .await
        .context("merged_into_id beállítása")?;

    let duplicate_match_groups = duplicate_match_groups(&mut tx, pair.survivor_competition).await?;
    finalize_archive(&mut tx, archive_id, &pair, duplicate_match_groups).await?;

    // S97: a beolvadó verseny-sora nem maradhat a DB-ben. A merge a meccseket és
    // a szezonokat átmozgatja, de a puszta `core.competition` sor tovább élt, és
    // egy későbbi ingest (bridge- vagy id-hint út) újra rá köthette a meccseket —
    // így ugyanaz a liga két csoportban jelent meg a kliensben. A törlés a merge
    // tranzakció része, a meccsek és szezonok már a survivor alatt vannak.
    let deleted = sqlx::query("DELETE FROM core.competition WHERE id = $1")
        .bind(pair.duplicate_competition)
        .execute(&mut *tx)
        .await
        .context("beolvadó verseny sorának törlése")?;
    if deleted.rows_affected() != 1 {
        bail!(
            "a beolvadó verseny sora nem törölhető: {}",
            pair.duplicate_competition
        );
    }
    insert_audit(
        &mut tx,
        &pair,
        note,
        matches_moved,
        seasons_moved,
        seasons_merged,
    )
    .await?;

    tx.commit().await.context("merge tranzakció commitja")?;

    Ok(CompetitionMergeOutcome::Merged(Box::new(
        CompetitionMergeReport {
            survivor_entity_id: pair.survivor_entity,
            duplicate_entity_id: pair.duplicate_entity,
            survivor_competition_id: pair.survivor_competition,
            duplicate_competition_id: pair.duplicate_competition,
            survivor_name: pair.survivor_name,
            duplicate_name: pair.duplicate_name,
            matches_moved,
            seasons_moved,
            seasons_merged,
            aliases_moved,
            aliases_dropped,
            refs_moved,
            duplicate_match_groups,
        },
    )))
}

async fn insert_archive(
    tx: &mut Transaction<'_, Postgres>,
    archive_id: Uuid,
    pair: &Pair,
    note: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO ops.entity_merge_archive
             (id, merged_at, survivor_entity_id, duplicate_entity_id, note, payload)
         SELECT $1, now(), $2, $3, $4,
                jsonb_build_object(
                    'entity', jsonb_build_object(
                        'survivor', to_jsonb(se),
                        'duplicate', to_jsonb(de)
                    ),
                    'competition', jsonb_build_object(
                        'survivor', to_jsonb(sc),
                        'duplicate', to_jsonb(dc)
                    ),
                    'aliases', (SELECT coalesce(jsonb_agg(to_jsonb(a)), '[]'::jsonb)
                                FROM identity.alias a WHERE a.entity_id = $3),
                    'refs', (SELECT coalesce(jsonb_agg(to_jsonb(r)), '[]'::jsonb)
                             FROM identity.source_entity_ref r WHERE r.entity_id = $3),
                    'seasons', (SELECT coalesce(jsonb_agg(to_jsonb(s)), '[]'::jsonb)
                                FROM core.season s WHERE s.competition_id = $5),
                    'match_ids', (SELECT coalesce(jsonb_agg(m.id), '[]'::jsonb)
                                  FROM core.match m WHERE m.competition_id = $5),
                    'counts_before', jsonb_build_object(
                        'survivor_matches', $6,
                        'duplicate_matches', $7,
                        'survivor_seasons', (SELECT count(*) FROM core.season WHERE competition_id = $4),
                        'duplicate_seasons', (SELECT count(*) FROM core.season WHERE competition_id = $5),
                        'aliases', (SELECT count(*) FROM identity.alias WHERE entity_id = $3),
                        'refs', (SELECT count(*) FROM identity.source_entity_ref WHERE entity_id = $3)
                    )
                )
         FROM core.entity se, core.entity de, core.competition sc, core.competition dc
         WHERE se.id = $2 AND de.id = $3 AND sc.id = $4 AND dc.id = $5",
    )
    .bind(archive_id)
    .bind(pair.survivor_entity)
    .bind(pair.duplicate_entity)
    .bind(pair.survivor_competition)
    .bind(pair.duplicate_competition)
    .bind(note)
    .bind(pair.survivor_matches)
    .bind(pair.duplicate_matches)
    .execute(&mut **tx)
    .await
    .context("merge-archívum írása")?;
    Ok(())
}

async fn finalize_archive(
    tx: &mut Transaction<'_, Postgres>,
    archive_id: Uuid,
    pair: &Pair,
    duplicate_match_groups: i64,
) -> Result<()> {
    sqlx::query(
        "UPDATE ops.entity_merge_archive
         SET payload = payload || jsonb_build_object(
                 'counts_after', jsonb_build_object(
                     'matches', (SELECT count(*) FROM core.match WHERE competition_id = $2),
                     'seasons', (SELECT count(*) FROM core.season WHERE competition_id = $2),
                     'aliases', (SELECT count(*) FROM identity.alias WHERE entity_id = $3),
                     'refs', (SELECT count(*) FROM identity.source_entity_ref WHERE entity_id = $3)
                 ),
                 'duplicate_match_groups', $4
             )
         WHERE id = $1",
    )
    .bind(archive_id)
    .bind(pair.survivor_competition)
    .bind(pair.survivor_entity)
    .bind(duplicate_match_groups)
    .execute(&mut **tx)
    .await
    .context("merge-archívum utólagos számlálása")?;
    Ok(())
}

async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    pair: &Pair,
    note: &str,
    matches_moved: i64,
    seasons_moved: i64,
    seasons_merged: i64,
) -> Result<()> {
    let detail = json!({
        "survivor_name": pair.survivor_name,
        "duplicate_name": pair.duplicate_name,
        "matches_moved": matches_moved,
        "seasons_moved": seasons_moved,
        "seasons_merged": seasons_merged,
        "note": note,
    });
    sqlx::query(
        "INSERT INTO ops.audit_event
             (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, 'cli', 'competition_merge', 'competition', $2, $3)",
    )
    .bind(Uuid::now_v7())
    .bind(pair.survivor_entity)
    .bind(detail)
    .execute(&mut **tx)
    .await
    .context("audit-esemény írása")?;
    Ok(())
}

async fn duplicate_match_groups(
    tx: &mut Transaction<'_, Postgres>,
    competition_id: Uuid,
) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM (
             SELECT 1
             FROM core.match
             WHERE competition_id = $1
             GROUP BY season_id, round_label, home_team_id, away_team_id, kickoff_at
             HAVING count(*) > 1
         ) duplicates",
    )
    .bind(competition_id)
    .fetch_one(&mut **tx)
    .await
    .context("ütköző meccscsoportok számlálása")
}
