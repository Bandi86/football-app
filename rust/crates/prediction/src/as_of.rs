//! Szigorú as-of őr (nincs look-ahead / jövőbeli információ).
//!
//! A v1-ben a `date_only` 12:00 placeholder miatt 213 predikció keletkezett
//! a kezdés után; a v2-ben ezért minden feature-nek és predikciónak
//! **szigorúan a kickoff előtt** rögzítettnek kell lennie, és a kezdési idő
//! pontossága is ellenőrzött kell legyen.

use chrono::{DateTime, Utc};

/// Csak olyan megfigyelés használható, amelyet a kezdés ELŐTT rögzítettek.
/// Az egyenlőség (pontosan kickoffkor) sem megengedett: az már élő adat lehet.
pub fn is_as_of(captured_at: DateTime<Utc>, kickoff_at: DateTime<Utc>) -> bool {
    captured_at < kickoff_at
}

/// A `date_only` sorok kezdési ideje placeholder (00:00 UTC), amiből nem
/// lehet as-of következtetést vonni — ezek kimaradnak a modellből.
pub fn has_usable_kickoff_precision(precision: Option<&str>) -> bool {
    precision.is_some_and(|value| value == "exact")
}

/// Teljes as-of ellenőrzés: pontos kezdés + rögzítés a kickoff előtt +
/// predikció rögzítési ideje is a kickoff előtt.
pub fn is_model_input_valid(
    kickoff_precision: Option<&str>,
    kickoff_at: DateTime<Utc>,
    feature_captured_at: DateTime<Utc>,
    prediction_created_at: DateTime<Utc>,
) -> bool {
    has_usable_kickoff_precision(kickoff_precision)
        && is_as_of(feature_captured_at, kickoff_at)
        && is_as_of(prediction_created_at, kickoff_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 27, 18, minute, 0).unwrap()
    }

    #[test]
    fn only_data_before_kickoff_is_usable() {
        assert!(is_as_of(at(10), at(30)));
        assert!(!is_as_of(at(30), at(30)), "pontosan kickoffkor már késő");
        assert!(!is_as_of(at(31), at(30)));
    }

    #[test]
    fn date_only_placeholder_is_rejected() {
        assert!(has_usable_kickoff_precision(Some("exact")));
        assert!(!has_usable_kickoff_precision(Some("date_only")));
        assert!(!has_usable_kickoff_precision(None));
    }

    #[test]
    fn full_guard_rejects_look_ahead_from_either_side() {
        assert!(is_model_input_valid(Some("exact"), at(30), at(10), at(20)));
        assert!(!is_model_input_valid(
            Some("date_only"),
            at(30),
            at(10),
            at(20)
        ));
        assert!(!is_model_input_valid(Some("exact"), at(30), at(40), at(20)));
        assert!(!is_model_input_valid(Some("exact"), at(30), at(10), at(40)));
    }
}
