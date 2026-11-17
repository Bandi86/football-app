use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_domain::MatchStatus;
use sqlx::PgPool;
use uuid::Uuid;

use crate::engine::SourceMaterialValues;

/// A meccshez hozzájáruló `match_core` megfigyelések anyagi értékei.
///
/// A korábbi natural-key alapú lekérés a kanonikus meccs-azonosítót kereste,
/// de a megfigyelések a *forrás* rekord-kulcsát hordozzák — így a trust- és
/// konfliktus-szabály gyakorlatilag sosem látott forrásértéket. A mostani út
/// a provenance-sorból indul, a forrás csapatkódjait aktív refből kanonikus
/// csapat-id-re oldja, és a tükrözött (fordított oldalú) soroknál a
/// pontszámokat a kanonikus orientációra cseréli.
pub async fn load_material_values(
    pool: &PgPool,
    match_id: Uuid,
) -> Result<Vec<SourceMaterialValues>> {
    let (match_home, match_away): (Uuid, Uuid) =
        sqlx::query_as("SELECT home_team_id, away_team_id FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(pool)
            .await
            .context("meccs csapatainak lekérése a forrásértékekhez")?;

    let rows: Vec<(Uuid, String, serde_json::Value)> = sqlx::query_as(
        "SELECT DISTINCT ON (o.source_id) o.source_id, s.code, o.normalized_payload
         FROM fact.provenance p
         JOIN stage.observation o ON o.id = p.observation_id
         JOIN ops.source s ON s.id = o.source_id
         WHERE p.subject_kind = 'match' AND p.subject_id = $1
           AND o.entity_kind = 'match' AND o.aspect = 'match_core'
         ORDER BY o.source_id, o.observed_at DESC",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .context("meccs forrás-megfigyeléseinek lekérése")?;

    let mut values = Vec::with_capacity(rows.len());
    for (source_id, source_code, payload) in rows {
        let (Some(home_code), Some(away_code)) = (
            payload
                .get("source_home_id")
                .and_then(serde_json::Value::as_str),
            payload
                .get("source_away_id")
                .and_then(serde_json::Value::as_str),
        ) else {
            continue;
        };
        let kickoff_at = required_str(&payload, "kickoff_at")?
            .parse::<DateTime<Utc>>()
            .with_context(|| format!("forrás kickoff: {match_id}"))?;
        let status = match required_str(&payload, "status")? {
            "scheduled" => MatchStatus::Scheduled,
            "confirmed" => MatchStatus::Confirmed,
            "live" => MatchStatus::Live,
            "halftime" => MatchStatus::Halftime,
            "finished" => MatchStatus::Finished,
            "postponed" => MatchStatus::Postponed,
            "canceled" => MatchStatus::Canceled,
            "abandoned" => MatchStatus::Abandoned,
            "awarded" => MatchStatus::Awarded,
            other => anyhow::bail!("ismeretlen státusz: {other}"),
        };
        let (Some(home), Some(away)) = (
            resolve_team_ref(pool, source_id, home_code).await?,
            resolve_team_ref(pool, source_id, away_code).await?,
        ) else {
            // A forrás csapatkódja még nincs egyértelműen kötve: kihagyjuk,
            // a konfliktus-összevetéshez nem használható.
            continue;
        };
        let home_ft = payload["result"]["home_ft"].as_i64().map(|v| v as i16);
        let away_ft = payload["result"]["away_ft"].as_i64().map(|v| v as i16);
        // A forrás saját orientációja a kanonikushoz képest eltérhet
        // (lock-first + tükrözött lista) — a pontszámot igazítjuk.
        let (home_ft, away_ft) = if home == match_home && away == match_away {
            (home_ft, away_ft)
        } else if home == match_away && away == match_home {
            (away_ft, home_ft)
        } else {
            continue;
        };
        values.push(SourceMaterialValues {
            source_code,
            home_team_id: match_home,
            away_team_id: match_away,
            kickoff_at,
            status,
            home_ft,
            away_ft,
        });
    }
    Ok(values)
}

async fn resolve_team_ref(pool: &PgPool, source_id: Uuid, code: &str) -> Result<Option<Uuid>> {
    sqlx::query_scalar(
        "SELECT entity_id FROM identity.source_entity_ref
         WHERE source_id=$1 AND entity_kind='team' AND source_entity_id=$2
           AND status='active'",
    )
    .bind(source_id)
    .bind(code)
    .fetch_optional(pool)
    .await
    .context("forrás csapatkód feloldása")
}

fn required_str<'a>(payload: &'a serde_json::Value, field: &str) -> Result<&'a str> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("hiányzó mező: {field}"))
}
