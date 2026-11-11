import XCTest
import FootballAppCore
@testable import FootballApp

final class LeagueGroupTests: XCTestCase {
    private func decode(_ json: String) throws -> [DayMatch] {
        let data = json.data(using: .utf8)!
        return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data).matches
    }

    func testGroupsSortByPriorityThenCountry() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6},
          {"id":"00000000-0000-0000-0000-000000000002","kickoff_at":"2026-11-11T16:00:00Z",
           "status":"scheduled","home":"C","away":"D","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a2","competition_name":"Premier League",
           "country_code":"ENG","country_name":"England","tier":1,"priority":1},
          {"id":"00000000-0000-0000-0000-000000000003","kickoff_at":"2026-11-11T19:00:00Z",
           "status":"scheduled","home":"E","away":"F","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a2","competition_name":"Premier League",
           "country_code":"ENG","country_name":"England","tier":1,"priority":1}
        ]}
        """)
        let groups = LeagueGroup.fromDay(matches)
        XCTAssertEqual(groups.count, 2)
        XCTAssertEqual(groups[0].name, "Premier League")
        XCTAssertEqual(groups[0].matches.count, 2)
        XCTAssertEqual(groups[0].country, "Anglia")
        XCTAssertEqual(groups[1].name, "NB I")
        XCTAssertEqual(groups[1].country, "Magyarország")
    }

    func testCardModelCarriesDisplayFields() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"finished","home":"Újpest","away":"Győr","home_score":2,"away_score":1,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6,
           "home_ht":1,"away_ht":0,"home_position":8,"away_position":6,
           "odds":{"home":2.2,"draw":3.3,"away":3.2,"btts_yes":1.85,"btts_no":1.95,"over25":2.15,"under25":1.7},
           "market_count":18,"odds_source":"Demo odds feed"}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertEqual(card.home, "Újpest")
        XCTAssertEqual(card.homeScore, 2)
        XCTAssertEqual(card.homeHt, 1)
        XCTAssertEqual(card.homePosition, 8)
        XCTAssertEqual(card.awayPosition, 6)
        XCTAssertEqual(card.odds?.home, 2.2)
        XCTAssertEqual(card.markets, 18)
        XCTAssertEqual(card.oddsSource, "Demo odds feed")
        XCTAssertEqual(card.winner1x2, .home)
        XCTAssertEqual(card.bttsYesWon, true)
        XCTAssertEqual(card.over25Won, true)
        XCTAssertNil(card.minute)
    }

    func testUnknownCountryFallsBackToName() {
        let display = CountryDisplay.display(code: "XYZ", name: "Ismeretlen ország")
        XCTAssertEqual(display.name, "Ismeretlen ország")
        XCTAssertEqual(display.flag, "🏳️")
    }
}