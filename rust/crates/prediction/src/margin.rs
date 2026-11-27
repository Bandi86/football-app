//! Piac-marginmentesítés (Shin) és belső érték (fair odds / edge).
//!
//! A bukméker szorzói nem arányosan áraznak (favorite-longshot bias): az
//! egyszerű normalizálás ezért torzít. A Shin-féle zsugorítás feltételezi,
//! hogy a piacon van egy belső információval rendelkező réteg, és a
//! valószínűségeket úgy korrigálja, hogy az összeg pontosan 1 legyen.
//!
//! Fail-closed: üres, nem pozitív vagy nem véges szorzók esetén `None`.

/// Shin-féle marginmentesített valószínűségek. A kimenetel sorrendje azonos
/// a bemenetével; az összeg 1 (a lebegőpontos maradék utolsó normalizálással
/// megszüntetjük). Nem használ véletlenszámot — determinisztikus biszekció.
pub fn shin_probabilities(odds: &[f64]) -> Option<Vec<f64>> {
    if odds.is_empty() {
        return None;
    }
    let raw: Vec<f64> = odds
        .iter()
        .map(|odds| (*odds > 1.0 && odds.is_finite()).then_some(1.0 / odds))
        .collect::<Option<Vec<_>>>()?;
    let sum: f64 = raw.iter().sum();
    if !sum.is_finite() || sum <= 1.0 {
        // Nincs beépített margin: a nyers normalizálás a helyes.
        return Some(raw.iter().map(|p| p / sum).collect());
    }

    let probs_at = |z: f64| -> Vec<f64> {
        raw.iter()
            .map(|p| {
                let inner = z * z + 4.0 * (1.0 - z) * p * p / sum;
                (inner.sqrt() - z) / (2.0 * (1.0 - z))
            })
            .collect()
    };

    let mut lo = 0.0_f64;
    let mut hi = 0.99_f64;
    for _ in 0..200 {
        let z = (lo + hi) / 2.0;
        if probs_at(z).iter().sum::<f64>() > 1.0 {
            lo = z;
        } else {
            hi = z;
        }
    }
    let z = (lo + hi) / 2.0;
    let mut probs = probs_at(z);
    let total: f64 = probs.iter().sum();
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    for p in &mut probs {
        *p /= total;
        if !p.is_finite() || *p <= 0.0 {
            return None;
        }
    }
    Some(probs)
}

/// A modell valószínűségéből számított tisztességes szorzó.
pub fn fair_odds(probability: f64) -> Option<f64> {
    (probability.is_finite() && probability > 0.0 && probability <= 1.0)
        .then_some(1.0 / probability)
}

/// Várható érték egy kimenetelre: `p * odds - 1`.
/// Pozitív = a kínált szorzó nagyobb, mint a valószínűség reciproka.
pub fn edge(probability: f64, odds: f64) -> Option<f64> {
    (probability.is_finite()
        && odds.is_finite()
        && probability > 0.0
        && probability <= 1.0
        && odds > 1.0)
        .then_some(probability * odds - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shin_sums_to_one_and_keeps_order() {
        let odds = [2.0, 3.5, 4.0];
        let p = shin_probabilities(&odds).expect("érvényes piac");
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(p[0] > p[1] && p[1] > p[2]);
        assert!(p.iter().all(|v| *v > 0.0 && *v < 1.0));
    }

    #[test]
    fn shin_shifts_probability_from_longshots_to_favorites() {
        let odds = [1.5, 4.5, 7.0];
        let p = shin_probabilities(&odds).unwrap();
        let naive: Vec<f64> = odds.iter().map(|o| 1.0 / o).collect();
        let naive_sum: f64 = naive.iter().sum();
        let naive: Vec<f64> = naive.iter().map(|v| v / naive_sum).collect();
        assert!(p[0] > naive[0], "favorit: {:.4} > {:.4}", p[0], naive[0]);
        assert!(p[2] < naive[2], "longshot: {:.4} < {:.4}", p[2], naive[2]);
    }

    #[test]
    fn shin_handles_two_way_and_no_margin_markets() {
        let two = shin_probabilities(&[1.8, 2.1]).unwrap();
        assert_eq!(two.len(), 2);
        assert!((two.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        let no_margin = shin_probabilities(&[4.0, 4.0]).unwrap();
        assert!((no_margin[0] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn invalid_odds_fail_closed() {
        assert!(shin_probabilities(&[]).is_none());
        assert!(shin_probabilities(&[1.0, 2.0]).is_none());
        assert!(shin_probabilities(&[f64::NAN, 2.0]).is_none());
        assert!(shin_probabilities(&[f64::INFINITY, 2.0]).is_none());
    }

    #[test]
    fn fair_odds_and_edge_are_well_defined() {
        assert_eq!(fair_odds(0.5), Some(2.0));
        assert!(fair_odds(0.0).is_none());
        assert!(fair_odds(1.01).is_none());
        let e = edge(0.5, 2.2).unwrap();
        assert!((e - 0.1).abs() < 1e-9);
        assert!(edge(0.5, 1.8).unwrap() < 0.0);
        assert!(edge(0.5, 1.0).is_none());
    }
}
