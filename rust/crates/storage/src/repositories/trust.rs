//! S2b-2 bizalmi réteg storage repository-k.
//!
//! Forrás-trust upsert és lekérés domainenként, meccs-oddsok betöltése
//! forrásonként provider-rel, valamint az odds trust_state tömeges frissítése
//! a gate utáni kiértékeléshez. A `load_match_odds` fail-closed: hiányzó odds
//! mező hibát ad (nem nullát) — a hiányzó adat NULL elve szerint a betöltés
//! nem egészíthet ki csendben.

use anyhow::{Context, Result};
use football_domain::trust::{OddsTrustState, OddsValues, SourceOdds, SourceTrust, TrustClass};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Forrás-trust sor upsert a (source_id, domain) kulcsra. Ütközéskor az
/// osztály és a provider frissül; a created_at marad.
pub async fn upsert_trust(
    pool: &PgPool,
    source_id: Uuid,
    domain: &str,
    trust_class: TrustClass,
    provider: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO ops.source_trust (source_id, domain, trust_class, provider)
         VALUES ($1,$2,$3,$4)
         ON CONFLICT (source_id, domain) DO UPDATE SET
           trust_class = EXCLUDED.trust_class,
           provider = EXCLUDED.provider",
    )
    .bind(source_id)
    .bind(domain)
    .bind(trust_class.as_db_str())
    .bind(provider)
    .execute(pool)
    .await
    .context("forrás trust upsert")?;
    Ok(())
}

/// A megadott forráskódok trust sorai domainenként (JOIN ops.source a kódon).
/// Hiányzó trust sor nem hiba: a hívó (verification context) medium defaultot
/// alkalmaz; itt csak a ténylegesen tárolt sorok jönnek vissza.
pub async fn trust_for_sources(pool: &PgPool, source_codes: &[String]) -> Result<Vec<SourceTrust>> {
    let rows = sqlx::query(
        "SELECT s.code AS source_code, t.domain, t.trust_class, t.provider
         FROM ops.source_trust t
         JOIN ops.source s ON s.id = t.source_id
         WHERE s.code = ANY($1)
         ORDER BY s.code, t.domain",
    )
    .bind(source_codes)
    .fetch_all(pool)
    .await
    .context("forrás trust lekérés")?;
    rows.iter()
        .map(|row| {
            let class: String = row.try_get("trust_class").context("trust_class oszlop")?;
            Ok(SourceTrust {
                source_code: row.try_get("source_code").context("source_code oszlop")?,
                domain: row.try_get("domain").context("domain oszlop")?,
                trust_class: TrustClass::from_db_str(&class)
                    .map_err(anyhow::Error::msg)
                    .context("trust osztály értelmezés")?,
                provider: row.try_get("provider").context("provider oszlop")?,
            })
        })
        .collect()
}

/// Egy meccs odds_snapshot sorai forrásonként, az odds domainű trust sor
/// providerével (LEFT JOIN: trust sor nélkül provider = None). Az odds jsonb
/// mind a 7 mezőt tartalmaznia kell; hiányzó mező hiba (fail-closed).
pub async fn load_match_odds(pool: &PgPool, match_id: Uuid) -> Result<Vec<SourceOdds>> {
    let rows = sqlx::query(
        "SELECT s.code AS source_code, t.provider, o.odds
         FROM fact.odds_snapshot o
         JOIN ops.source s ON s.id = o.source_id
         LEFT JOIN ops.source_trust t
           ON t.source_id = o.source_id AND t.domain = 'odds'
         WHERE o.match_id = $1
         ORDER BY s.code",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .context("meccs odds lekérés")?;
    rows.iter()
        .map(|row| {
            let v: serde_json::Value = row.try_get("odds").context("odds oszlop")?;
            let field = |k: &str| -> Result<f64> {
                v[k].as_f64()
                    .ok_or_else(|| anyhow::anyhow!("hiányzó odds mező: {k}"))
            };
            Ok(SourceOdds {
                source_code: row.try_get("source_code").context("source_code oszlop")?,
                provider: row.try_get("provider").context("provider oszlop")?,
                odds: OddsValues {
                    home: field("home")?,
                    draw: field("draw")?,
                    away: field("away")?,
                    btts_yes: field("btts_yes")?,
                    btts_no: field("btts_no")?,
                    over25: field("over25")?,
                    under25: field("under25")?,
                },
            })
        })
        .collect()
}

/// A meccs ÖSSZES odds sorának trust_state frissítése (a kiértékelés meccs
/// szintű). Visszatérés: érintett sorok száma (0 = nincs odds sor, nem hiba).
pub async fn update_odds_trust_state(
    pool: &PgPool,
    match_id: Uuid,
    state: OddsTrustState,
) -> Result<u64> {
    let res = sqlx::query("UPDATE fact.odds_snapshot SET trust_state = $2 WHERE match_id = $1")
        .bind(match_id)
        .bind(state.as_db_str())
        .execute(pool)
        .await
        .context("odds trust állapot frissítés")?;
    Ok(res.rows_affected())
}
