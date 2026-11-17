#![cfg(feature = "obscura")]

//! Élő Soccerway render teszt (opt-in): csak `OBSCURA_LIVE_TEST=1` esetén fut.
//! Napi oldal renderelése Obscurával; a tartalmi őrök, a dátum-guard és a
//! teljes parse-út ellenőrzése egy valós napon. Kiírja a kezdési időket a
//! TZ-pin (UTC) ellenőrzéséhez. Default futásban 0 élő kérés.

use std::time::Duration;

use chrono::Local;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher, RawPayload};
use football_ingest::obscura::{ObscuraConfig, ObscuraFetcher, server::ObscuraServer};
use football_ingest::soccerway::SoccerwayAdapter;

fn obscura_bin() -> String {
    std::env::var("OBSCURA_BIN").unwrap_or_else(|_| {
        format!(
            "{}/../../../../football-app/tools/obscura/obscura",
            env!("CARGO_MANIFEST_DIR")
        )
    })
}

fn first_group_slug(html: &str) -> Option<String> {
    let marker = "class=\"headerLeague__title\"";
    let idx = html.find(marker)?;
    let before = &html[..idx];
    let href = before.rfind("href=\"/")?;
    let rest = &html[href + 7..];
    let end = rest.find('"')?;
    Some(rest[..end].trim_matches('/').to_string())
}

#[tokio::test]
async fn live_soccerway_day_parses_with_date_guard() {
    if std::env::var("OBSCURA_LIVE_TEST").as_deref() != Ok("1") {
        return;
    }
    let bin = obscura_bin();
    if !std::path::Path::new(&bin).exists() {
        eprintln!("skip: nincs obscura bináris ({bin})");
        return;
    }

    let cfg = ObscuraConfig {
        bin: bin.into(),
        url: None,
        port: 19229,
        timeout_ms: 90000,
    };
    let mut srv = ObscuraServer::start(&cfg).await.unwrap();
    let fetcher = ObscuraFetcher::connect(srv.cdp_url(), Duration::from_millis(cfg.timeout_ms))
        .await
        .unwrap();
    let req = FetchRequest {
        url: "https://www.soccerway.com/".into(),
        request_key: "live".into(),
        endpoint_key: "matches".into(),
        ready_marker: Some("div.event__match[data-event-row]".into()),
        bearer_token: None,
    };
    let limits = FetchLimits {
        max_bytes: 20_000_000,
        timeout: Duration::from_secs(90),
    };
    let raw = fetcher.fetch(&req, &limits).await.unwrap();
    srv.stop();
    assert_eq!(raw.http_status, 200);

    let html = String::from_utf8_lossy(&raw.bytes).to_string();
    assert!(
        !football_ingest::soccerway::is_decoy(&html),
        "decoy-200 hiboldal jött vissza"
    );
    let rows = html.matches("data-event-row").count();
    assert!(rows > 0, "nincs renderelt meccssor");
    let today = Local::now().date_naive();
    let label = football_ingest::soccerway::page_date(&html).expect("nap-címke (aria-label)");
    eprintln!(
        "nap-címke: {label}, sorok: {rows}, bájtok: {}",
        raw.bytes.len()
    );

    // TZ-megfigyelés: az első ütemezett kezdési idők (a render UTC-re pinnelve).
    let times: Vec<String> = html
        .match_indices("class=\"event__time\">")
        .take(5)
        .map(|(i, m)| {
            let rest = &html[i + m.len()..];
            rest.chars().take(5).collect()
        })
        .collect();
    eprintln!("kezdési idők (első 5): {times:?}");

    let slug = first_group_slug(&html).expect("első verseny-slug");
    let adapter = SoccerwayAdapter::default();
    let ctx = ParseCtx {
        requested_date: today,
        request_key: format!("soccerway:day:{today}"),
    };
    let payload = RawPayload {
        bytes: raw.bytes.clone(),
        content_type: raw.content_type.clone(),
        http_status: raw.http_status,
        fetched_at: raw.fetched_at,
        sha256: raw.sha256.clone(),
    };
    let obs = adapter.parse(&payload, &ctx).expect("parse");
    let matches = obs.iter().filter(|o| o.aspect == "match_core").count();
    let teams = obs.iter().filter(|o| o.aspect == "team_identity").count();
    let competitions: std::collections::BTreeSet<String> = obs
        .iter()
        .filter(|o| o.aspect == "match_core")
        .filter_map(|o| o.payload.pointer("/competition/name"))
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    eprintln!(
        "első slug: {slug}, meccsek: {matches}, csapatok: {teams}, versenyek: {}",
        competitions.len()
    );
    assert!(
        matches > 0,
        "a nap-oldalon van sor, de nincs parse-olt meccs"
    );
    assert!(teams >= 2);
    assert!(!competitions.is_empty());
    assert_eq!(label, today, "a renderelt nap a helyi mai nap");
}
