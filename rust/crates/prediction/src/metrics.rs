//! Valószínűségi metrikák és **megítélhetőségi** szerződés.
//!
//! A v1 tapasztalata: egy CI nélküli Brier-összehasonlítás hamis verdiktet
//! adhat (a backtest szignifikáns javulása élesben eltűnt). Itt ezért minden
//! összehasonlítás klaszterezett bootstrap-pel és Wilson-intervallummal jön.

/// Többosztályos Brier-pontosság egy megfigyelésre: Σ_i (p_i − y_i)².
/// A v1 `Brier/3` konvenciója a `classes` osztóval kérhető.
pub fn brier(probabilities: &[f64], actual: usize, classes: Option<f64>) -> Option<f64> {
    if probabilities.is_empty()
        || actual >= probabilities.len()
        || probabilities.iter().any(|p| !p.is_finite() || *p < 0.0)
    {
        return None;
    }
    let divisor = classes.unwrap_or(1.0);
    if !divisor.is_finite() || divisor <= 0.0 {
        return None;
    }
    let total: f64 = probabilities.iter().sum();
    if (total - 1.0).abs() > 1e-6 {
        return None;
    }
    let sum: f64 = probabilities
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let y = if i == actual { 1.0 } else { 0.0 };
            (p - y) * (p - y)
        })
        .sum();
    Some(sum / divisor)
}

/// Log-loss (multi-class cross-entropy) egy megfigyelésre.
pub fn log_loss(probabilities: &[f64], actual: usize) -> Option<f64> {
    if probabilities.is_empty()
        || actual >= probabilities.len()
        || probabilities
            .iter()
            .any(|p| !p.is_finite() || *p < 0.0 || *p > 1.0)
    {
        return None;
    }
    let p = probabilities[actual];
    if p <= 0.0 {
        return None;
    }
    Some(-p.ln())
}

/// Wilson-féle konfidenciaintervallum egy arányhoz (pl. kalibrációs sáv
/// találati aránya). `z = 1.96` ≈ 95%.
pub fn wilson_interval(successes: usize, n: usize, z: f64) -> Option<(f64, f64)> {
    if n == 0 || successes > n || !z.is_finite() || z <= 0.0 {
        return None;
    }
    let n = n as f64;
    let p = successes as f64 / n;
    let z2 = z * z;
    let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let spread = z * ((p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt()) / (1.0 + z2 / n);
    Some(((center - spread).max(0.0), (center + spread).min(1.0)))
}

/// Egy meccs-szintű páros minta: ugyanazon megfigyelésen a két modell Brier-e.
#[derive(Debug, Clone, Copy)]
pub struct PairedSample {
    /// A klaszter (pl. liga vagy nap) azonosítója — együtt húzzuk, hogy ne
    /// feltételezzük a meccsek függetlenségét.
    pub cluster: u64,
    pub brier_a: f64,
    pub brier_b: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct BootstrapResult {
    /// Átlagos különbség (A − B); negatív = A jobb.
    pub mean_difference: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub iterations: usize,
}

/// Determinista xorshift64 — a riportok reprodukálhatók, nincs `rand`-függőség.
fn uniform(seed: &mut u64) -> f64 {
    let mut x = *seed;
    if x == 0 {
        x = 0x9E37_79B9_7F4A_7C15;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *seed = x;
    (x as f64) / (u64::MAX as f64)
}

/// Klaszterezett bootstrap a páros Brier-különbséghez.
/// A konfidenciaintervallum nem keresztezi a 0-t → szignifikáns különbség.
pub fn cluster_bootstrap_difference(
    samples: &[PairedSample],
    iterations: usize,
    seed: u64,
) -> Option<BootstrapResult> {
    if samples.is_empty() || iterations < 100 {
        return None;
    }
    if samples
        .iter()
        .any(|s| !s.brier_a.is_finite() || !s.brier_b.is_finite())
    {
        return None;
    }
    let mut by_cluster: std::collections::BTreeMap<u64, Vec<(f64, f64)>> =
        std::collections::BTreeMap::new();
    for sample in samples {
        by_cluster
            .entry(sample.cluster)
            .or_default()
            .push((sample.brier_a, sample.brier_b));
    }
    let keys: Vec<u64> = by_cluster.keys().copied().collect();
    let mut rng = seed;
    let mut differences = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let mut total_a = 0.0;
        let mut total_b = 0.0;
        let mut count = 0usize;
        for _ in 0..keys.len() {
            let index = (uniform(&mut rng) * keys.len() as f64) as usize;
            let key = keys[index.min(keys.len() - 1)];
            for (a, b) in &by_cluster[&key] {
                total_a += *a;
                total_b += *b;
                count += 1;
            }
        }
        if count > 0 {
            differences.push((total_a - total_b) / count as f64);
        }
    }
    if differences.len() < 100 {
        return None;
    }
    differences.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pick = |fraction: f64| {
        let index = ((differences.len() as f64 - 1.0) * fraction).round() as usize;
        differences[index.min(differences.len() - 1)]
    };
    let mean = differences.iter().sum::<f64>() / differences.len() as f64;
    Some(BootstrapResult {
        mean_difference: mean,
        ci_low: pick(0.025),
        ci_high: pick(0.975),
        iterations: differences.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brier_and_log_loss_match_known_values() {
        let p = [0.5, 0.3, 0.2];
        let b = brier(&p, 0, None).unwrap();
        assert!((b - 0.38).abs() < 1e-9, "brier: {b}");
        let b3 = brier(&p, 0, Some(3.0)).unwrap();
        assert!((b3 - 0.38 / 3.0).abs() < 1e-9);
        assert!((log_loss(&p, 0).unwrap() - 0.5_f64.ln().abs()).abs() < 1e-9);
    }

    #[test]
    fn invalid_probabilities_fail_closed() {
        assert!(brier(&[0.5, 0.4], 0, None).is_none());
        assert!(brier(&[], 0, None).is_none());
        assert!(brier(&[0.5, 0.3, 0.2], 3, None).is_none());
        assert!(log_loss(&[0.0, 1.0], 0).is_none());
    }

    #[test]
    fn wilson_interval_contains_the_point_estimate() {
        let (lo, hi) = wilson_interval(70, 100, 1.96).unwrap();
        assert!(lo < 0.70 && hi > 0.70);
        assert!(lo > 0.55 && hi < 0.85);
        assert!(wilson_interval(0, 0, 1.96).is_none());
    }

    #[test]
    fn bootstrap_detects_a_significant_difference() {
        let samples: Vec<PairedSample> = (0..60)
            .map(|i| PairedSample {
                cluster: (i % 6) as u64,
                brier_a: 0.18 + (i as f64) * 0.0001,
                brier_b: 0.22 + (i as f64) * 0.0001,
            })
            .collect();
        let result = cluster_bootstrap_difference(&samples, 2000, 42).unwrap();
        assert!(result.mean_difference < 0.0);
        assert!(
            result.ci_high < 0.0,
            "CI: [{}, {}]",
            result.ci_low,
            result.ci_high
        );
        assert_eq!(result.iterations, 2000);
        assert!(cluster_bootstrap_difference(&samples, 10, 42).is_none());
    }

    #[test]
    fn bootstrap_does_not_claim_significance_from_noise() {
        let samples: Vec<PairedSample> = (0..40)
            .map(|i| PairedSample {
                cluster: (i % 4) as u64,
                brier_a: if i % 2 == 0 { 0.20 } else { 0.24 },
                brier_b: if i % 2 == 0 { 0.24 } else { 0.20 },
            })
            .collect();
        let result = cluster_bootstrap_difference(&samples, 2000, 7).unwrap();
        assert!(result.ci_low < 0.0 && result.ci_high > 0.0);
    }
}
