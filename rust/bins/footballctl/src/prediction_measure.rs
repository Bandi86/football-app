//! S196/S199: offline mérési snapshot — piaci oddsra vagy rögzített shadow
//! predikciókra. Csak olvasott kanonikus adat; `--apply` nélkül nem ír.
//!
//! A mérési szerződés része: Wilson-intervallum a találati arányra és
//! klaszterezett bootstrap a páros Brier-különbséghez (versenyenként).

use anyhow::{Context, Result};
use chrono::NaiveDate;
use football_prediction::{as_of, margin, metrics};
use football_storage::repositories::analytics::{self, NewMeasurement};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashMap;

const MARKET_MODEL: &str = "market-shin-v1";
const MARKET: &str = "1x2";

#[derive(Default)]
struct Accumulator {
    paired: Vec<metrics::PairedSample>,
    brier_sum: f64,
    baseline_sum: f64,
    market_sum: f64,
    market_count: usize,
    log_loss_sum: f64,
    hits: usize,
    excluded: usize,
}

impl Accumulator {
    fn add(
        &mut self,
        probabilities: [f64; 3],
        market: Option<[f64; 3]>,
        actual: usize,
        cluster: u64,
    ) {
        let Some(brier) = metrics::brier(&probabilities, actual, Some(3.0)) else {
            self.excluded += 1;
            return;
        };
        let Some(ll) = metrics::log_loss(&probabilities, actual) else {
            self.excluded += 1;
            return;
        };
        let baseline = [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];
        let Some(baseline_brier) = metrics::brier(&baseline, actual, Some(3.0)) else {
            self.excluded += 1;
            return;
        };
        if let Some(market) = market
            && let Some(mb) = metrics::brier(&market, actual, Some(3.0))
        {
            self.market_sum += mb;
            self.market_count += 1;
        }
        let best = probabilities
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if probabilities[actual] == best {
            self.hits += 1;
        }
        self.paired.push(metrics::PairedSample {
            cluster,
            brier_a: brier,
            brier_b: baseline_brier,
        });
        self.brier_sum += brier;
        self.baseline_sum += baseline_brier;
        self.log_loss_sum += ll;
    }
}

fn outcome(home: i16, away: i16) -> usize {
    if home > away {
        0
    } else if home == away {
        1
    } else {
        2
    }
}

fn cluster_of(id: uuid::Uuid) -> u64 {
    (id.as_u128() % (u64::MAX as u128)) as u64
}

pub async fn run(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
    model_version: Option<&str>,
    apply: bool,
) -> Result<()> {
    let model_version = model_version.unwrap_or(MARKET_MODEL).to_owned();
    let mut acc = Accumulator::default();

    if model_version == MARKET_MODEL {
        // A piac maga mint „modell”: Shin-féle de-vigelés az as-of 1X2-re.
        let samples = analytics::market_odds_samples(pool, from, to).await?;
        for sample in &samples {
            if !as_of::is_as_of(sample.captured_at, sample.kickoff_at) {
                acc.excluded += 1;
                continue;
            }
            let Some(p) =
                margin::shin_probabilities(&[sample.home_odds, sample.draw_odds, sample.away_odds])
            else {
                acc.excluded += 1;
                continue;
            };
            acc.add(
                [p[0], p[1], p[2]],
                Some([p[0], p[1], p[2]]),
                outcome(sample.home_ft, sample.away_ft),
                cluster_of(sample.competition_id),
            );
        }
    } else {
        // Rögzített (backtest vagy shadow) predikciók vs. eredmények.
        let rows = analytics::predicted_outcomes(pool, &model_version, from, to).await?;
        let mut market_by_match: HashMap<uuid::Uuid, [f64; 3]> = HashMap::new();
        for sample in &analytics::market_odds_samples(pool, from, to).await? {
            if let Some(p) =
                margin::shin_probabilities(&[sample.home_odds, sample.draw_odds, sample.away_odds])
            {
                market_by_match.insert(sample.match_id, [p[0], p[1], p[2]]);
            }
        }
        for row in &rows {
            acc.add(
                [row.home_prob, row.draw_prob, row.away_prob],
                market_by_match.get(&row.match_id).copied(),
                outcome(row.home_ft, row.away_ft),
                cluster_of(row.competition_id),
            );
        }
    }
    if acc.paired.is_empty() {
        println!("Nincs mérhető minta a(z) {from} – {to} ablakban (modell: {model_version}).");
        return Ok(());
    }
    let n = acc.paired.len();
    let brier = acc.brier_sum / n as f64;
    let baseline = acc.baseline_sum / n as f64;
    let market_brier = if acc.market_count > 0 {
        acc.market_sum / acc.market_count as f64
    } else {
        brier
    };
    let log_loss = acc.log_loss_sum / n as f64;
    let bootstrap = metrics::cluster_bootstrap_difference(&acc.paired, 2000, 42)
        .context("klaszterezett bootstrap")?;
    let (wilson_low, wilson_high) =
        metrics::wilson_interval(acc.hits, n, 1.96).context("Wilson-intervallum")?;

    let report = serde_json::json!({
        "model_version": model_version,
        "market": MARKET,
        "from": from,
        "to": to,
        "sample_size": n,
        "excluded": acc.excluded,
        "brier_per_3": brier,
        "brier_baseline": baseline,
        "brier_market": market_brier,
        "difference": brier - baseline,
        "ci_low": bootstrap.ci_low,
        "ci_high": bootstrap.ci_high,
        "log_loss": log_loss,
        "top1_hits": acc.hits,
        "wilson_low": wilson_low,
        "wilson_high": wilson_high,
    });
    let inputs_hash = hex::encode(Sha256::digest(serde_json::to_vec(&report)?));

    println!(
        "Predikció mérés [{from} – {to}] modell={model_version} piac={MARKET}\n\
         minta={n} (kiszűrve: {})\n\
         Brier/3={brier:.4}  bázis={baseline:.4}  piac={market_brier:.4}  \
         különbség={:.4}  bootstrap CI=[{:.4}, {:.4}]\n\
         log-loss={log_loss:.4}  top-1 találat={}/{n}  Wilson CI=[{wilson_low:.4}, {wilson_high:.4}]\n\
         inputs_hash={inputs_hash}",
        acc.excluded,
        brier - baseline,
        bootstrap.ci_low,
        bootstrap.ci_high,
        acc.hits
    );

    if apply {
        let id = analytics::upsert_measurement(
            pool,
            &NewMeasurement {
                model_version: model_version.clone(),
                date_from: from,
                date_to: to,
                market: MARKET,
                sample_size: n as i32,
                brier,
                brier_baseline: baseline,
                brier_market: market_brier,
                difference: brier - baseline,
                ci_low: bootstrap.ci_low,
                ci_high: bootstrap.ci_high,
                log_loss,
                wilson_low,
                wilson_high,
                inputs_hash,
            },
        )
        .await?;
        println!("Mérés rögzítve: analytics.measurement id={id}");
    } else {
        println!("Dry-run — a mérés nem került rögzítésre (--apply).");
    }
    Ok(())
}
