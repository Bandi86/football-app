//! S196: offline mérési snapshot a piaci 1X2 oddsokra.
//!
//! Csak olvasott kanonikus adat; az eredmény alapértelmezés szerint **nem ír**.
//! `--apply` esetén egy idempotens `analytics.measurement` sort rögzít,
//! modellverzióval, as-of hash-sel és konfidenciaintervallumokkal.
//! Ez még nem predikciós modell: a „modell" itt a Shin-féle de-vigeléses piac,
//! a bázis az egyenletes 1/3–1/3–1/3 — a mérési gépezetet és a piac
//! erősségét igazolja a későbbi Dixon–Coles modell elé.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use football_prediction::{as_of, margin, metrics};
use football_storage::repositories::analytics::{self, NewMeasurement};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

const MODEL_VERSION: &str = "market-shin-v1";
const MARKET: &str = "1x2";

pub async fn run(pool: &PgPool, from: NaiveDate, to: NaiveDate, apply: bool) -> Result<()> {
    let samples = analytics::market_odds_samples(pool, from, to).await?;
    if samples.is_empty() {
        println!("Nincs as-of 1X2 minta a(z) {from} – {to} ablakban.");
        return Ok(());
    }

    let mut paired = Vec::new();
    let mut brier_sum = 0.0;
    let mut baseline_sum = 0.0;
    let mut log_loss_sum = 0.0;
    let mut hits = 0usize;
    let mut excluded = 0usize;

    for sample in &samples {
        // Szigorú as-of: a legfrissebb odds is csak a kickoff előtt használható.
        if !as_of::is_as_of(sample.captured_at, sample.kickoff_at) {
            excluded += 1;
            continue;
        }
        let Some(probabilities) =
            margin::shin_probabilities(&[sample.home_odds, sample.draw_odds, sample.away_odds])
        else {
            excluded += 1;
            continue;
        };
        let actual = if sample.home_ft > sample.away_ft {
            0
        } else if sample.home_ft == sample.away_ft {
            1
        } else {
            2
        };
        let Some(brier) = metrics::brier(&probabilities, actual, Some(3.0)) else {
            excluded += 1;
            continue;
        };
        let Some(ll) = metrics::log_loss(&probabilities, actual) else {
            excluded += 1;
            continue;
        };
        let baseline = [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];
        let Some(baseline_brier) = metrics::brier(&baseline, actual, Some(3.0)) else {
            excluded += 1;
            continue;
        };
        if probabilities[actual]
            == probabilities
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max)
        {
            hits += 1;
        }
        paired.push(metrics::PairedSample {
            cluster: (sample.competition_id.as_u128() % (u64::MAX as u128)) as u64,
            brier_a: brier,
            brier_b: baseline_brier,
        });
        brier_sum += brier;
        baseline_sum += baseline_brier;
        log_loss_sum += ll;
    }
    if paired.is_empty() {
        println!("Minden minta kiszűrődött (hibás odds vagy as-of).");
        return Ok(());
    }
    let n = paired.len();
    let brier = brier_sum / n as f64;
    let baseline = baseline_sum / n as f64;
    let log_loss = log_loss_sum / n as f64;
    let bootstrap = metrics::cluster_bootstrap_difference(&paired, 2000, 42)
        .context("klaszterezett bootstrap")?;
    let (wilson_low, wilson_high) =
        metrics::wilson_interval(hits, n, 1.96).context("Wilson-intervallum")?;

    let report = serde_json::json!({
        "model_version": MODEL_VERSION,
        "market": MARKET,
        "from": from,
        "to": to,
        "sample_size": n,
        "excluded": excluded,
        "brier_per_3": brier,
        "brier_baseline": baseline,
        "difference": brier - baseline,
        "ci_low": bootstrap.ci_low,
        "ci_high": bootstrap.ci_high,
        "log_loss": log_loss,
        "top1_hits": hits,
        "wilson_low": wilson_low,
        "wilson_high": wilson_high,
    });
    let inputs_hash = hex::encode(Sha256::digest(serde_json::to_vec(&report)?));

    println!(
        "Predikció mérés [{from} – {to}] modell={MODEL_VERSION} piac={MARKET}\n\
         minta={n} (kiszűrve: {excluded})\n\
         Brier/3={brier:.4}  bázis={baseline:.4}  különbség={:.4}  bootstrap CI=[{:.4}, {:.4}]\n\
         log-loss={log_loss:.4}  top-1 találat={hits}/{n}  Wilson CI=[{wilson_low:.4}, {wilson_high:.4}]\n\
         inputs_hash={inputs_hash}",
        brier - baseline,
        bootstrap.ci_low,
        bootstrap.ci_high
    );

    if apply {
        let id = analytics::upsert_measurement(
            pool,
            &NewMeasurement {
                model_version: MODEL_VERSION,
                date_from: from,
                date_to: to,
                market: MARKET,
                sample_size: n as i32,
                brier,
                brier_baseline: baseline,
                brier_market: brier,
                difference: brier - baseline,
                ci_low: bootstrap.ci_low,
                ci_high: bootstrap.ci_high,
                log_loss,
                wilson_low,
                wilson_high,
                inputs_hash: inputs_hash.clone(),
            },
        )
        .await?;
        println!("Mérés rögzítve: analytics.measurement id={id}");
    } else {
        println!("Dry-run — a mérés nem került rögzítésre (--apply).");
    }
    Ok(())
}
