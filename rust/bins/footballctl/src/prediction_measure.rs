//! S196/S199/S203: offline mérési snapshot — piaci oddsra vagy rögzített
//! shadow predikciókra (1X2, OU 2,5, BTTS). Csak olvasott kanonikus adat;
//! `--apply` nélkül nem ír.
//!
//! Mérési szerződés: Wilson-intervallum a találati arányra és klaszterezett
//! bootstrap a páros Brier-különbséghez (versenyenként, 2000 iteráció).

use anyhow::{Context, Result};
use chrono::NaiveDate;
use football_prediction::{as_of, margin, metrics};
use football_storage::repositories::analytics::{self, NewMeasurement};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap};

const MARKET_MODEL: &str = "market-shin-v1";

/// (meccs, verseny, [(kimenetel, valószínűség)], hazai gólok, vendég gólok)
type MarketSample = (uuid::Uuid, uuid::Uuid, Vec<(String, f64)>, i16, i16);

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
    fn add(&mut self, probabilities: &[f64], market: Option<&[f64]>, actual: usize, cluster: u64) {
        let Some(brier) = metrics::brier(probabilities, actual, Some(probabilities.len() as f64))
        else {
            self.excluded += 1;
            return;
        };
        let Some(ll) = metrics::log_loss(probabilities, actual) else {
            self.excluded += 1;
            return;
        };
        let baseline = vec![1.0 / probabilities.len() as f64; probabilities.len()];
        let Some(baseline_brier) = metrics::brier(&baseline, actual, Some(baseline.len() as f64))
        else {
            self.excluded += 1;
            return;
        };
        if let Some(market) = market
            && let Some(mb) = metrics::brier(market, actual, Some(market.len() as f64))
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

fn outcome_1x2(home: i16, away: i16) -> usize {
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

async fn report_and_store(
    pool: &PgPool,
    acc: Accumulator,
    model_version: &str,
    market: &str,
    from: NaiveDate,
    to: NaiveDate,
    apply: bool,
) -> Result<()> {
    if acc.paired.is_empty() {
        println!(
            "Nincs mérhető minta a(z) {from} – {to} ablakban (modell: {model_version}, piac: {market})."
        );
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
        "model_version": model_version, "market": market,
        "from": from, "to": to, "sample_size": n, "excluded": acc.excluded,
        "brier_per_outcome": brier, "brier_baseline": baseline,
        "brier_market": market_brier, "difference": brier - baseline,
        "ci_low": bootstrap.ci_low, "ci_high": bootstrap.ci_high,
        "log_loss": log_loss, "top1_hits": acc.hits,
        "wilson_low": wilson_low, "wilson_high": wilson_high,
    });
    let inputs_hash = hex::encode(Sha256::digest(serde_json::to_vec(&report)?));
    println!(
        "Mérés [{from} – {to}] modell={model_version} piac={market}\n\
         minta={n} (kiszűrve: {})  Brier={brier:.4}  bázis={baseline:.4}  piac={market_brier:.4}\n\
         különbség={:.4}  bootstrap CI=[{:.4}, {:.4}]  log-loss={log_loss:.4}\n\
         top-1={}/{n}  Wilson CI=[{wilson_low:.4}, {wilson_high:.4}]  hash={inputs_hash}",
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
                model_version: model_version.to_owned(),
                date_from: from,
                date_to: to,
                market: market.to_owned(),
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
    }
    Ok(())
}

pub async fn run(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
    model_version: Option<&str>,
    apply: bool,
) -> Result<()> {
    let model_version = model_version.unwrap_or(MARKET_MODEL).to_owned();

    if model_version == MARKET_MODEL {
        let mut acc = Accumulator::default();
        for sample in &analytics::market_odds_samples(pool, from, to).await? {
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
            let probs = [p[0], p[1], p[2]];
            acc.add(
                &probs,
                Some(&probs),
                outcome_1x2(sample.home_ft, sample.away_ft),
                cluster_of(sample.competition_id),
            );
        }
        report_and_store(pool, acc, &model_version, "1x2", from, to, apply).await?;
        return Ok(());
    }

    // Rögzített shadow predikciók: minden piac külön mérést kap.
    let mut market_by_match: HashMap<uuid::Uuid, Vec<f64>> = HashMap::new();
    for sample in &analytics::market_odds_samples(pool, from, to).await? {
        if let Some(p) =
            margin::shin_probabilities(&[sample.home_odds, sample.draw_odds, sample.away_odds])
        {
            market_by_match.insert(sample.match_id, p);
        }
    }
    let rows = analytics::prediction_market_samples(pool, &model_version, from, to).await?;
    let mut markets: BTreeMap<String, Vec<MarketSample>> = BTreeMap::new();
    for row in rows {
        markets.entry(row.market).or_default().push((
            row.match_id,
            row.competition_id,
            vec![(row.outcome, row.probability)],
            row.home_ft,
            row.away_ft,
        ));
    }
    // Ugyanazon meccs több kimenetelét összevonjuk.
    for samples in markets.values_mut() {
        let mut merged: Vec<MarketSample> = Vec::new();
        for (mid, cid, outcome, hg, ag) in samples.drain(..) {
            if let Some(existing) = merged.iter_mut().find(|m| m.0 == mid) {
                existing.2.extend(outcome);
            } else {
                merged.push((mid, cid, outcome, hg, ag));
            }
        }
        *samples = merged;
    }
    for (market, samples) in markets {
        let mut acc = Accumulator::default();
        for (mid, cid, outcomes, hg, ag) in samples {
            let names: Vec<&str> = match market.as_str() {
                "1x2" => vec!["home", "draw", "away"],
                "ou" => vec!["over", "under"],
                "btts" => vec!["yes", "no"],
                _ => continue,
            };
            let mut probs = Vec::new();
            for name in &names {
                let Some((_, p)) = outcomes.iter().find(|(outcome, _)| outcome == name) else {
                    acc.excluded += 1;
                    probs.clear();
                    break;
                };
                probs.push(*p);
            }
            if probs.is_empty() {
                continue;
            }
            let actual = match market.as_str() {
                "1x2" => outcome_1x2(hg, ag),
                "ou" => {
                    if hg + ag >= 3 {
                        0
                    } else {
                        1
                    }
                }
                "btts" => {
                    if hg > 0 && ag > 0 {
                        0
                    } else {
                        1
                    }
                }
                _ => continue,
            };
            // A piac-összehasonlítás csak 1X2-re korlátozott: az OU/BTTS
            // záró piaca nincs a mintában, ezért ott `None` (nem találgatunk).
            let market_probs = if market == "1x2" {
                market_by_match.get(&mid)
            } else {
                None
            };
            acc.add(
                &probs,
                market_probs.map(Vec::as_slice),
                actual,
                cluster_of(cid),
            );
        }
        report_and_store(pool, acc, &model_version, &market, from, to, apply).await?;
    }
    if !apply {
        println!("Dry-run — egyik mérés sem került rögzítésre (--apply).");
    }
    Ok(())
}
