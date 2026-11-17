//! Csapat-entitás merge (S19): azonos nevű/kontextusú duplikált kanonikus
//! csapatok összevonása. A merge tranzakciós: archívum → meccs-/tabella-/
//! asset-/alias-/ref-átmozgatás → duplikált team-sor törlése → az áldozat
//! entitás `merged_into_id`-vel megjelölve (lenyomat marad) → audit.
//!
//! A merge csak **azonos normalizált név + azonos kontextus** esetén megy
//! át (nincs fuzzy); a konfliktusos gyermeksorok (alias, standing,
//! szezon-statisztika, asset) az áldozat oldaláról eldobásra kerülnek, de
//! az archívumban megmaradnak.

use anyhow::{Context, Result, bail};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DuplicateTeamRow {
    pub entity_id: Uuid,
    pub team_id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
    pub matches: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct DuplicateTeamGroup {
    pub name: String,
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
    pub teams: Vec<DuplicateTeamRow>,
}

/// Azonos normalizált nevű és kontextusú (ország/nem/korosztály/ág),
/// még nem merge-ölt csapat-entitások csoportjai (≥2 tag).
pub async fn duplicate_team_groups(pool: &PgPool) -> Result<Vec<DuplicateTeamGroup>> {
    type Row = (
        Uuid,
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
        chrono::DateTime<chrono::Utc>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT e.id, t.id, e.canonical_name, c.code, t.gender, t.age_group, t.branch,
                (SELECT count(*) FROM core.match m
                  WHERE m.home_team_id = t.id OR m.away_team_id = t.id) AS matches,
                e.created_at
         FROM core.entity e
         JOIN core.team t ON t.entity_id = e.id
         LEFT JOIN core.country c ON c.id = t.country_id
         WHERE e.kind = 'team' AND e.merged_into_id IS NULL
         ORDER BY lower(e.canonical_name), e.created_at",
    )
    .fetch_all(pool)
    .await
    .context("csapat-entitás duplikátum lista")?;

    let mut groups: Vec<DuplicateTeamGroup> = Vec::new();
    for (entity_id, team_id, name, cc, gender, age_group, branch, matches, created_at) in rows {
        let key = (
            name.to_lowercase(),
            cc.clone(),
            gender.clone(),
            age_group.clone(),
            branch.clone(),
        );
        let row = DuplicateTeamRow {
            entity_id,
            team_id,
            name: name.clone(),
            country_code: cc.clone(),
            gender: gender.clone(),
            age_group: age_group.clone(),
            branch: branch.clone(),
            matches,
            created_at,
        };
        match groups.iter_mut().find(|g| {
            (
                g.name.to_lowercase(),
                g.country_code.clone(),
                g.gender.clone(),
                g.age_group.clone(),
                g.branch.clone(),
            ) == key
        }) {
            Some(group) => group.teams.push(row),
            None => groups.push(DuplicateTeamGroup {
                name,
                country_code: cc,
                gender,
                age_group,
                branch,
                teams: vec![row],
            }),
        }
    }
    groups.retain(|g| g.teams.len() > 1);
    Ok(groups)
}

#[derive(Debug, Default, Clone)]
pub struct TeamMergePreview {
    pub survivor_name: String,
    pub duplicate_name: String,
    pub matches_moved: i64,
    /// Az áldozat meccsei közül azok, amelyek a túlélőnél ugyanarra a
    /// verseny+szezon+kickoff+ellenfél párosra már léteznek.
    pub potential_duplicate_matches: i64,
    pub aliases: i64,
    pub refs: i64,
    pub standings: i64,
    pub season_stats: i64,
    pub assets: i64,
}

/// Száraz előnézet: mit mozgatna a merge (írás nélkül).
pub async fn merge_team_preview(
    pool: &PgPool,
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
) -> Result<TeamMergePreview> {
    let mut conn = pool.acquire().await?;
    let (survivor_team, survivor_name, dup_team, dup_name) =
        load_pair(&mut conn, survivor_entity, duplicate_entity).await?;
    drop(conn);

    let count1 = |sql: &'static str, a: Uuid| async move {
        sqlx::query_scalar::<_, i64>(sql)
            .bind(a)
            .fetch_one(pool)
            .await
    };
    let count2 = |sql: &'static str, a: Uuid, b: Uuid| async move {
        sqlx::query_scalar::<_, i64>(sql)
            .bind(a)
            .bind(b)
            .fetch_one(pool)
            .await
    };
    let matches_moved = count1(
        "SELECT count(*) FROM core.match m
          WHERE m.home_team_id = $1 OR m.away_team_id = $1",
        dup_team,
    )
    .await?;
    let potential_duplicate_matches = count2(
        "SELECT count(*) FROM core.match d
          WHERE (d.home_team_id = $1 OR d.away_team_id = $1)
            AND EXISTS (
              SELECT 1 FROM core.match s
               WHERE s.id <> d.id
                 AND s.competition_id = d.competition_id
                 AND s.season_id = d.season_id
                 AND s.kickoff_at = d.kickoff_at
                 AND s.home_team_id = CASE WHEN d.home_team_id = $1 THEN $2 ELSE d.home_team_id END
                 AND s.away_team_id = CASE WHEN d.away_team_id = $1 THEN $2 ELSE d.away_team_id END)",
        dup_team,
        survivor_team,
    )
    .await?;
    let aliases = count1(
        "SELECT count(*) FROM identity.alias WHERE entity_id = $1",
        duplicate_entity,
    )
    .await?;
    let refs = count1(
        "SELECT count(*) FROM identity.source_entity_ref WHERE entity_id = $1",
        duplicate_entity,
    )
    .await?;
    let standings = count1(
        "SELECT count(*) FROM core.standing WHERE team_id = $1",
        dup_team,
    )
    .await?;
    let season_stats = count1(
        "SELECT count(*) FROM fact.season_team_stats WHERE team_id = $1",
        dup_team,
    )
    .await?;
    let assets = count1(
        "SELECT count(*) FROM ops.asset WHERE entity_kind = 'team' AND entity_id = $1",
        dup_team,
    )
    .await?;

    Ok(TeamMergePreview {
        survivor_name,
        duplicate_name: dup_name,
        matches_moved,
        potential_duplicate_matches,
        aliases,
        refs,
        standings,
        season_stats,
        assets,
    })
}

#[derive(Debug, Default, Clone)]
pub struct TeamMergeReport {
    pub matches_moved: u64,
    pub aliases_moved: u64,
    pub aliases_dropped: u64,
    pub refs_moved: u64,
    pub standings_moved: u64,
    pub standings_dropped: u64,
    pub season_stats_moved: u64,
    pub season_stats_dropped: u64,
    pub assets_moved: u64,
    pub assets_dropped: u64,
    pub duplicate_matches: i64,
}

/// A duplikált csapat-entitás összevonása a túlélőbe (tranzakció).
pub async fn merge_team(
    pool: &PgPool,
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
    note: &str,
) -> Result<TeamMergeReport> {
    let mut tx = pool.begin().await?;

    let (survivor_team, survivor_name, dup_team, dup_name) =
        load_pair(&mut tx, survivor_entity, duplicate_entity).await?;

    // 1. Archívum: a teljes előtte-állapot (visszaállítási út).
    sqlx::query(
        "INSERT INTO ops.entity_merge_archive
         (id, merged_at, survivor_entity_id, duplicate_entity_id, note, payload)
         SELECT $1, now(), $2, $3, $4, jsonb_build_object(
            'entity',        (SELECT to_jsonb(e) FROM core.entity e WHERE e.id = $3),
            'team',          (SELECT to_jsonb(t) FROM core.team t WHERE t.entity_id = $3),
            'aliases',       (SELECT COALESCE(jsonb_agg(to_jsonb(a)), '[]'::jsonb)
                                FROM identity.alias a WHERE a.entity_id = $3),
            'refs',          (SELECT COALESCE(jsonb_agg(to_jsonb(r)), '[]'::jsonb)
                                FROM identity.source_entity_ref r WHERE r.entity_id = $3),
            'match_ids',     (SELECT COALESCE(jsonb_agg(m.id), '[]'::jsonb)
                                FROM core.match m
                               WHERE m.home_team_id = (SELECT id FROM core.team WHERE entity_id = $3)
                                  OR m.away_team_id = (SELECT id FROM core.team WHERE entity_id = $3)),
            'standings',     (SELECT COALESCE(jsonb_agg(to_jsonb(s)), '[]'::jsonb)
                                FROM core.standing s
                               WHERE s.team_id = (SELECT id FROM core.team WHERE entity_id = $3)),
            'season_stats',  (SELECT COALESCE(jsonb_agg(to_jsonb(x)), '[]'::jsonb)
                                FROM fact.season_team_stats x
                               WHERE x.team_id = (SELECT id FROM core.team WHERE entity_id = $3)),
            'assets',        (SELECT COALESCE(jsonb_agg(to_jsonb(a)), '[]'::jsonb)
                                FROM ops.asset a
                               WHERE a.entity_kind = 'team'
                                 AND a.entity_id = (SELECT id FROM core.team WHERE entity_id = $3)),
            'survivor_entity', (SELECT to_jsonb(e) FROM core.entity e WHERE e.id = $2)
         )",
    )
    .bind(Uuid::now_v7())
    .bind(survivor_entity)
    .bind(duplicate_entity)
    .bind(note)
    .execute(&mut *tx)
    .await
    .context("entitás-merge archívum")?;

    // 2. Meccsek átmozgatása.
    let matches_moved =
        sqlx::query("UPDATE core.match SET home_team_id = $1 WHERE home_team_id = $2")
            .bind(survivor_team)
            .bind(dup_team)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            + sqlx::query("UPDATE core.match SET away_team_id = $1 WHERE away_team_id = $2")
                .bind(survivor_team)
                .bind(dup_team)
                .execute(&mut *tx)
                .await?
                .rows_affected();

    // 3. Tabella-sorok: a túlélőnél létező szezon-konfliktusok eldobása.
    let standings_dropped = sqlx::query(
        "DELETE FROM core.standing s
          WHERE s.team_id = $2
            AND EXISTS (SELECT 1 FROM core.standing x
                         WHERE x.team_id = $1 AND x.season_id = s.season_id)",
    )
    .bind(survivor_team)
    .bind(dup_team)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let standings_moved = sqlx::query("UPDATE core.standing SET team_id = $1 WHERE team_id = $2")
        .bind(survivor_team)
        .bind(dup_team)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    // 4. Szezon-statisztika (PK: season, team, source, stat_key).
    let season_stats_dropped = sqlx::query(
        "DELETE FROM fact.season_team_stats x
          WHERE x.team_id = $2
            AND EXISTS (SELECT 1 FROM fact.season_team_stats y
                         WHERE y.team_id = $1 AND y.season_id = x.season_id
                           AND y.source_id = x.source_id AND y.stat_key = x.stat_key)",
    )
    .bind(survivor_team)
    .bind(dup_team)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let season_stats_moved =
        sqlx::query("UPDATE fact.season_team_stats SET team_id = $1 WHERE team_id = $2")
            .bind(survivor_team)
            .bind(dup_team)
            .execute(&mut *tx)
            .await?
            .rows_affected();

    // 5. Assetek (entity_id itt a team id): azonos (kind,url) párok eldobása.
    let assets_dropped = sqlx::query(
        "DELETE FROM ops.asset a
          WHERE a.entity_kind = 'team' AND a.entity_id = $2
            AND EXISTS (SELECT 1 FROM ops.asset b
                         WHERE b.entity_kind = 'team' AND b.entity_id = $1
                           AND b.kind = a.kind AND b.url = a.url)",
    )
    .bind(survivor_team)
    .bind(dup_team)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let assets_moved = sqlx::query(
        "UPDATE ops.asset SET entity_id = $1 WHERE entity_kind = 'team' AND entity_id = $2",
    )
    .bind(survivor_team)
    .bind(dup_team)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // 6. Aliasok: azonos (alias, lang, kontextus) párok eldobása, majd átmozgatás.
    let aliases_dropped = sqlx::query(
        "DELETE FROM identity.alias a
          WHERE a.entity_id = $2
            AND EXISTS (SELECT 1 FROM identity.alias s
                         WHERE s.entity_id = $1 AND s.alias = a.alias AND s.lang = a.lang
                           AND s.country_code IS NOT DISTINCT FROM a.country_code
                           AND s.gender IS NOT DISTINCT FROM a.gender
                           AND s.age_group IS NOT DISTINCT FROM a.age_group
                           AND s.branch IS NOT DISTINCT FROM a.branch)",
    )
    .bind(survivor_entity)
    .bind(duplicate_entity)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let aliases_moved =
        sqlx::query("UPDATE identity.alias SET entity_id = $1 WHERE entity_id = $2")
            .bind(survivor_entity)
            .bind(duplicate_entity)
            .execute(&mut *tx)
            .await?
            .rows_affected();

    // 7. Forrás-refek átmozgatása (nincs FK; a linker kulcsa).
    let refs_moved =
        sqlx::query("UPDATE identity.source_entity_ref SET entity_id = $1 WHERE entity_id = $2")
            .bind(survivor_entity)
            .bind(duplicate_entity)
            .execute(&mut *tx)
            .await?
            .rows_affected();

    // 8. A duplikált team-sor törlése, az entitás lenyomat-megjelölése.
    sqlx::query("DELETE FROM core.team WHERE id = $1")
        .bind(dup_team)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE core.entity SET merged_into_id = $1 WHERE id = $2")
        .bind(survivor_entity)
        .bind(duplicate_entity)
        .execute(&mut *tx)
        .await?;

    // 9. A merge után keletkezett duplikált meccsek száma (a takarítás a
    //    meccs-dedupe eszközzel történik).
    let duplicate_matches: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
            SELECT 1 FROM core.match
             WHERE home_team_id = $1 OR away_team_id = $1
             GROUP BY competition_id, season_id, home_team_id, away_team_id, kickoff_at
            HAVING count(*) > 1
         ) d",
    )
    .bind(survivor_team)
    .fetch_one(&mut *tx)
    .await?;

    // 10. Audit.
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, 'cli', 'team_merge', 'team', $2, $3)",
    )
    .bind(Uuid::now_v7())
    .bind(survivor_entity)
    .bind(serde_json::json!({
        "duplicate_entity_id": duplicate_entity,
        "duplicate_name": dup_name,
        "survivor_name": survivor_name,
        "note": note,
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(TeamMergeReport {
        matches_moved,
        aliases_moved,
        aliases_dropped,
        refs_moved,
        standings_moved,
        standings_dropped,
        season_stats_moved,
        season_stats_dropped,
        assets_moved,
        assets_dropped,
        duplicate_matches,
    })
}

/// Betölti és ellenőrzi a párost: azonos kind, azonos normalizált név,
/// azonos kontextus, egyik sem merge-ölt. Visszaadja a team-id-ket és neveket.
async fn load_pair(
    conn: &mut sqlx::PgConnection,
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
) -> Result<(Uuid, String, Uuid, String)> {
    type EntityRow = (Uuid, String, String, Option<Uuid>); // id, név, kind, merged_into
    type TeamRow = (
        Uuid,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    const ENTITY_SQL: &str =
        "SELECT id, canonical_name, kind, merged_into_id FROM core.entity WHERE id = $1";
    const TEAM_SQL: &str = "SELECT t.id, c.code, t.gender, t.age_group, t.branch
         FROM core.team t
         LEFT JOIN core.country c ON c.id = t.country_id
         WHERE t.entity_id = $1";

    if survivor_entity == duplicate_entity {
        bail!("a túlélő és az áldozat entitás azonos");
    }
    let survivor: EntityRow = sqlx::query_as(ENTITY_SQL)
        .bind(survivor_entity)
        .fetch_optional(&mut *conn)
        .await
        .context("túlélő entitás lekérés")?
        .with_context(|| format!("nincs entitás: {survivor_entity}"))?;
    let duplicate: EntityRow = sqlx::query_as(ENTITY_SQL)
        .bind(duplicate_entity)
        .fetch_optional(&mut *conn)
        .await
        .context("áldozat entitás lekérés")?
        .with_context(|| format!("nincs entitás: {duplicate_entity}"))?;

    if survivor.2 != "team" || duplicate.2 != "team" {
        bail!(
            "a merge csak team kind-ra megy (túlélő: {}, áldozat: {})",
            survivor.2,
            duplicate.2
        );
    }
    if survivor.3.is_some() || duplicate.3.is_some() {
        bail!("valamelyik entitás már merge-ölt");
    }
    if survivor.1.trim().to_lowercase() != duplicate.1.trim().to_lowercase() {
        bail!(
            "eltérő név ({} vs {}): a merge csak azonos nevű duplikátumra megy",
            survivor.1,
            duplicate.1
        );
    }

    let survivor_team: TeamRow = sqlx::query_as(TEAM_SQL)
        .bind(survivor_entity)
        .fetch_optional(&mut *conn)
        .await
        .context("túlélő team lekérés")?
        .with_context(|| format!("nincs csapat-sor a túlélőhöz: {survivor_entity}"))?;
    let dup_team: TeamRow = sqlx::query_as(TEAM_SQL)
        .bind(duplicate_entity)
        .fetch_optional(&mut *conn)
        .await
        .context("áldozat team lekérés")?
        .with_context(|| format!("nincs csapat-sor az áldozathoz: {duplicate_entity}"))?;

    let same_context = survivor_team.1 == dup_team.1
        && survivor_team.2 == dup_team.2
        && survivor_team.3 == dup_team.3
        && survivor_team.4 == dup_team.4;
    if !same_context {
        bail!(
            "eltérő kontextus (ország/nem/korosztály/ág): {:?} vs {:?}",
            (
                &survivor_team.1,
                &survivor_team.2,
                &survivor_team.3,
                &survivor_team.4
            ),
            (&dup_team.1, &dup_team.2, &dup_team.3, &dup_team.4)
        );
    }

    Ok((survivor_team.0, survivor.1, dup_team.0, duplicate.1))
}
