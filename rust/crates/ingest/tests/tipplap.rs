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
