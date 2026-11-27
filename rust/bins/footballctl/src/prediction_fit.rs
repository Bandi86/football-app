//! S198: shadow predikciók as-of paraméterekből — csak olvasott kanonikus adat,
//! `--apply` nélkül nem ír. A modell elfogadása a P0 mérési szerződés szerint
//! külön lépés; ez a parancs nem ad fogadási tanácsot.

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use football_prediction::{fit, goals};
use football_storage::repositories::analytics::{self, NewPrediction, ScoredMatchRow};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

const MODEL_VERSION: &str = "dixon-coles-eb-v1";

pub async fn run(pool: &PgPool, date: NaiveDate, apply: bool) -> Result<()> {
    let cutoff = Utc::now();
    let history = analytics::scored_matches_before(pool, cutoff, 365).await?;
    let upcoming = analytics::upcoming_matches(pool, date, cutoff).await?;
    if upcoming.is_empty() {
        println!("Nincs jövőbeli meccs a(z) {date} napon.");
        return Ok(());
    }
    println!(
        "Predikciók [{date}] modell={MODEL_VERSION} as-of={} history={} upcoming={}",
        cutoff.to_rfc3339(),
        history.len(),
        upcoming.len()
    );

    let mut written = 0usize;
    for match_row in &upcoming {
        let scored: Vec<fit::ScoredMatch> = history
            .iter()
            .map(|m: &ScoredMatchRow| fit::ScoredMatch {
                kickoff_at: m.kickoff_at,
                home: m.home,
                away: m.away,
                home_goals: m.home_goals,
                away_goals: m.away_goals,
            })
            .collect();
        let Some(params) = fit::fit_lambdas(
            match_row.home,
            match_row.away,
            cutoff,
            &scored,
            fit::V1_HALF_LIFE_DAYS,
            fit::V1_K,
        ) else {
            println!("  {} — nincs elég as-of adat, kimarad", match_row.id);
            continue;
        };
        let Some(matrix) = goals::goal_matrix(params.lambda_home, params.lambda_away, -0.05, 14)
        else {
            continue;
        };
        let Some(markets) = goals::market_probabilities(&matrix) else {
            continue;
        };
        let odds = goals::fair_odds_for_markets(&markets).context("fair odds")?;
        let report = serde_json::json!({
            "model": MODEL_VERSION,
            "match_id": match_row.id,
            "as_of": cutoff,
            "lambda_home": params.lambda_home,
            "lambda_away": params.lambda_away,
            "home": markets.home,
            "draw": markets.draw,
            "away": markets.away,
        });
        let hash = hex::encode(Sha256::digest(serde_json::to_vec(&report)?));
        println!(
            "  {} λ={:.3}/{:.3} 1X2={:.3}/{:.3}/{:.3} fair={:.2}/{:.2}/{:.2}",
            match_row.id,
            params.lambda_home,
            params.lambda_away,
            markets.home,
            markets.draw,
            markets.away,
            odds[0],
            odds[1],
            odds[2]
        );
        if apply {
            let outcomes = [
                ("home", markets.home, odds[0], 0.0),
                ("draw", markets.draw, odds[1], 0.0),
                ("away", markets.away, odds[2], 0.0),
            ];
            written += analytics::upsert_prediction(
                pool,
                &NewPrediction {
                    match_id: match_row.id,
                    model_version: MODEL_VERSION,
                    outcomes: outcomes.to_vec(),
                    quality: "ok",
                    inputs_hash: hash.clone(),
                    as_of: cutoff,
                },
            )
            .await?;
        }
    }
    if apply {
        println!("Predikciók rögzítve: {written} sor (analytics.prediction).");
    } else {
        println!("Dry-run — egyik predikció sem került rögzítésre (--apply).");
    }
    Ok(())
}
