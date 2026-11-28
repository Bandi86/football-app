//! Regressziós teszt: ESPN-soccer scoreboard → normalizált observation + provenance.
//!
//! Szintetikus fixture; nincs hálózat, nincs DB-írás, dry-run.

use chrono::{DateTime, Utc};
use football_ingest::espn_sportsleader::{
    EspnSoccerLeague, REFERENCE_LEAGUE_ENG1, SOURCE_CODE, SPORT_SOCCER, parse_soccer_scoreboard,
};

const FIXTURE: &str = include_str!("fixtures/espn_soccer_eng1_scoreboard.json");
const SOURCE_URL: &str =
    "https://site.api.espn.com/apis/site/v2/sports/soccer/eng.1/scoreboard?dates=20261127";

fn captured_at() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-28T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn named_source_is_espn_sportsleader_soccer_eng1() {
    assert_eq!(SOURCE_CODE, "espn_sportsleader");
    assert_eq!(SPORT_SOCCER, "soccer");
    assert_eq!(REFERENCE_LEAGUE_ENG1, "eng.1");
    let league = EspnSoccerLeague::eng_1();
    assert_eq!(league.sport, "soccer");
    assert_eq!(league.league, "eng.1");
}

#[test]
fn parses_finished_match_with_full_provenance() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .expect("scoreboard parses");

    // Dry-run: nem ír semmit, csak observation-okat ad.
    assert!(out.report.dry_run);
    assert_eq!(out.report.source, "espn_sportsleader");
    assert_eq!(out.report.source_url, SOURCE_URL);
    assert_eq!(
        out.report.captured_at.as_deref(),
        Some("2026-11-28T10:00:00Z")
    );

    let core = out
        .observations
        .iter()
        .find(|o| o.aspect == "match_core" && o.source_record_key == "espn:event:401879225")
        .expect("match_core for finished event");

    assert_eq!(core.entity_kind, "match");
    assert_eq!(core.natural_key, "soccer:eng.1:event:401879225");
    assert_eq!(core.observed_at, captured_at());

    let p = &core.payload;
    assert_eq!(p["source_code"], "espn_sportsleader");
    assert_eq!(p["captured_at"], "2026-11-28T10:00:00Z");
    assert_eq!(p["provenance"]["source"], "espn_sportsleader");
    assert_eq!(p["provenance"]["source_url"], SOURCE_URL);
    assert_eq!(p["provenance"]["captured_at"], "2026-11-28T10:00:00Z");
    assert_eq!(p["provenance"]["sport"], "soccer");
    assert_eq!(p["provenance"]["league"], "eng.1");

    assert_eq!(p["source_home_id"], "espn:team:393");
    assert_eq!(p["source_away_id"], "espn:team:363");
    assert_eq!(p["kickoff_at"], "2026-11-27T20:00:00Z");
    assert_eq!(p["kickoff_precision"], "exact");
    assert_eq!(p["competition_timezone"], "UTC");
    assert_eq!(p["status"], "finished");
    assert_eq!(p["result"]["home_ft"], 3);
    assert_eq!(p["result"]["away_ft"], 2);
    // Hiányzó eredmény-mezők NULL/unknown, nem kitalált 0.
    assert!(p["result"]["home_ht"].is_null());
    assert_eq!(p["competition"]["name"], "English Premier League");
    assert_eq!(p["competition"]["source_entity_id"], "espn:league:700");
    assert_eq!(p["competition"]["country_code"], "");
    assert_eq!(p["season"]["label"], "2026-27 English Premier League");

    // Provenance-mezők minden rekordon: source_url + captured_at + source.
    for obs in &out.observations {
        if obs.aspect == "match_link" {
            // match_link payload tömb (NormalizedDay-kompatibilis); elemei hordozzák.
            let arr = obs.payload.as_array().expect("match_link array");
            assert!(!arr.is_empty());
            for l in arr {
                assert_eq!(l["source"], "espn_sportsleader");
                assert_eq!(l["source_url"], SOURCE_URL);
                assert_eq!(l["captured_at"], "2026-11-28T10:00:00Z");
            }
        } else {
            assert_eq!(obs.payload["source"], "espn_sportsleader", "obs {obs:?}");
            assert_eq!(obs.payload["source_url"], SOURCE_URL, "obs {obs:?}");
            assert_eq!(
                obs.payload["captured_at"], "2026-11-28T10:00:00Z",
                "obs {obs:?}"
            );
            assert_eq!(
                obs.payload["provenance"]["source_url"], SOURCE_URL,
                "obs {obs:?}"
            );
            assert_eq!(
                obs.payload["provenance"]["captured_at"],
                "2026-11-28T10:00:00Z"
            );
        }
    }
}

#[test]
fn scheduled_match_has_no_invented_result() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    let core = out
        .observations
        .iter()
        .find(|o| o.aspect == "match_core" && o.source_record_key == "espn:event:401879226")
        .expect("scheduled match_core");
    assert_eq!(core.payload["status"], "scheduled");
    assert!(core.payload["result"].is_null());
    assert!(core.payload["result"].get("home_ft").is_none() || core.payload["result"].is_null());
}

#[test]
fn team_identity_observations_are_stable_and_not_fuzzy() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    let team = out
        .observations
        .iter()
        .find(|o| o.aspect == "team_identity" && o.source_record_key == "espn:team:393")
        .expect("team_identity");
    assert_eq!(team.entity_kind, "team");
    assert_eq!(team.natural_key, "soccer:eng.1:team:393");
    assert_eq!(team.payload["name"], "Nottingham Forest");
    // Nincs kitalált ország: üres, nem gyártott kód.
    assert_eq!(team.payload["country_code"], "");
    assert_eq!(team.payload["gender"], "male");
    assert_eq!(team.payload["age_group"], "senior");
    assert_eq!(team.payload["branch"], "senior");
}

#[test]
fn match_environment_only_when_source_proves_venue() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    let env = out
        .observations
        .iter()
        .find(|o| o.aspect == "match_environment" && o.source_record_key == "espn:event:401879225")
        .expect("match_environment from venue");
    assert_eq!(env.payload["venue_name"], "The City Ground");
    assert_eq!(env.payload["venue_city"], "Nottingham");
    assert_eq!(env.payload["venue_country"], "England");
    // Nincs időjárás/capacity adat a forrásban → nem kitalált.
    assert!(env.payload["temperature_c"].is_null());
    assert!(env.payload["venue_capacity"].is_null());
    // A második, venue nélküli meccshez nincs match_environment.
    assert!(
        !out.observations
            .iter()
            .any(|o| o.aspect == "match_environment"
                && o.source_record_key == "espn:event:401879226")
    );
}

#[test]
fn match_referee_only_when_source_proves_officials() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    // A nyers soccer scoreboard nem ad officials-t → nincs match_referee.
    assert!(!out.observations.iter().any(|o| o.aspect == "match_referee"));
    assert_eq!(out.report.match_referee_emitted, 0);
}

#[test]
fn match_link_carries_source_url_provenance() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    let link = out
        .observations
        .iter()
        .find(|o| o.aspect == "match_link" && o.source_record_key == "espn:event:401879225")
        .expect("match_link");
    let arr = link.payload.as_array().expect("match_link payload array");
    assert!(!arr.is_empty());
    assert!(
        arr.iter()
            .any(|l| l["url"].as_str().is_some_and(|u| u.contains("espn.com")))
    );
    assert!(arr.iter().all(|l| l["source"] == "espn_sportsleader"));
    assert!(
        arr.iter()
            .all(|l| l["captured_at"] == "2026-11-28T10:00:00Z")
    );
}

#[test]
fn evidence_missing_records_are_dropped_and_counted_not_fabricated() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    let r = &out.report;
    assert_eq!(r.events_seen, 6);
    assert_eq!(r.events_dropped_missing_id, 1);
    assert_eq!(r.events_dropped_missing_kickoff, 1);
    assert_eq!(r.events_dropped_missing_competitor, 1);
    assert_eq!(r.events_dropped_unmapped_status, 1);
    assert_eq!(r.match_core_emitted, 2);
    assert_eq!(r.observations_emitted, out.observations.len());
}

#[test]
fn stable_dedupe_keys_and_no_duplicate_observations() {
    let a = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    let b = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    let key = |o: &football_ingest::adapter::Observation| {
        (
            o.entity_kind.clone(),
            o.aspect.clone(),
            o.source_record_key.clone(),
        )
    };
    let mut ka: Vec<_> = a.observations.iter().map(key).collect();
    let mut kb: Vec<_> = b.observations.iter().map(key).collect();
    ka.sort();
    kb.sort();
    assert_eq!(ka, kb, "determinisztikus kimenet");
    ka.dedup();
    let mut uniq = ka.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(
        ka.len(),
        uniq.len(),
        "nincs duplikált source_record_key per aspect"
    );
}

#[test]
fn schema_drift_is_typed_error_not_silent() {
    let err = parse_soccer_scoreboard(
        "{ not json",
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap_err();
    // ParseError::SchemaDrift vagy Parse, nem pánik, nem csendben üres.
    let msg = format!("{err}");
    assert!(!msg.is_empty());
}

#[test]
fn output_is_normalized_day_from_observations_compatible() {
    let out = parse_soccer_scoreboard(
        FIXTURE,
        SOURCE_URL,
        captured_at(),
        &EspnSoccerLeague::eng_1(),
    )
    .unwrap();
    // A meglévő source-union út: `NormalizedDay::from_observations`.
    let day = football_ingest::normalized::NormalizedDay::from_observations(&out.observations)
        .expect("NormalizedDay::from_observations");
    assert_eq!(day.source_code, "espn_sportsleader");
    assert_eq!(day.matches.len(), 2);
    assert!(
        day.teams
            .iter()
            .any(|t| t.source_entity_id == "espn:team:393")
    );
    let m = day
        .matches
        .iter()
        .find(|m| m.source_match_id == "espn:event:401879225")
        .unwrap();
    assert_eq!(m.status, "finished");
    assert!(m.result.is_some());
    assert!(m.environment.is_some());
}
