use chrono::NaiveDate;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::tipplap::TipplapAdapter;

fn raw(bytes: &[u8]) -> RawPayload {
    RawPayload {
        bytes: bytes.to_vec(),
        content_type: "text/html".into(),
        http_status: 200,
        fetched_at: chrono::Utc::now(),
        sha256: String::new(),
    }
}

fn ctx(date: &str) -> ParseCtx {
    ParseCtx {
        requested_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        request_key: "tipplap:day:2026-11-17".into(),
    }
}

const PAGE: &str = r##"<!DOCTYPE html><html><body>
<select id="dates" name="date">
  <option value="2026-11-17" selected="selected">2026. 11. 17. (ma)</option>
  <option value="2026-11-18">2026. 11. 18.</option>
</select>
<table border="0" class="table_format" id="calcTableDesktop">
  <tr><th colspan="9" class="tleft">Labdarúgás – Afrikai Nemzetek Kupája-sel.,A</th></tr>
  <tr data-event="5593445" data-event-expire="1794920400">
    <td width="80" data-market-number>19463</td>
    <td width="46" data-min-combi>1</td>
    <td width="100%"><div class="dflex aicenter"><div class="flex1">
      <a href="https://www.tipplap.hu/meccsek/2026-11-17-lesotho-gabon" data-event-name target="_blank">Lesotho - Gabon</a>
    </div></div></td>
    <td width="64" class="item" data-name="H"><div class="votes">0</div><div class="item_inner"><strong data-odds>7.25</strong></div></td>
    <td width="64" class="item" data-name="D"><div class="votes">0</div><div class="item_inner"><strong data-odds>4.10</strong></div></td>
    <td width="64" class="item" data-name="V"><div class="votes">1</div><div class="item_inner"><strong data-odds>1.34</strong></div></td>
    <td width="72">11.17.</td>
    <td width="72">14:00</td>
    <td width="58"><a href="#" class="showMarkets">+73</a></td>
  </tr>
  <tr><th colspan="9" class="tleft">Labdarúgás – U19 Eb-selejtező, A Liga, 1 cs</th></tr>
  <tr data-event="5599999">
    <td width="80" data-market-number>1</td><td width="46" data-min-combi>1</td>
    <td width="100%"><div><a href="#" data-event-name>Magyarország U19 - Szlovénia U19</a></div></td>
    <td width="64" class="item" data-name="H"><strong data-odds>2.10</strong></td>
    <td width="64" class="item" data-name="D"><strong data-odds>3.40</strong></td>
    <td width="64" class="item" data-name="V"><strong data-odds>3.10</strong></td>
    <td width="72">11.17.</td><td width="72">18:00</td><td width="58"></td>
  </tr>
  <tr><th colspan="9" class="tleft">Labdarúgás – Válogatott mérkőzés</th></tr>
  <tr data-event="5598888">
    <td width="80" data-market-number>2</td><td width="46" data-min-combi>1</td>
    <td width="100%"><div><a href="#" data-event-name>Magyarország hány gólt szerez? (9,5) - Igen</a></div></td>
    <td width="64" class="item" data-name="H"><strong data-odds>1.80</strong></td>
    <td width="64" class="item" data-name="D"><strong data-odds>2.00</strong></td>
    <td width="64" class="item" data-name="V"><strong data-odds>4.00</strong></td>
    <td width="72">11.17.</td><td width="72">20:45</td><td width="58"></td>
  </tr>
</table>
</body></html>"##;

#[test]
fn parses_international_event_with_odds_and_round() {
    let obs = TipplapAdapter
        .parse(&raw(PAGE.as_bytes()), &ctx("2026-11-17"))
        .unwrap();
    let teams: Vec<_> = obs.iter().filter(|o| o.aspect == "team_identity").collect();
    let matches: Vec<_> = obs.iter().filter(|o| o.aspect == "match_core").collect();
    let odds: Vec<_> = obs.iter().filter(|o| o.aspect == "match_odds").collect();
    assert_eq!(teams.len(), 2, "Lesotho + Gabon");
    assert_eq!(matches.len(), 1);
    assert_eq!(odds.len(), 1);

    let core = matches[0];
    assert_eq!(
        core.payload["competition"]["name"],
        "Africa Cup of Nations - Qualification"
    );
    assert_eq!(core.payload["round_label"], "A");
    // 14:00 Budapest (CET, +1) → 13:00 UTC.
    assert_eq!(core.payload["kickoff_at"], "2026-11-17T13:00:00Z");
    assert_eq!(core.payload["season"]["label"], "2026/27");

    let quotes = odds[0].payload["quotes"].as_array().unwrap();
    assert_eq!(quotes.len(), 3);
    assert_eq!(quotes[0]["bookmaker"], "Tippmix");
    assert_eq!(quotes[0]["price"], 7.25);

    let lesotho = teams
        .iter()
        .find(|t| t.payload["name"] == "Lesotho")
        .expect("Lesotho team");
    assert_eq!(lesotho.payload["country_code"], "LS");
}

#[test]
fn date_guard_fails_closed() {
    let err = TipplapAdapter
        .parse(&raw(PAGE.as_bytes()), &ctx("2026-11-18"))
        .unwrap_err();
    let text = format!("{err:?}");
    assert!(text.contains("DateGuard"), "várt dátum-guard hiba: {text}");
}

#[test]
fn unknown_leagues_and_special_bets_are_skipped() {
    let obs = TipplapAdapter
        .parse(&raw(PAGE.as_bytes()), &ctx("2026-11-17"))
        .unwrap();
    // Csak az Afrikai Kupa meccs marad; az U19 liga ismeretlen, a kérdőjeles
    // sor speciális tipp → egyik sem termel observationt.
    assert_eq!(obs.iter().filter(|o| o.aspect == "match_core").count(), 1);
    assert!(obs.iter().all(|o| !o.source_record_key.contains("5599999")));
    assert!(obs.iter().all(|o| !o.source_record_key.contains("5598888")));
}

#[test]
fn empty_table_is_schema_drift() {
    let empty = PAGE.replace("data-event=", "data-hidden=");
    let err = TipplapAdapter
        .parse(&raw(empty.as_bytes()), &ctx("2026-11-17"))
        .unwrap_err();
    assert!(format!("{err:?}").contains("SchemaDrift"));
}

const NEW_LEAGUE_PAGE: &str = r##"<!DOCTYPE html><html><body>
<select id="dates" name="date">
  <option value="2026-11-17" selected="selected">2026. 11. 17. (ma)</option>
</select>
<table border="0" class="table_format" id="calcTableDesktop">
  <tr><th colspan="9" class="tleft">Labdarúgás – Angol Liga 1.</th></tr>
  <tr data-event="5600001">
    <td width="80" data-market-number>1</td><td width="46" data-min-combi>1</td>
    <td width="100%"><div><a href="#" data-event-name>Peterborough - Barnsley</a></div></td>
    <td width="64" class="item" data-name="H"><strong data-odds>2.30</strong></td>
    <td width="64" class="item" data-name="D"><strong data-odds>3.40</strong></td>
    <td width="64" class="item" data-name="V"><strong data-odds>2.90</strong></td>
    <td width="72">11.17.</td><td width="72">20:45</td><td width="58"></td>
  </tr>
</table>
</body></html>"##;

#[test]
fn parses_extended_league_from_general_table() {
    let obs = TipplapAdapter
        .parse(&raw(NEW_LEAGUE_PAGE.as_bytes()), &ctx("2026-11-17"))
        .unwrap();
    let core: Vec<_> = obs.iter().filter(|o| o.aspect == "match_core").collect();
    assert_eq!(core.len(), 1);
    let comp = &core[0].payload["competition"];
    assert_eq!(comp["name"], "League One");
    assert_eq!(comp["country_code"], "GB");
    assert_eq!(comp["country_name"], "Scotland");
    assert_eq!(comp["tier"], 3);
    let odds: Vec<_> = obs.iter().filter(|o| o.aspect == "match_odds").collect();
    assert_eq!(odds.len(), 1);
    assert_eq!(odds[0].payload["quotes"][0]["price"], 2.30);
}

#[test]
fn markets_job_targets_the_event_endpoint() {
    use football_domain::policy::FetchJob;
    use uuid::Uuid;

    let job = FetchJob {
        id: Uuid::now_v7(),
        source_slug: "tipplap".into(),
        domain: "markets".into(),
        entity_key: "markets-5621252".into(),
        event_type: "refresh".into(),
        time_window: "2026-11-27".into(),
        due_at: chrono::Utc::now(),
        priority: 10,
    };
    let plan = TipplapAdapter.plan(&job);
    assert_eq!(plan.len(), 1);
    assert_eq!(
        plan[0].url,
        "https://www.tipplap.hu/kalkulator/markets/5621252"
    );
    assert_eq!(plan[0].request_key, "tipplap:markets:5621252");
    assert_eq!(plan[0].endpoint_key, "markets");
}

#[test]
fn markets_parse_emits_all_quotes_under_the_day_source_key() {
    // Kézzel írt, minimal markets-fixture — nyers scrape-elt payload nem kerül
    // a repóba.
    let desktop = r#"
      <tr data-event="5621252" data-market><th class="tleft">Mindkét csapat szerez gólt</th></tr>
      <tr data-event="5621252" data-market>
        <td data-market-number>16943</td><td><div data-outcome-name>Igen</div></td>
        <td data-outcome-code>H</td>
        <td data-outcome-id="101080923" data-market-name="Mindkét csapat szerez gólt">
          <div data-odds>2.02</div></td>
      </tr>
      <tr data-event="5621252" data-market>
        <td data-market-number>16943</td><td><div data-outcome-name>Nem</div></td>
        <td data-outcome-code>V</td>
        <td data-outcome-id="101080924" data-market-name="Mindkét csapat szerez gólt">
          <div data-odds>1.63</div></td>
      </tr>
      <tr data-event="5621252" data-market><th class="tleft">Gólszám 2,5</th></tr>
      <tr data-event="5621252" data-market>
        <td data-market-number>16948</td><td><div data-outcome-name>Több, mint 2,5</div></td>
        <td data-outcome-code>V</td>
        <td data-outcome-id="101080928" data-market-name="Gólszám 2,5">
          <div data-odds>2.31</div></td>
      </tr>
      <tr data-event="5621252" data-market><th class="tleft">Hendikep 0:1</th></tr>
      <tr data-event="5621252" data-market>
        <td data-market-number>16951</td><td><div data-outcome-name>Azam</div></td>
        <td data-outcome-code>H</td>
        <td data-outcome-id="101080935" data-market-name="Hendikep 0:1">
          <div data-odds>6.00</div></td>
      </tr>
      <tr data-event="5621252" data-market>
        <td data-market-number>16951</td><td><div data-outcome-name>Döntetlen</div></td>
        <td data-outcome-code>D</td>
        <td data-outcome-id="101080936" data-market-name="Hendikep 0:1">
          <div data-odds>3.95</div></td>
      </tr>"#;
    let payload = format!(
        "{{\"desktop\":{},\"mobile\":\"\"}}",
        serde_json::json!(desktop)
    );
    let mut context = ctx("2026-11-27");
    context.request_key = "tipplap:markets:5621252".into();

    let obs = TipplapAdapter
        .parse(&raw(payload.as_bytes()), &context)
        .unwrap();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].aspect, "match_odds");
    assert_eq!(obs[0].source_record_key, "tipplap-5621252");
    assert_eq!(obs[0].payload["market_count"], 4);
    assert_eq!(obs[0].payload["quotes"][0]["market"], "btts");
    assert_eq!(obs[0].payload["quotes"][0]["outcome"], "yes");
    assert_eq!(obs[0].payload["quotes"][2]["market"], "ou");
    assert_eq!(obs[0].payload["quotes"][2]["outcome"], "over");
    assert_eq!(obs[0].payload["quotes"][2]["line"], 2.5);

    // A hendikep negatív vonala nem mehet a DB-be: az előjelet a market viszi.
    let handicap: Vec<_> = obs[0].payload["quotes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|q| q["market"].as_str().unwrap_or("").starts_with("handicap_"))
        .collect();
    assert_eq!(handicap.len(), 2);
    assert!(
        handicap
            .iter()
            .all(|q| q["line"].as_f64().unwrap_or(-1.0) >= 0.0)
    );
    assert!(
        handicap[0]["market"]
            .as_str()
            .unwrap()
            .starts_with("handicap_-1")
    );

    // A `fact.odds_quote` PK (market, outcome, line, phase) egyedi kell legyen.
    let keys: Vec<String> = obs[0].payload["quotes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            format!(
                "{}|{}|{}|{}",
                q["market"].as_str().unwrap_or(""),
                q["outcome"].as_str().unwrap_or(""),
                q["line"],
                q["phase"].as_str().unwrap_or("")
            )
        })
        .collect();
    let unique: std::collections::HashSet<&String> = keys.iter().collect();
    assert_eq!(unique.len(), keys.len());
}
