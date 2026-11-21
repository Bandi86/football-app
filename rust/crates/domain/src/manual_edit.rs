//! Kézi meccsszerkesztés domain-szabályai (S80).
//!
//! A kliens kézi bevitelének teljes állapot-leírója: kezdés, pontosság, státusz
//! és a teljes eredmény (FT/HT/ET/tizenegyes + `decided_by`). A modul tiszta:
//! nincs DB- és HTTP-ismerete, csak a beérkező szándékot validálja és normalizálja.
//!
//! A szabályok forrása az API-szerződés:
//! - a hiányzó eredmény-mező a cserélt sorban `NULL`, nem 0;
//! - HT <= FT, ET csak döntetlen FT után és ET >= FT, tizenegyes csak döntetlen
//!   ET után;
//! - a `decided_by` a tényleges végkimenetelt jelöli, ezért konzisztensnek kell
//!   lennie a megadott eredménnyel (pens → tizenegyes, et → hosszabbítás,
//!   különben ft);
//! - `date_only` pontosságnál a kickoff a 12:00:00Z helyőrző;
//! - lezárt/elfogadott státuszhoz eredmény tartozik, a scheduled/confirmed/live/
//!   halftime/canceled/postponed/abandoned státuszhoz viszont nem.

use chrono::{DateTime, Timelike, Utc};
use thiserror::Error;

use crate::match_state::MatchStatus;

/// A kezdés pontossága: pontos időpont vagy csak naptári nap (12:00:00Z helyőrző).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickoffPrecision {
    Exact,
    DateOnly,
}

impl KickoffPrecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::DateOnly => "date_only",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
            "date_only" => Some(Self::DateOnly),
            _ => None,
        }
    }
}

/// A meccs tényleges végkimenetele: rendes játékidő, hosszabbítás, tizenegyes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecidedBy {
    Ft,
    Et,
    Pens,
}

impl DecidedBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ft => "ft",
            Self::Et => "et",
            Self::Pens => "pens",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ft" => Some(Self::Ft),
            "et" => Some(Self::Et),
            "pens" => Some(Self::Pens),
            _ => None,
        }
    }
}

/// A `core.match_result.winner_method` értéke, a `decided_by`-ból és a státuszból
/// származtatva (a kliens nem írhatja közvetlenül).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinnerMethod {
    Regular,
    ExtraTime,
    Penalties,
    Awarded,
}

impl WinnerMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::ExtraTime => "extra_time",
            Self::Penalties => "penalties",
            Self::Awarded => "awarded",
        }
    }
}

/// A `core.match_result.validity` értéke: kézi úton csak érvényes vagy
/// megítélt (awarded) eredmény keletkezhet, törölt/elállt nem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultValidity {
    Valid,
    Awarded,
}

impl ResultValidity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Awarded => "awarded",
        }
    }
}

/// A kliens által küldött teljes kézi szerkesztési szándék.
///
/// A `result: Some(..)` azt jelenti, hogy az eredmény-sor **teljesen**
/// cserélődik; a hiányzó pontszám a cserélt sorban `NULL`. `None` esetén az
/// eredmény érintetlen marad (kivéve a canceled/postponed törlést).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualMatchEdit {
    pub kickoff_at: Option<DateTime<Utc>>,
    pub kickoff_precision: Option<KickoffPrecision>,
    pub status: Option<MatchStatus>,
    pub result: Option<ManualResultEdit>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualResultEdit {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    pub decided_by: Option<DecidedBy>,
}

/// Validált, normalizált szerkesztés: a státusz már tartalmazza az
/// „eredmény megadva, státusz nincs → finished” szabályt, a `decided_by`,
/// `winner_method` és `validity` pedig származtatva van.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedManualEdit {
    pub kickoff_at: Option<DateTime<Utc>>,
    pub kickoff_precision: Option<KickoffPrecision>,
    pub status: Option<MatchStatus>,
    pub result: Option<ValidatedMatchResult>,
    /// Status canceled/postponed és nincs eredmény-mező: a meglévő eredmény törlendő.
    pub clear_result: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMatchResult {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    pub decided_by: DecidedBy,
    pub winner_method: WinnerMethod,
    pub validity: ResultValidity,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ManualEditError {
    #[error("Az indoklás kötelező.")]
    EmptyNote,
    #[error("Ismeretlen státusz: {0}.")]
    InvalidStatus(String),
    #[error("Ismeretlen kickoff-pontosság: {0}.")]
    InvalidKickoffPrecision(String),
    #[error("Ismeretlen decided_by: {0}.")]
    InvalidDecidedBy(String),
    #[error("A(z) {status} státuszhoz nem tartozhat eredmény.")]
    ResultWithNonResultStatus { status: &'static str },
    #[error("date_only pontosságnál a kezdés csak 12:00:00Z helyőrző lehet.")]
    DateOnlyPlaceholder,
    #[error("A(z) {field} értéke nem lehet negatív.")]
    NegativeScore { field: &'static str },
    #[error("Az FT eredményt mindkét oldalon meg kell adni.")]
    IncompleteFt,
    #[error("A félidőeredményt csak a rendes játékidő eredményével együtt lehet megadni.")]
    HtWithoutFt,
    #[error("A(z) {field} nem lehet nagyobb a(z) {ft_field} értékénél.")]
    HtGreaterThanFt {
        field: &'static str,
        ft_field: &'static str,
    },
    #[error("Hosszabbítás csak döntetlen rendes játékidő után értelmezhető.")]
    ExtraTimeWithoutDraw,
    #[error("A hosszabbítás eredményét mindkét oldalon meg kell adni.")]
    IncompleteExtraTime,
    #[error("A(z) {field} hosszabbítási eredmény nem lehet kisebb a rendes játékidő eredményénél.")]
    ExtraTimeLessThanFt { field: &'static str },
    #[error("Tizenegyespárbaj csak hosszabbítás után értelmezhető.")]
    PensWithoutExtraTime,
    #[error("Tizenegyespárbaj csak döntetlen hosszabbítás után értelmezhető.")]
    PensWithoutDrawExtraTime,
    #[error("A tizenegyespárbaj eredménye nem lehet döntetlen.")]
    PensDraw,
    #[error("A decided_by=pens értékhez tizenegyeseredmény kell.")]
    DecidedByPensWithoutPens,
    #[error("A decided_by=et értékhez hosszabbítási eredmény kell.")]
    DecidedByEtWithoutEt,
    #[error(
        "A decided_by=ft érték nem egyeztethető össze hosszabbítási vagy tizenegyeseredménnyel."
    )]
    DecidedByFtWithExtraTime,
    #[error("A hosszabbítási eredmény döntetlen, ezért a decided_by nem lehet et.")]
    EtDrawDecidedByEt,
    #[error("Awarded státusznál a félidő-, hosszabbítás- és tizenegyeseredmény nem lehet megadva.")]
    AwardedExtraFields,
}

/// A kliens szándékának validálása és normalizálása.
pub fn validate_manual_edit(
    edit: &ManualMatchEdit,
) -> Result<ValidatedManualEdit, ManualEditError> {
    if edit.note.trim().is_empty() {
        return Err(ManualEditError::EmptyNote);
    }

    if let Some(status) = edit.status
        && edit.result.is_some()
        && !allows_result(status)
    {
        return Err(ManualEditError::ResultWithNonResultStatus {
            status: status.as_db_str(),
        });
    }

    if matches!(edit.kickoff_precision, Some(KickoffPrecision::DateOnly))
        && let Some(kickoff_at) = edit.kickoff_at
        && !is_date_only_placeholder(kickoff_at)
    {
        return Err(ManualEditError::DateOnlyPlaceholder);
    }

    // „Eredmény megadva, státusz nincs → finished”; a törlést a canceled/
    // postponed státusz végzi, ha nincs eredmény-mező.
    let status = match (edit.status, edit.result.is_some()) {
        (Some(status), _) => Some(status),
        (None, true) => Some(MatchStatus::Finished),
        (None, false) => None,
    };
    let clear_result = edit.result.is_none()
        && matches!(
            edit.status,
            Some(MatchStatus::Canceled | MatchStatus::Postponed)
        );

    let result = edit
        .result
        .as_ref()
        .map(|result| validate_result(result, status))
        .transpose()?;

    Ok(ValidatedManualEdit {
        kickoff_at: edit.kickoff_at,
        kickoff_precision: edit.kickoff_precision,
        status,
        result,
        clear_result,
        note: edit.note.clone(),
    })
}

fn allows_result(status: MatchStatus) -> bool {
    matches!(status, MatchStatus::Finished | MatchStatus::Awarded)
}

fn is_date_only_placeholder(kickoff_at: DateTime<Utc>) -> bool {
    kickoff_at.hour() == 12
        && kickoff_at.minute() == 0
        && kickoff_at.second() == 0
        && kickoff_at.nanosecond() == 0
}

fn validate_result(
    result: &ManualResultEdit,
    status: Option<MatchStatus>,
) -> Result<ValidatedMatchResult, ManualEditError> {
    for (field, value) in [
        ("home_ft", result.home_ft),
        ("away_ft", result.away_ft),
        ("home_ht", result.home_ht),
        ("away_ht", result.away_ht),
        ("home_et", result.home_et),
        ("away_et", result.away_et),
        ("home_pens", result.home_pens),
        ("away_pens", result.away_pens),
    ] {
        if value.is_some_and(|value| value < 0) {
            return Err(ManualEditError::NegativeScore { field });
        }
    }

    // Ha bármelyik eredmény-mező jelen van, a cserélt sornak teljes FT-t kell
    // tartalmaznia: a lezárt meccs végeredménye a rendes játékidő állása.
    let (Some(home_ft), Some(away_ft)) = (result.home_ft, result.away_ft) else {
        return Err(ManualEditError::IncompleteFt);
    };

    if result.home_ht.is_some() || result.away_ht.is_some() {
        let (Some(home_ht), Some(away_ht)) = (result.home_ht, result.away_ht) else {
            return Err(ManualEditError::HtWithoutFt);
        };
        if home_ht > home_ft {
            return Err(ManualEditError::HtGreaterThanFt {
                field: "home_ht",
                ft_field: "home_ft",
            });
        }
        if away_ht > away_ft {
            return Err(ManualEditError::HtGreaterThanFt {
                field: "away_ht",
                ft_field: "away_ft",
            });
        }
    }

    let has_et = result.home_et.is_some() || result.away_et.is_some();
    let has_pens = result.home_pens.is_some() || result.away_pens.is_some();

    if has_et {
        if home_ft != away_ft {
            return Err(ManualEditError::ExtraTimeWithoutDraw);
        }
        let (Some(home_et), Some(away_et)) = (result.home_et, result.away_et) else {
            return Err(ManualEditError::IncompleteExtraTime);
        };
        if home_et < home_ft {
            return Err(ManualEditError::ExtraTimeLessThanFt { field: "home_et" });
        }
        if away_et < away_ft {
            return Err(ManualEditError::ExtraTimeLessThanFt { field: "away_et" });
        }
    }

    if has_pens {
        let (Some(home_et), Some(away_et)) = (result.home_et, result.away_et) else {
            return Err(ManualEditError::PensWithoutExtraTime);
        };
        if home_et != away_et {
            return Err(ManualEditError::PensWithoutDrawExtraTime);
        }
        let (Some(home_pens), Some(away_pens)) = (result.home_pens, result.away_pens) else {
            return Err(ManualEditError::PensWithoutExtraTime);
        };
        if home_pens == away_pens {
            return Err(ManualEditError::PensDraw);
        }
    }

    // A decided_by-t a megadott értékből, hiányában az eredmény alakjából
    // származtatjuk; a kliens csak a konzisztens értéket küldheti.
    let decided_by = match result.decided_by {
        Some(decided_by) => decided_by,
        None if has_pens => DecidedBy::Pens,
        None if has_et => DecidedBy::Et,
        None => DecidedBy::Ft,
    };
    match decided_by {
        DecidedBy::Pens if !has_pens => return Err(ManualEditError::DecidedByPensWithoutPens),
        DecidedBy::Et if !has_et => return Err(ManualEditError::DecidedByEtWithoutEt),
        DecidedBy::Ft if has_et || has_pens => {
            return Err(ManualEditError::DecidedByFtWithExtraTime);
        }
        DecidedBy::Et => {
            if let (Some(home_et), Some(away_et)) = (result.home_et, result.away_et)
                && home_et == away_et
            {
                return Err(ManualEditError::EtDrawDecidedByEt);
            }
        }
        _ => {}
    }

    let awarded = status == Some(MatchStatus::Awarded);
    if awarded
        && (result.home_ht.is_some()
            || result.away_ht.is_some()
            || result.home_et.is_some()
            || result.away_et.is_some()
            || result.home_pens.is_some()
            || result.away_pens.is_some())
    {
        return Err(ManualEditError::AwardedExtraFields);
    }

    let winner_method = if awarded {
        WinnerMethod::Awarded
    } else {
        match decided_by {
            DecidedBy::Ft => WinnerMethod::Regular,
            DecidedBy::Et => WinnerMethod::ExtraTime,
            DecidedBy::Pens => WinnerMethod::Penalties,
        }
    };
    let validity = if awarded {
        ResultValidity::Awarded
    } else {
        ResultValidity::Valid
    };

    Ok(ValidatedMatchResult {
        home_ft: result.home_ft,
        away_ft: result.away_ft,
        home_ht: result.home_ht,
        away_ht: result.away_ht,
        home_et: result.home_et,
        away_et: result.away_et,
        home_pens: result.home_pens,
        away_pens: result.away_pens,
        decided_by,
        winner_method,
        validity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn timestamp(raw: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(raw)
            .expect("érvényes RFC 3339")
            .with_timezone(&Utc)
    }

    fn edit(result: ManualResultEdit) -> ManualMatchEdit {
        ManualMatchEdit {
            kickoff_at: None,
            kickoff_precision: None,
            status: None,
            result: Some(result),
            note: "teszt".into(),
        }
    }

    fn result_ft(home: i16, away: i16) -> ManualResultEdit {
        ManualResultEdit {
            home_ft: Some(home),
            away_ft: Some(away),
            home_ht: None,
            away_ht: None,
            home_et: None,
            away_et: None,
            home_pens: None,
            away_pens: None,
            decided_by: None,
        }
    }

    #[test]
    fn regular_time_result_is_finished_by_default() {
        let validated = validate_manual_edit(&edit(result_ft(2, 1))).expect("érvényes");
        assert_eq!(validated.status, Some(MatchStatus::Finished));
        let result = validated.result.expect("eredmény");
        assert_eq!(result.decided_by, DecidedBy::Ft);
        assert_eq!(result.winner_method, WinnerMethod::Regular);
        assert_eq!(result.validity, ResultValidity::Valid);
        assert!(!validated.clear_result);
    }

    #[test]
    fn empty_note_is_rejected() {
        let mut input = edit(result_ft(1, 0));
        input.note = "   ".into();
        assert_eq!(
            validate_manual_edit(&input),
            Err(ManualEditError::EmptyNote)
        );
    }

    #[test]
    fn negative_score_is_rejected_per_field() {
        let mut result = result_ft(1, 0);
        result.away_ht = Some(-1);
        result.home_ht = Some(0);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::NegativeScore { field: "away_ht" })
        );
    }

    #[test]
    fn ht_cannot_exceed_ft() {
        let mut result = result_ft(1, 0);
        result.home_ht = Some(2);
        result.away_ht = Some(0);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::HtGreaterThanFt {
                field: "home_ht",
                ft_field: "home_ft",
            })
        );
    }

    #[test]
    fn ht_requires_both_sides() {
        let mut result = result_ft(1, 0);
        result.home_ht = Some(1);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::HtWithoutFt)
        );
    }

    #[test]
    fn extra_time_requires_drawn_ft() {
        let mut result = result_ft(2, 1);
        result.home_et = Some(3);
        result.away_et = Some(1);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::ExtraTimeWithoutDraw)
        );
    }

    #[test]
    fn extra_time_cannot_shrink_the_score() {
        let mut result = result_ft(1, 1);
        result.home_et = Some(0);
        result.away_et = Some(1);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::ExtraTimeLessThanFt { field: "home_et" })
        );
    }

    #[test]
    fn extra_time_draw_with_et_decision_is_rejected() {
        let mut result = result_ft(1, 1);
        result.home_et = Some(2);
        result.away_et = Some(2);
        result.decided_by = Some(DecidedBy::Et);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::EtDrawDecidedByEt)
        );
    }

    #[test]
    fn penalties_require_extra_time() {
        let mut result = result_ft(1, 1);
        result.home_pens = Some(4);
        result.away_pens = Some(3);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::PensWithoutExtraTime)
        );
    }

    #[test]
    fn penalties_require_drawn_extra_time() {
        let mut result = result_ft(1, 1);
        result.home_et = Some(2);
        result.away_et = Some(1);
        result.home_pens = Some(4);
        result.away_pens = Some(3);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::PensWithoutDrawExtraTime)
        );
    }

    #[test]
    fn penalties_cannot_be_drawn() {
        let mut result = result_ft(1, 1);
        result.home_et = Some(2);
        result.away_et = Some(2);
        result.home_pens = Some(4);
        result.away_pens = Some(4);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::PensDraw)
        );
    }

    #[test]
    fn penalties_derive_decided_by_and_winner_method() {
        let mut result = result_ft(1, 1);
        result.home_et = Some(2);
        result.away_et = Some(2);
        result.home_pens = Some(4);
        result.away_pens = Some(3);
        let validated = validate_manual_edit(&edit(result)).expect("érvényes");
        let result = validated.result.expect("eredmény");
        assert_eq!(result.decided_by, DecidedBy::Pens);
        assert_eq!(result.winner_method, WinnerMethod::Penalties);
    }

    #[test]
    fn extra_time_derives_winning_method() {
        let mut result = result_ft(1, 1);
        result.home_et = Some(2);
        result.away_et = Some(1);
        let validated = validate_manual_edit(&edit(result)).expect("érvényes");
        let result = validated.result.expect("eredmény");
        assert_eq!(result.decided_by, DecidedBy::Et);
        assert_eq!(result.winner_method, WinnerMethod::ExtraTime);
    }

    #[test]
    fn decided_by_must_match_the_scores() {
        let mut result = result_ft(1, 1);
        result.decided_by = Some(DecidedBy::Pens);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::DecidedByPensWithoutPens)
        );

        let mut result = result_ft(1, 1);
        result.decided_by = Some(DecidedBy::Et);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::DecidedByEtWithoutEt)
        );

        let mut result = result_ft(1, 1);
        result.home_et = Some(2);
        result.away_et = Some(1);
        result.decided_by = Some(DecidedBy::Ft);
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::DecidedByFtWithExtraTime)
        );
    }

    #[test]
    fn partial_ft_is_rejected() {
        let mut result = result_ft(1, 0);
        result.away_ft = None;
        assert_eq!(
            validate_manual_edit(&edit(result)),
            Err(ManualEditError::IncompleteFt)
        );
    }

    #[test]
    fn result_with_non_result_status_is_rejected() {
        for status in [
            MatchStatus::Scheduled,
            MatchStatus::Confirmed,
            MatchStatus::Live,
            MatchStatus::Halftime,
            MatchStatus::Canceled,
            MatchStatus::Postponed,
            MatchStatus::Abandoned,
        ] {
            let mut input = edit(result_ft(1, 0));
            input.status = Some(status);
            assert_eq!(
                validate_manual_edit(&input),
                Err(ManualEditError::ResultWithNonResultStatus {
                    status: status.as_db_str(),
                })
            );
        }
    }

    #[test]
    fn awarded_result_cannot_carry_ht_et_or_pens() {
        let mut result = result_ft(3, 0);
        result.home_ht = Some(1);
        result.away_ht = Some(0);
        let mut input = edit(result);
        input.status = Some(MatchStatus::Awarded);
        assert_eq!(
            validate_manual_edit(&input),
            Err(ManualEditError::AwardedExtraFields)
        );
    }

    #[test]
    fn awarded_result_sets_winner_method_and_validity() {
        let mut input = edit(result_ft(3, 0));
        input.status = Some(MatchStatus::Awarded);
        let validated = validate_manual_edit(&input).expect("érvényes");
        let result = validated.result.expect("eredmény");
        assert_eq!(result.winner_method, WinnerMethod::Awarded);
        assert_eq!(result.validity, ResultValidity::Awarded);
        assert!(!validated.clear_result);
    }

    #[test]
    fn canceled_without_result_clears_result_and_keeps_status() {
        let input = ManualMatchEdit {
            kickoff_at: None,
            kickoff_precision: None,
            status: Some(MatchStatus::Canceled),
            result: None,
            note: "elmaradt".into(),
        };
        let validated = validate_manual_edit(&input).expect("érvényes");
        assert!(validated.clear_result);
        assert_eq!(validated.status, Some(MatchStatus::Canceled));
        assert!(validated.result.is_none());
    }

    #[test]
    fn date_only_requires_noon_placeholder() {
        let mut input = edit(result_ft(1, 0));
        input.kickoff_precision = Some(KickoffPrecision::DateOnly);
        input.kickoff_at = Some(timestamp("2026-11-21T17:00:00Z"));
        assert_eq!(
            validate_manual_edit(&input),
            Err(ManualEditError::DateOnlyPlaceholder)
        );

        input.kickoff_at = Some(timestamp("2026-11-21T12:00:00Z"));
        assert!(validate_manual_edit(&input).is_ok());
    }

    #[test]
    fn kickoff_only_edit_is_valid() {
        let input = ManualMatchEdit {
            kickoff_at: Some(timestamp("2026-11-22T17:00:00Z")),
            kickoff_precision: Some(KickoffPrecision::Exact),
            status: None,
            result: None,
            note: "áthelyezve".into(),
        };
        let validated = validate_manual_edit(&input).expect("érvényes");
        assert!(validated.result.is_none());
        assert!(!validated.clear_result);
        assert_eq!(validated.status, None);
    }

    #[test]
    fn status_without_result_is_allowed() {
        let input = ManualMatchEdit {
            kickoff_at: None,
            kickoff_precision: None,
            status: Some(MatchStatus::Live),
            result: None,
            note: "élő".into(),
        };
        let validated = validate_manual_edit(&input).expect("érvényes");
        assert_eq!(validated.status, Some(MatchStatus::Live));
        assert!(validated.result.is_none());
    }

    #[test]
    fn invalid_enum_values_are_visible_in_errors() {
        assert!(KickoffPrecision::parse("nope").is_none());
        assert!(DecidedBy::parse("nope").is_none());
        assert_eq!(
            ManualEditError::InvalidStatus("nope".into()).to_string(),
            "Ismeretlen státusz: nope."
        );
        let _ = Utc.timestamp_opt(0, 0);
    }
}
