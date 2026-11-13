use anyhow::{Context, Result};
use chrono::Utc;
use football_domain::{MatchResultSnapshot, MatchSnapshot, MatchStatus};
use sqlx::PgPool;
use uuid::Uuid;

use crate::fixture::{FixtureSource, FixtureTeam};

pub struct Canonicalizer;

#[derive(Debug, Clone)]
pub struct MaterializedMatch {
    pub match_id: Uuid,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub snapshot: MatchSnapshot,
    pub observation_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct MaterializedDay {
    pub matches: Vec<MaterializedMatch>,
}

impl Canonicalizer {
    pub async fn materialize(pool: &PgPool, fixture: &FixtureSource) -> Result<MaterializedDay> {
        let mut tx = pool.begin().await?;

        let country_id = upsert_country(
            &mut tx,
            &fixture.competition.country_code,
            &fixture.competition.country_name,
        )
        .await?;

        upsert_entity(
            &mut tx,
            fixture.competition.id,
            "competition",
            &fixture.competition.name,
        )
        .await?;

        sqlx::query(
            "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
             VALUES ($1,$1,$2,$3,$4,$5)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               tier = EXCLUDED.tier,
               priority = EXCLUDED.priority",
        )
        .bind(fixture.competition.id)
        .bind(country_id)
        .bind(&fixture.competition.name)
        .bind(fixture.competition.tier)
        .bind(fixture.competition.priority)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO core.season (id, competition_id, label)
             VALUES ($1,$2,$3)
             ON CONFLICT (id) DO UPDATE SET label = EXCLUDED.label",
        )
        .bind(fixture.season.id)
        .bind(fixture.competition.id)
        .bind(&fixture.season.label)
        .execute(&mut *tx)
        .await?;

        for team in &fixture.teams {
            upsert_team(&mut tx, team, country_id, fixture.source_code.as_str()).await?;
        }

        let mut matches = Vec::new();

        for fixture_match in &fixture.matches {
            let home = team_by_source_id(&mut tx, &fixture_match.home_source_entity_id).await?;
            let away = team_by_source_id(&mut tx, &fixture_match.away_source_entity_id).await?;
            let status = parse_status(&fixture_match.status)?;

            sqlx::query(
                "INSERT INTO core.match
                 (id, competition_id, season_id, round_label, home_team_id, away_team_id,
                  kickoff_at, competition_timezone, status)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT (id) DO UPDATE SET
                   round_label = EXCLUDED.round_label,
                   home_team_id = EXCLUDED.home_team_id,
                   away_team_id = EXCLUDED.away_team_id,
                   kickoff_at = EXCLUDED.kickoff_at,
                   status = EXCLUDED.status,
                   updated_at = now()",
            )
            .bind(fixture_match.id)
            .bind(fixture.competition.id)
            .bind(fixture.season.id)
            .bind(&fixture_match.round_label)
            .bind(home)
            .bind(away)
            .bind(fixture_match.kickoff_at)
            .bind(&fixture_match.competition_timezone)
            .bind(status_db(status))
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

            persist_result(&mut tx, fixture_match.id, result.as_ref()).await?;

            let observation_ids =
                observations_for_match(&mut tx, &fixture.source_code, fixture_match.id).await?;

            for field in [
                "home_team_id",
                "away_team_id",
                "kickoff_at",
                "status",
                "result",
            ] {
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
                    .bind(fixture_match.id)
                    .bind(field)
                    .bind(observation_id)
                    .bind(Utc::now())
                    .execute(&mut *tx)
                    .await?;
                }
            }

            matches.push(MaterializedMatch {
                match_id: fixture_match.id,
                home_team_id: home,
                away_team_id: away,
                snapshot: MatchSnapshot {
                    id: fixture_match.id,
                    home_team_id: home,
                    away_team_id: away,
                    kickoff_at: fixture_match.kickoff_at,
                    status,
                    result,
                },
                observation_ids,
            });
        }

        let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
            .bind(&fixture.source_code)
            .fetch_one(&mut *tx)
            .await?;

        for fixture_match in &fixture.matches {
            let Some(odds) = &fixture_match.odds else {
                continue;
            };
            let market_count = fixture_match
                .market_count
                .with_context(|| format!("hiányzó piacszám az odds mellé: {}", fixture_match.id))?;
            // Új vagy frissülő odds sor mindig provisional: a verified
            // állapotot a gate utáni odds-kiértékelés állítja be (S2b-2).
            sqlx::query(
                "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, odds)
                 VALUES ($1,$2,$3,$4,$5)
                 ON CONFLICT (match_id, source_id) DO UPDATE SET
                   captured_at = EXCLUDED.captured_at,
                   market_count = EXCLUDED.market_count,
                   odds = EXCLUDED.odds,
                   trust_state = 'provisional'",
            )
            .bind(fixture_match.id)
            .bind(source_id)
            .bind(fixture.captured_at)
            .bind(market_count)
            .bind(serde_json::to_value(odds)?)
            .execute(&mut *tx)
            .await?;
        }

        if let Some(standings) = &fixture.standings {
            for standing in standings {
                let team_id = team_by_source_id(&mut tx, &standing.team_source_entity_id).await?;
                sqlx::query(
                    "INSERT INTO core.standing (season_id, team_id, position, source_id, captured_at)
                     VALUES ($1,$2,$3,$4,$5)
                     ON CONFLICT (season_id, team_id) DO UPDATE SET
                       position = EXCLUDED.position,
                       source_id = EXCLUDED.source_id,
                       captured_at = EXCLUDED.captured_at",
                )
                .bind(fixture.season.id)
                .bind(team_id)
                .bind(standing.position)
                .bind(source_id)
                .bind(fixture.captured_at)
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
    sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name)
         VALUES ($1,$2,$3)
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(code)
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

async fn upsert_team(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team: &FixtureTeam,
    country_id: Uuid,
    source_code: &str,
) -> Result<()> {
    upsert_entity(tx, team.id, "team", &team.name).await?;

    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,$4,$5,$6)
         ON CONFLICT (id) DO UPDATE SET
           name = EXCLUDED.name,
           country_id = EXCLUDED.country_id,
           gender = EXCLUDED.gender,
           age_group = EXCLUDED.age_group,
           branch = EXCLUDED.branch",
    )
    .bind(team.id)
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
    .bind(team.id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn team_by_source_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_entity_id: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(
        "SELECT entity_id FROM identity.source_entity_ref
         WHERE entity_kind='team' AND source_entity_id=$1 AND status='active'",
    )
    .bind(source_entity_id)
    .fetch_one(&mut **tx)
    .await
    .with_context(|| format!("ismeretlen csapatforrás: {source_entity_id}"))
}

async fn persist_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    result: Option<&MatchResultSnapshot>,
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
         (match_id, home_ft, away_ft, home_ht, away_ht, home_et, away_et, home_pens, away_pens)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         ON CONFLICT (match_id) DO UPDATE SET
           home_ft=EXCLUDED.home_ft, away_ft=EXCLUDED.away_ft,
           home_ht=EXCLUDED.home_ht, away_ht=EXCLUDED.away_ht,
           home_et=EXCLUDED.home_et, away_et=EXCLUDED.away_et,
           home_pens=EXCLUDED.home_pens, away_pens=EXCLUDED.away_pens",
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
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn observations_for_match(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source_code: &str,
    match_id: Uuid,
) -> Result<Vec<Uuid>> {
    let rows = sqlx::query_scalar::<_, Uuid>(
        "SELECT o.id
         FROM stage.observation o
         JOIN ops.source s ON s.id = o.source_id
         WHERE s.code = $1 AND o.entity_kind = 'match'
           AND o.aspect = 'match_core' AND o.natural_key = $2
         ORDER BY o.observed_at DESC",
    )
    .bind(source_code)
    .bind(match_id.to_string())
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows)
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
