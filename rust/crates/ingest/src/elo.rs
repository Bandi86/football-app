//! Saját Elo-számítás (S23f): a kanonikus, lezárt meccsek eredményeiből,
//! időrendben. A clubelo élő forrás elvetett (2026-11-12), ezért a rating
//! a saját adatbázisból számolt, `source='own-elo'`, `provenance='computed'`.

/// Kezdő rating.
pub const START: f64 = 1500.0;
/// K-faktor (mérkőzésenkénti maximális elmozdulás nagyságrendje).
pub const K: f64 = 24.0;

/// A hazai csapat várható pontja (0..1) a ratingkülönbségből.
pub fn expected(home: f64, away: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((away - home) / 400.0))
}

/// Egy meccs eredménye utáni ratingfrissítés (hazai, vendég).
pub fn update(home: f64, away: f64, home_score: i16, away_score: i16) -> (f64, f64) {
    let score = if home_score > away_score {
        1.0
    } else if home_score < away_score {
        0.0
    } else {
        0.5
    };
    let e = expected(home, away);
    (
        home + K * (score - e),
        away + K * ((1.0 - score) - (1.0 - e)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ratings_expect_half() {
        assert!((expected(1500.0, 1500.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn stronger_side_expects_more() {
        assert!(expected(1700.0, 1500.0) > 0.5);
        assert!(expected(1500.0, 1700.0) < 0.5);
    }

    #[test]
    fn winner_gains_loser_loses_zero_sum() {
        let (h, a) = update(1500.0, 1500.0, 2, 0);
        assert!(h > 1500.0);
        assert!(a < 1500.0);
        assert!((h + a - 3000.0).abs() < 1e-9);
    }

    #[test]
    fn draw_between_equals_keeps_ratings() {
        let (h, a) = update(1500.0, 1500.0, 1, 1);
        assert!((h - 1500.0).abs() < 1e-9);
        assert!((a - 1500.0).abs() < 1e-9);
    }

    #[test]
    fn upset_moves_more_than_expected_win() {
        let (underdog, _) = update(1400.0, 1700.0, 1, 0);
        let (favorite, _) = update(1700.0, 1400.0, 1, 0);
        assert!(underdog - 1400.0 > favorite - 1700.0);
    }
}
