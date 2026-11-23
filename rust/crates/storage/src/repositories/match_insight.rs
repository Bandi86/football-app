//! A research meccsinsight tábla írása/olvasása.
//!
//! Egy meccshez egy sor tartozik (forrás: research). A hiányzó szekciók
//! JSONB-ben NULL-ok — a payloadból kivont mezők üres kulcsa szintén NULL.

use anyhow::Result;
use serde_json::Value;
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn upsert<'e, E>(
    executor: E,
    match_id: Uuid,
    source_id: Uuid,
    captured_at: chrono::DateTime<chrono::Utc>,
    payload: &Value,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO fact.match_insight
           (match_id, source_id, captured_at, preview_hu, prediction, h2h_streaks,
            standings, season_stats, injuries, fan_poll, odds_markets, venue_details)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
         ON CONFLICT (match_id, source_id) DO UPDATE SET
           captured_at=EXCLUDED.captured_at,
           preview_hu=EXCLUDED.preview_hu, prediction=EXCLUDED.prediction,
           h2h_streaks=EXCLUDED.h2h_streaks, standings=EXCLUDED.standings,
           season_stats=EXCLUDED.season_stats, injuries=EXCLUDED.injuries,
           fan_poll=EXCLUDED.fan_poll, odds_markets=EXCLUDED.odds_markets,
           venue_details=EXCLUDED.venue_details",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(captured_at)
    .bind(payload.get("preview_hu").cloned())
    .bind(payload.get("prediction").cloned())
    .bind(payload.get("h2h_streaks").cloned())
    .bind(payload.get("standings").cloned())
    .bind(payload.get("season_stats").cloned())
    .bind(payload.get("injuries").cloned())
    .bind(payload.get("fan_poll").cloned())
    .bind(payload.get("odds_markets").cloned())
    .bind(payload.get("venue_details").cloned())
    .execute(executor)
    .await?;
    Ok(())
}

/// A legfrissebb insight nyers JSON-ja a kliens API-nak (forrás + idővel).
pub async fn fetch_json(pool: &PgPool, match_id: Uuid) -> Result<Option<Value>> {
    sqlx::query_scalar::<_, Value>(
        "SELECT jsonb_build_object(
            'captured_at', mi.captured_at,
            'source', src.code,
            'preview_hu', mi.preview_hu,
            'prediction', mi.prediction,
            'h2h_streaks', mi.h2h_streaks,
            'standings', mi.standings,
            'season_stats', mi.season_stats,
            'injuries', mi.injuries,
            'fan_poll', mi.fan_poll,
            'odds_markets', mi.odds_markets,
            'venue_details', mi.venue_details
         )
           FROM fact.match_insight mi
           JOIN ops.source src ON src.id = mi.source_id
          WHERE mi.match_id = $1
          ORDER BY mi.captured_at DESC
          LIMIT 1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}
