import XCTest
import FootballAppCore
@testable import FootballApp

/// S45: a nap-szűrők (hiányzó kezdésidő, lejárt eredmény nélküli meccs,
/// ország/osztály/női szűrők) viselkedésének egységtesztjei.
final class MatchFilterTests: XCTestCase {
    private func decode(_ json: String) throws -> [DayMatch] {
        let data = json.data(using: .utf8)!
        return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data).matches
    }

    private func match(id: String, kickoff: String, status: String,
                       precision: String? = "exact") -> DayMatch {
        // A JSON-dekódolást használjuk, hogy a DayMatch init belső maradhasson.
        let precisionField = precision.map { "\"kickoff_precision\":\"\($0)\"," } ?? ""
        let json = """
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"\(id)","kickoff_at":"\(kickoff)",\(precisionField)
           "status":"\(status)","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_name":"Teszt Liga","country_code":"HUN","country_name":"Hungary","tier":1}
        ]}
        """
        return try! decode(json)[0]
    }

    func testMissingKickoffSelectsOnlyDateOnlyMatches() {
        let dateOnly = match(id: "00000000-0000-0000-0000-000000000001",
                             kickoff: "2026-11-20T13:00:00Z", status: "scheduled",
                             precision: "date_only")
        let exact = match(id: "00000000-0000-0000-0000-000000000002",
                          kickoff: "2026-11-20T18:00:00Z", status: "scheduled")
        XCTAssertTrue(MatchFilter.missingKickoff.matches(dateOnly))
        XCTAssertFalse(MatchFilter.missingKickoff.matches(exact))
        // A hiány-szűrők a státusz-szűrő logikát nem használják.
        XCTAssertFalse(MatchFilter.missingKickoff.matches(status: "scheduled"))
    }

    func testMissingResultSelectsPastUnfinishedMatchesOnly() {
        let now = ISO8601DateFormatter().date(from: "2026-11-21T12:00:00Z")!
        let oldScheduled = match(id: "00000000-0000-0000-0000-000000000003",
                                 kickoff: "2026-11-20T18:00:00Z", status: "scheduled")
        let oldFinished = match(id: "00000000-0000-0000-0000-000000000004",
                                kickoff: "2026-11-20T18:00:00Z", status: "finished")
        let recentScheduled = match(id: "00000000-0000-0000-0000-000000000005",
                                    kickoff: "2026-11-21T11:00:00Z", status: "scheduled")
        let oldPostponed = match(id: "00000000-0000-0000-0000-000000000006",
                                 kickoff: "2026-11-20T18:00:00Z", status: "postponed")
        XCTAssertTrue(MatchFilter.missingResult.matches(oldScheduled, now: now))
        XCTAssertFalse(MatchFilter.missingResult.matches(oldFinished, now: now))
        XCTAssertFalse(MatchFilter.missingResult.matches(recentScheduled, now: now))
        XCTAssertFalse(MatchFilter.missingResult.matches(oldPostponed, now: now))
    }

    func testLeagueGroupDetectsCupsAndWomensLeagues() throws {
        let cup = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000011","kickoff_at":"2026-11-20T18:00:00Z",
           "status":"scheduled","home":"A","away":"B","publish_state":"verified_publish","blockers":[],
           "competition_name":"FA Cup","country_code":"GB-ENG","country_name":"England"}
        ]}
        """)
        let womens = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000012","kickoff_at":"2026-11-20T18:00:00Z",
           "status":"scheduled","home":"A (W)","away":"B (W)","publish_state":"verified_publish","blockers":[],
           "competition_name":"A-League (női)","country_code":"AU","country_name":"Australia"}
        ]}
        """)
        XCTAssertTrue(LeagueGroup.fromDay(cup)[0].isCup)
        XCTAssertFalse(LeagueGroup.fromDay(cup)[0].isWomens)
        XCTAssertTrue(LeagueGroup.fromDay(womens)[0].isWomens)
        XCTAssertFalse(LeagueGroup.fromDay(womens)[0].isCup)
    }
}
