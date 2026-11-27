//! Gólpiacok modellje (S197 / Phase 1): Poisson + Dixon–Coles alacsony
//! gólszám-korrekció, tiszta Rust. Nincs adatbázis, nincs illesztés, nincs
//! kanonikus írás — a modell paramétereit kívülről kapja.
//!
//! A sima Poisson feltételezi a hazai és vendég gólok függetlenségét; a
//! valóságban az alacsony eredmények (0-0, 1-0, 0-1, 1-1) gyakrabban és
//! másképp fordulnak elő. A Dixon–Coles `tau` ezt korrigálja `rho` paraméterrel.

use crate::margin::fair_odds;

/// Poisson valószínűségi tömegfüggvény: P(X = k), `lambda > 0`.
pub fn poisson_pmf(lambda: f64, k: u32) -> Option<f64> {
    if !lambda.is_finite() || lambda <= 0.0 {
        return None;
    }
    let mut term = (-lambda).exp();
    for i in 1..=k {
        term *= lambda / i as f64;
    }
    let value = term.max(0.0);
    value.is_finite().then_some(value)
}

/// Dixon–Coles korrekció az alacsony gólszámú cellákra. A négy érintett
/// cella szorzója; `rho = 0` esetén minden cella 1.0 (sima Poisson).
pub fn dixon_coles_tau(x: u32, y: u32, lambda: f64, mu: f64, rho: f64) -> f64 {
    if rho == 0.0 {
        return 1.0;
    }
    match (x, y) {
        (0, 0) => 1.0 - lambda * mu * rho,
        (0, 1) => 1.0 + lambda * rho,
        (1, 0) => 1.0 + mu * rho,
        (1, 1) => 1.0 - rho,
        _ => 1.0,
    }
}

/// Közös góleloszlás (`matrix[home][away]`) legfeljebb `max_goals` gólig.
/// A `tau`-val korrigált mátrixot a végén normalizáljuk (a csonkolt farok
/// miatt az összeg így is pontosan 1).
pub fn goal_matrix(
    lambda_home: f64,
    lambda_away: f64,
    rho: f64,
    max_goals: usize,
) -> Option<Vec<Vec<f64>>> {
    if max_goals < 4 {
        return None;
    }
    let mut matrix = Vec::with_capacity(max_goals + 1);
    for x in 0..=max_goals {
        let mut row = Vec::with_capacity(max_goals + 1);
        for y in 0..=max_goals {
            let base = poisson_pmf(lambda_home, x as u32)? * poisson_pmf(lambda_away, y as u32)?;
            let adjusted =
                base * dixon_coles_tau(x as u32, y as u32, lambda_home, lambda_away, rho);
            row.push(adjusted.max(0.0));
        }
        matrix.push(row);
    }
    let total: f64 = matrix.iter().flat_map(|row| row.iter()).sum();
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    for row in &mut matrix {
        for value in row {
            *value /= total;
        }
    }
    Some(matrix)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoalMarketProbabilities {
    pub home: f64,
    pub draw: f64,
    pub away: f64,
    pub over25: f64,
    pub under25: f64,
    pub btts_yes: f64,
    pub btts_no: f64,
}

/// A gólmátrixból származtatott piacok — minden valószínűség ugyanabból
/// az as-of eloszlásból jön (nincs külön heurisztika).
pub fn market_probabilities(matrix: &[Vec<f64>]) -> Option<GoalMarketProbabilities> {
    if matrix.is_empty() || matrix.iter().any(|row| row.len() != matrix.len()) {
        return None;
    }
    let mut home = 0.0;
    let mut draw = 0.0;
    let mut away = 0.0;
    let mut over = 0.0;
    let mut btts = 0.0;
    for (x, row) in matrix.iter().enumerate() {
        for (y, p) in row.iter().enumerate() {
            if x > y {
                home += p;
            } else if x == y {
                draw += p;
            } else {
                away += p;
            }
            if x + y >= 3 {
                over += p;
            }
            if x > 0 && y > 0 {
                btts += p;
            }
        }
    }
    let total: f64 = matrix.iter().flat_map(|row| row.iter()).sum();
    if (total - 1.0).abs() > 1e-6 {
        return None;
    }
    Some(GoalMarketProbabilities {
        home,
        draw,
        away,
        over25: over,
        under25: 1.0 - over,
        btts_yes: btts,
        btts_no: 1.0 - btts,
    })
}

/// A piacokhoz tartozó tisztességes szorzók (fair odds), azonos sorrendben:
/// 1X2, O/U 2.5, BTTS.
pub fn fair_odds_for_markets(p: &GoalMarketProbabilities) -> Option<[f64; 7]> {
    let values = [
        p.home, p.draw, p.away, p.over25, p.under25, p.btts_yes, p.btts_no,
    ];
    values
        .iter()
        .map(|v| fair_odds(*v))
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_matches_known_values() {
        let p = poisson_pmf(1.0, 0).unwrap();
        assert!((p - (-1.0_f64).exp()).abs() < 1e-12);
        let p1 = poisson_pmf(2.0, 1).unwrap();
        assert!((p1 - 2.0 * (-2.0_f64).exp()).abs() < 1e-12);
        assert!(poisson_pmf(0.0, 1).is_none());
    }

    #[test]
    fn rho_zero_reproduces_independent_poisson() {
        let matrix = goal_matrix(1.4, 1.1, 0.0, 12).unwrap();
        let total: f64 = matrix.iter().flat_map(|r| r.iter()).sum();
        assert!((total - 1.0).abs() < 1e-9);
        let cell = matrix[1][1];
        let expected = poisson_pmf(1.4, 1).unwrap() * poisson_pmf(1.1, 1).unwrap();
        // A csonkolás miatt a normalize kis korrekciót ad — 1%-nál kisebb kell legyen.
        assert!((cell - expected).abs() / expected < 0.01);
    }

    #[test]
    fn dixon_coles_adjusts_only_low_score_cells() {
        assert_eq!(dixon_coles_tau(2, 2, 1.4, 1.1, -0.1), 1.0);
        assert!(dixon_coles_tau(0, 0, 1.4, 1.1, -0.1) > 1.0);
        assert!(dixon_coles_tau(1, 1, 1.4, 1.1, 0.1) < 1.0);
    }

    #[test]
    fn markets_sum_to_one_and_are_mutually_consistent() {
        let matrix = goal_matrix(1.5, 1.2, -0.08, 14).unwrap();
        let p = market_probabilities(&matrix).unwrap();
        assert!((p.home + p.draw + p.away - 1.0).abs() < 1e-9);
        assert!((p.over25 + p.under25 - 1.0).abs() < 1e-9);
        assert!((p.btts_yes + p.btts_no - 1.0).abs() < 1e-9);
        assert!(p.home > p.draw && p.home > p.away, "hazai favorit: {p:?}");
        let odds = fair_odds_for_markets(&p).unwrap();
        assert!(odds.iter().all(|v| *v > 1.0));
        assert!((odds[0] - 1.0 / p.home).abs() < 1e-9);
    }

    #[test]
    fn invalid_inputs_fail_closed() {
        assert!(goal_matrix(1.0, 1.0, 0.0, 2).is_none());
        assert!(goal_matrix(-1.0, 1.0, 0.0, 10).is_none());
        assert!(market_probabilities(&[]).is_none());
    }
}
