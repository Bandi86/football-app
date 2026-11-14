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
    pub async fn materialize(pool: &PgPool, day: &NormalizedDay) -> Result<MaterializedDay> {
        let mut tx = pool.begin().await?;

        let country_id = upsert_country(
            &mut tx,
            &day.competition.country_code,
            &day.competition.country_name,
        )
        .await?;
        let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
            .bind(&day.source_code)
            .fetch_one(&mut *tx)
            .await
            .with_context(|| format!("ismeretlen forrás: {}", day.source_code))?;

        // Élő módban nincs id-hint: natural-key find-or-create.
        let competition_id = match day.competition.id {
            Some(id) => id,
            None => sqlx::query_scalar(
                "SELECT id FROM core.competition WHERE country_id=$1 AND name=$2 LIMIT 1",
            )
            .bind(country_id)
            .bind(&day.competition.name)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or_else(Uuid::now_v7),
        };
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

        upsert_entity(
            &mut tx,
            competition_id,
            "competition",
            &day.competition.name,
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
        .bind(competition_id)
        .bind(country_id)
        .bind(&day.competition.name)
        .bind(day.competition.tier)
        .bind(day.competition.priority)
        .execute(&mut *tx)
        .await?;

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
                    upsert_team(&mut tx, team, country_id, day.source_code.as_str()).await?;
                    team_ids.insert(
                        team.source_entity_id.clone(),
                        team.id.expect("hint jelen van"),
                    );
                }
                // Élő út: a fail-closed linker oldja fel; nem egzakt → kimarad.
                None => {
                    let ctx = football_domain::identity::IdentityContext {
                        country_code: Some(team.country_code.clone()),
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
                            ensure_live_team(&mut tx, entity_id, &team.name, country_id).await?;
                            team_ids.insert(team.source_entity_id.clone(), entity_id);
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
            let match_id = match fixture_match.id {
                Some(id) => id,
                None => {
                    find_or_insert_match_id(
                        &mut tx,
                        competition_id,
                        season_id,
                        fixture_match,
                        home,
                        away,
                    )
                    .await?
                }
            };
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
            .bind(match_id)
            .bind(competition_id)
            .bind(season_id)
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

            persist_result(&mut tx, match_id, result.as_ref()).await?;

            let observation_ids =
                observations_for_match(&mut tx, &day.source_code, &fixture_match.source_match_id)
                    .await?;

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
                write_odds(&mut tx, match_id, source_id, day.captured_at, odds).await?;
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
    team: &NormalizedTeam,
    country_id: Uuid,
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
           country_id = EXCLUDED.country_id,
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

/// Élő csapat: a linkeres Exact entitáshoz létezzen core.team sor
/// (az invariáns: core.team.id == entity.id); hiányzó sor a forrás nevével
/// jön létre.
async fn ensure_live_team(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: Uuid,
    name: &str,
    country_id: Uuid,
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

/// Élő meccs natural-key keresése (NULL-safe round_label), különben új id.
async fn find_or_insert_match_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    competition_id: Uuid,
    season_id: Uuid,
    m: &NormalizedMatch,
    home: Uuid,
    away: Uuid,
) -> Result<Uuid> {
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.match
         WHERE competition_id=$1 AND season_id=$2
           AND round_label IS NOT DISTINCT FROM $3
           AND home_team_id=$4 AND away_team_id=$5 AND kickoff_at=$6
         LIMIT 1",
    )
    .bind(competition_id)
    .bind(season_id)
    .bind(&m.round_label)
    .bind(home)
    .bind(away)
    .bind(m.kickoff_at)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(existing.unwrap_or_else(Uuid::now_v7))
}

/// Odds-fejléc upsert + a forrás quote-blokkjának teljes cseréje
/// (a friss/új blokk mindig provisional — a gate utáni kiértékelés zárja).
async fn write_odds(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    source_id: Uuid,
    captured_at: chrono::DateTime<Utc>,
    odds: &crate::normalized::NormalizedOdds,
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
        sqlx::query(
            "INSERT INTO fact.odds_quote
             (match_id, source_id, bookmaker, market, outcome, line, phase, price)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(&quote.bookmaker)
        .bind(&quote.market)
        .bind(&quote.outcome)
        .bind(quote.line)
        .bind(&quote.phase)
        .bind(quote.price)
        .execute(&mut **tx)
        .await?;
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
