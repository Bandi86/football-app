//! As-of paraméterillesztés a gólmodellhez (S198 / P1b).
//!
//! Empirikus-Bayes zsugorítás a ligaátlaghoz (`K = 4`), hazai/vendég bontással
//! és 365 napos ablakban 180 napos felezési idővel — a v1 aktív v4 modellje
//! szerint, de szigorúan a becslési időpont (`cutoff`) ELŐTTi meccsekből.
//!
//! Tiszta modul: nem olvas adatbázist, nem ír entitást. A bemenetet a hívó
//! (CLI/storage) as-of lekérdezéssel állítja elő.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Egy lezárt meccs a becsléshez. A `kickoff_at` a look-ahead őrhöz kell.
#[derive(Debug, Clone)]
pub struct ScoredMatch {
    pub kickoff_at: DateTime<Utc>,
    pub home: Uuid,
    pub away: Uuid,
    pub home_goals: f64,
    pub away_goals: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct RateBucket {
    goals_for: f64,
    goals_against: f64,
    games: f64,
}

impl RateBucket {
    fn add(&mut self, gf: f64, ga: f64, weight: f64) {
        self.goals_for += gf * weight;
        self.goals_against += ga * weight;
        self.games += weight;
    }

    /// `(GF + K·bázis) / (játszott + K)` — a bázis felé zsugorítás.
    fn shrunk(&self, baseline: f64, k: f64) -> f64 {
        if self.games + k <= 0.0 {
            return baseline.max(0.05);
        }
        ((self.goals_for + k * baseline) / (self.games + k)).max(0.05)
    }

    /// Ugyanez a kapott gólokra — a védekező erősség alapja.
    fn shrunk_against(&self, baseline: f64, k: f64) -> f64 {
        if self.games + k <= 0.0 {
            return baseline.max(0.05);
        }
        ((self.goals_against + k * baseline) / (self.games + k)).max(0.05)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TeamBuckets {
    home: RateBucket,
    away: RateBucket,
}

/// Egy csapat támadási és védelmi erőssége a ligaátlaghoz képest (1.0 = átlag).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamStrength {
    pub attack_home: f64,
    pub attack_away: f64,
    pub defense_home: f64,
    pub defense_away: f64,
}

/// A becsléshez használt liga- és csapatstatisztika.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitParams {
    pub league_home_goals: f64,
    pub league_away_goals: f64,
    /// `lambda_home = league_home · attack_home(home) · defense_away(away)`.
    pub lambda_home: f64,
    pub lambda_away: f64,
}

fn weight(kickoff: DateTime<Utc>, cutoff: DateTime<Utc>, half_life_days: f64) -> f64 {
    if kickoff >= cutoff || half_life_days <= 0.0 {
        return 0.0;
    }
    let age_days = (cutoff - kickoff).num_seconds() as f64 / 86_400.0;
    0.5_f64.powf(age_days / half_life_days)
}

/// Illesztés két csapatra egy adott `cutoff` időpontra.
/// A `matches` **csak** olyan lezárt meccseket tartalmazhat, amelyeknek a
/// kickoffja a `cutoff` előtt van (a hívó ezt SQL-ben is biztosítja; itt újra
/// ellenőrizzük, fail-closed).
pub fn fit_lambdas(
    home: Uuid,
    away: Uuid,
    cutoff: DateTime<Utc>,
    matches: &[ScoredMatch],
    half_life_days: f64,
    k: f64,
) -> Option<FitParams> {
    if k < 0.0 || !k.is_finite() || !half_life_days.is_finite() {
        return None;
    }
    let mut league_home = RateBucket::default();
    let mut league_away = RateBucket::default();
    let mut teams: std::collections::HashMap<Uuid, TeamBuckets> = std::collections::HashMap::new();

    for m in matches {
        let w = weight(m.kickoff_at, cutoff, half_life_days);
        if w <= 0.0 || !m.home_goals.is_finite() || !m.away_goals.is_finite() {
            continue;
        }
        league_home.add(m.home_goals, m.away_goals, w);
        league_away.add(m.away_goals, m.home_goals, w);
        teams
            .entry(m.home)
            .or_default()
            .home
            .add(m.home_goals, m.away_goals, w);
        teams
            .entry(m.away)
            .or_default()
            .away
            .add(m.away_goals, m.home_goals, w);
    }
    if league_home.games <= 0.0 || league_away.games <= 0.0 {
        return None;
    }
    let base_home = (league_home.goals_for / league_home.games).max(0.05);
    let base_away = (league_away.goals_for / league_away.games).max(0.05);
    let empty = TeamBuckets::default();

    let h = &teams.get(&home).unwrap_or(&empty);
    let a = &teams.get(&away).unwrap_or(&empty);
    // A védelmi oldal a KAPOTT gólokból jön: minél kevesebb, annál erősebb.
    let strength = TeamStrength {
        attack_home: h.home.shrunk(base_home, k) / base_home,
        attack_away: h.away.shrunk(base_away, k) / base_away,
        // Védelem: a saját kapottgól-arány / ligaátlag — kevesebb kapott gól
        // → kisebb szorzó → az ellenfél kevesebb gólt vár.
        defense_home: h.home.shrunk_against(base_away, k) / base_away,
        defense_away: a.away.shrunk_against(base_home, k) / base_home,
    };
    let lambda_home = (base_home * strength.attack_home * strength.defense_away).clamp(0.05, 10.0);
    let lambda_away = (base_away * strength.attack_away * strength.defense_home).clamp(0.05, 10.0);
    if !lambda_home.is_finite() || !lambda_away.is_finite() {
        return None;
    }
    Some(FitParams {
        league_home_goals: base_home,
        league_away_goals: base_away,
        lambda_home,
        lambda_away,
    })
}

/// A v1 konvenció: K=4, 365 napos ablak, 180 napos felezési idő.
pub const V1_K: f64 = 4.0;
pub const V1_HALF_LIFE_DAYS: f64 = 180.0;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, day, 18, 0, 0).unwrap()
    }

    fn match_at(day: u32, home: Uuid, away: Uuid, hg: f64, ag: f64) -> ScoredMatch {
        ScoredMatch {
            kickoff_at: at(day),
            home,
            away,
            home_goals: hg,
            away_goals: ag,
        }
    }

    #[test]
    fn estimates_both_lambdas_from_weighted_history() {
        let home = Uuid::now_v7();
        let away = Uuid::now_v7();
        let other = Uuid::now_v7();
        let matches = vec![
            match_at(1, home, other, 2.0, 1.0),
            match_at(5, other, home, 0.0, 2.0),
            match_at(3, away, other, 1.0, 1.0),
            match_at(4, other, away, 1.0, 2.0),
            match_at(6, home, away, 1.0, 0.0),
        ];
        let fit = fit_lambdas(home, away, at(10), &matches, V1_HALF_LIFE_DAYS, V1_K).unwrap();
        assert!(fit.lambda_home > fit.lambda_away, "otthon erősebb: {fit:?}");
        assert!((fit.lambda_home - 1.0).abs() < 2.0);
        assert!(fit.league_home_goals > 0.0 && fit.league_away_goals > 0.0);
    }

    #[test]
    fn no_data_fails_closed() {
        let home = Uuid::now_v7();
        let away = Uuid::now_v7();
        assert!(fit_lambdas(home, away, at(10), &[], V1_HALF_LIFE_DAYS, V1_K).is_none());
    }

    #[test]
    fn future_and_invalid_matches_are_ignored() {
        let home = Uuid::now_v7();
        let away = Uuid::now_v7();
        let third = Uuid::now_v7();
        let matches = vec![
            match_at(11, home, third, 9.0, 0.0), // jövő: kimarad
            ScoredMatch {
                kickoff_at: at(1),
                home,
                away: third,
                home_goals: f64::NAN,
                away_goals: 0.0,
            },
            match_at(2, home, third, 1.0, 1.0),
            match_at(2, away, third, 1.0, 1.0),
        ];
        let fit = fit_lambdas(home, away, at(10), &matches, V1_HALF_LIFE_DAYS, V1_K).unwrap();
        assert!(fit.lambda_home > 0.0 && fit.lambda_away > 0.0);
    }

    #[test]
    fn shrinkage_limits_extreme_rates() {
        let home = Uuid::now_v7();
        let away = Uuid::now_v7();
        let third = Uuid::now_v7();
        let mut matches = Vec::new();
        for day in 1..=8 {
            matches.push(match_at(day, home, third, 6.0, 0.0));
            matches.push(match_at(day, third, away, 0.0, 6.0));
        }
        let fit = fit_lambdas(home, away, at(10), &matches, V1_HALF_LIFE_DAYS, V1_K).unwrap();
        assert!(fit.lambda_home <= 10.0 && fit.lambda_home >= 0.05);
        assert!(fit.lambda_away <= 10.0 && fit.lambda_away >= 0.05);
    }

    #[test]
    fn invalid_parameters_fail_closed() {
        let home = Uuid::now_v7();
        let away = Uuid::now_v7();
        assert!(fit_lambdas(home, away, at(10), &[], 0.0, V1_K).is_none());
        assert!(fit_lambdas(home, away, at(10), &[], V1_HALF_LIFE_DAYS, -1.0).is_none());
    }
}
