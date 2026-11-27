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
                       precision: String? = "exact",
                       homeScore: Int? = nil, awayScore: Int? = nil) -> DayMatch {
        // A JSON-dekódolást használjuk, hogy a DayMatch init belső maradhasson.
        let precisionField = precision.map { "\"kickoff_precision\":\"\($0)\"," } ?? ""
        let homeScoreField = homeScore.map(String.init) ?? "null"
        let awayScoreField = awayScore.map(String.init) ?? "null"
        let json = """
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"\(id)","kickoff_at":"\(kickoff)",\(precisionField)
           "status":"\(status)","home":"A","away":"B",
           "home_score":\(homeScoreField),"away_score":\(awayScoreField),
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

    /// „Eredmény nélkül" = lezárt (`finished`), de nincs végeredménye. Nem a
    /// lejárt, nyitott meccseket jelenti — azoknak külön szűrőjük van.
    func testMissingResultSelectsFinishedMatchesWithoutFinalScoreOnly() {
        let now = ISO8601DateFormatter().date(from: "2026-11-21T12:00:00Z")!
        let oldScheduled = match(id: "00000000-0000-0000-0000-000000000003",
                                 kickoff: "2026-11-20T18:00:00Z", status: "scheduled")
        let finishedWithoutScore = match(id: "00000000-0000-0000-0000-000000000004",
                                         kickoff: "2026-11-20T18:00:00Z", status: "finished")
        let finishedWithScore = match(id: "00000000-0000-0000-0000-000000000005",
                                      kickoff: "2026-11-20T18:00:00Z", status: "finished",
                                      homeScore: 2, awayScore: 1)
        let recentFinishedWithoutScore = match(id: "00000000-0000-0000-0000-000000000006",
                                               kickoff: "2026-11-21T11:00:00Z", status: "finished")
        let oldPostponed = match(id: "00000000-0000-0000-0000-000000000007",
                                 kickoff: "2026-11-20T18:00:00Z", status: "postponed")
        XCTAssertTrue(MatchFilter.missingResult.matches(finishedWithoutScore, now: now))
        XCTAssertTrue(MatchFilter.missingResult.matches(recentFinishedWithoutScore, now: now))
        XCTAssertFalse(MatchFilter.missingResult.matches(finishedWithScore, now: now))
        XCTAssertFalse(MatchFilter.missingResult.matches(oldScheduled, now: now))
        XCTAssertFalse(MatchFilter.missingResult.matches(oldPostponed, now: now))
    }

    /// „Lejárt, nyitott" = nyitott státusz, de a kezdés már több mint 10 perce volt;
    /// az élő meccsek addig az „Élő" szűrőben maradnak (150 perces ablak).
    func testOpenPastSelectsExpiredOpenStatusesOnly() {
        let now = ISO8601DateFormatter().date(from: "2026-11-21T12:00:00Z")!
        let recentScheduled = match(id: "00000000-0000-0000-0000-000000000011",
                                    kickoff: "2026-11-21T11:55:00Z", status: "scheduled")
        let twelveMinutesAgo = match(id: "00000000-0000-0000-0000-000000000016",
                                     kickoff: "2026-11-21T11:48:00Z", status: "scheduled")
        let liveWithinWindow = match(id: "00000000-0000-0000-0000-000000000017",
                                     kickoff: "2026-11-21T10:00:00Z", status: "live")
        let oldScheduled = match(id: "00000000-0000-0000-0000-000000000012",
                                 kickoff: "2026-11-21T08:00:00Z", status: "scheduled")
        let oldLive = match(id: "00000000-0000-0000-0000-000000000013",
                            kickoff: "2026-11-21T07:00:00Z", status: "live")
        let oldFinished = match(id: "00000000-0000-0000-0000-000000000014",
                                kickoff: "2026-11-21T08:00:00Z", status: "finished")
        let oldPostponed = match(id: "00000000-0000-0000-0000-000000000015",
                                 kickoff: "2026-11-21T08:00:00Z", status: "postponed")
        XCTAssertTrue(MatchFilter.openPast.matches(oldScheduled, now: now))
        XCTAssertTrue(MatchFilter.openPast.matches(oldLive, now: now))
        XCTAssertTrue(MatchFilter.openPast.matches(twelveMinutesAgo, now: now))
        XCTAssertFalse(MatchFilter.openPast.matches(liveWithinWindow, now: now))
        XCTAssertFalse(MatchFilter.openPast.matches(recentScheduled, now: now))
        XCTAssertFalse(MatchFilter.openPast.matches(oldFinished, now: now))
        XCTAssertFalse(MatchFilter.openPast.matches(oldPostponed, now: now))
    }

    /// Az „Élő" szűrő a státusz mellett legfeljebb 150 perces kezdés-ablakot enged
    /// (45 + 15 + 45 + hosszabbítás; a backend stale-live védelem kliensoldali párja).
    /// S212: az elhalasztott meccs külön szűrőben van, nem a „Következik"-ben.
    func testPostponedIsSeparateFromUpcoming() {
        let now = ISO8601DateFormatter().date(from: "2026-11-21T12:00:00Z")!
        let future = match(id: "00000000-0000-0000-0000-0000000000a1",
                           kickoff: "2026-11-21T18:00:00Z", status: "postponed")
        let past = match(id: "00000000-0000-0000-0000-0000000000a2",
                         kickoff: "2026-11-20T18:00:00Z", status: "postponed")
        let scheduled = match(id: "00000000-0000-0000-0000-0000000000a3",
                              kickoff: "2026-11-21T18:00:00Z", status: "scheduled")
        XCTAssertTrue(MatchFilter.postponed.matches(future, now: now))
        XCTAssertTrue(MatchFilter.postponed.matches(past, now: now))
        XCTAssertFalse(MatchFilter.upcoming.matches(future, now: now))
        XCTAssertFalse(MatchFilter.upcoming.matches(past, now: now))
        XCTAssertFalse(MatchFilter.openPast.matches(past, now: now))
        XCTAssertFalse(MatchFilter.postponed.matches(scheduled, now: now))
        XCTAssertTrue(MatchFilter.homeFilters.contains(.postponed))
    }

    func testLiveFilterRequiresLiveStatusWithin150Minutes() {
        let now = ISO8601DateFormatter().date(from: "2026-11-21T12:00:00Z")!
        let liveRecent = match(id: "00000000-0000-0000-0000-000000000021",
                               kickoff: "2026-11-21T11:00:00Z", status: "live")
        let liveStale = match(id: "00000000-0000-0000-0000-000000000022",
                              kickoff: "2026-11-21T04:00:00Z", status: "live")
        let halftimeRecent = match(id: "00000000-0000-0000-0000-000000000023",
                                   kickoff: "2026-11-21T11:00:00Z", status: "halftime")
        let scheduledRecent = match(id: "00000000-0000-0000-0000-000000000024",
                                    kickoff: "2026-11-21T11:00:00Z", status: "scheduled")
        XCTAssertTrue(MatchFilter.live.matches(liveRecent, now: now))
        XCTAssertTrue(MatchFilter.live.matches(halftimeRecent, now: now))
        XCTAssertFalse(MatchFilter.live.matches(liveStale, now: now))
        XCTAssertFalse(MatchFilter.live.matches(scheduledRecent, now: now))
    }

    /// S131: a „Következik" nem mutathat lejárt meccset csak azért, mert a
    /// forrás még mindig `scheduled` státuszt ad (pl. 16:15-ös frissítéskor
    /// 16:00-kor kezdett meccs 18:12-kor még mindig scheduled).
    func testUpcomingRequiresFutureKickoff() {
        let now = ISO8601DateFormatter().date(from: "2026-11-24T17:12:00Z")!
        func matchAt(_ offsetMinutes: Int, status: String = "scheduled",
                     suffix: String) -> DayMatch {
            let kickoff = now.addingTimeInterval(Double(offsetMinutes * 60))
            let text = ISO8601DateFormatter().string(from: kickoff)
            return match(id: "00000000-0000-0000-0000-000000000\(suffix)",
                         kickoff: text, status: status)
        }
        // Jövőbeli kezdés: következik.
        XCTAssertTrue(MatchFilter.upcoming.matches(matchAt(30, suffix: "031"), now: now))
        // 5 perces tűréshatár a forráskésés miatt.
        XCTAssertTrue(MatchFilter.upcoming.matches(matchAt(-2, suffix: "032"), now: now))
        // A felhasználó esete: 15:00Z (16:00 Budapest), 132 perce kezdődött.
        XCTAssertFalse(MatchFilter.upcoming.matches(matchAt(-132, suffix: "033"), now: now))
        // Túl a tűréshatáron, de még a meccs közben.
        XCTAssertFalse(MatchFilter.upcoming.matches(matchAt(-6, suffix: "034"), now: now))
        // Elhalasztott meccs múlt idejű eredeti időponttal szintén nem következik.
        XCTAssertFalse(MatchFilter.upcoming.matches(
            matchAt(-989, status: "postponed", suffix: "035"), now: now))
        // A státuszellenőrzés továbbra is él: lezárt meccs jövőbeli idővel nem az.
        XCTAssertFalse(MatchFilter.upcoming.matches(
            matchAt(30, status: "finished", suffix: "036"), now: now))
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

    func testMissingOddsSelectsMatchesWithoutOdds() throws {
        let json = """
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000011","kickoff_at":"2026-11-21T18:00:00Z",
           "kickoff_precision":"exact","status":"scheduled","home":"A","away":"B",
           "publish_state":"verified_publish","blockers":[],
           "competition_name":"Teszt Liga","country_code":"HUN","country_name":"Hungary","tier":1,
           "odds":{"home":2.1,"draw":3.4,"away":3.2}},
          {"id":"00000000-0000-0000-0000-000000000012","kickoff_at":"2026-11-21T20:00:00Z",
           "kickoff_precision":"exact","status":"scheduled","home":"C","away":"D",
           "publish_state":"verified_publish","blockers":[],
           "competition_name":"Teszt Liga","country_code":"HUN","country_name":"Hungary","tier":1}
        ]}
        """
        let matches = try decode(json)
        let ids = matches.filter { MatchFilter.missingOdds.matches($0) }.map(\.home)
        XCTAssertEqual(ids, ["C"])
    }

    func testMissingLogoSelectsMatchesLackingEitherCrest() throws {
        let json = """
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000021","kickoff_at":"2026-11-21T18:00:00Z",
           "kickoff_precision":"exact","status":"scheduled","home":"A","away":"B",
           "home_logo":"/v1/asset/a","away_logo":"/v1/asset/b","publish_state":"verified_publish",
           "blockers":[],"competition_name":"Teszt Liga","country_code":"HUN","country_name":"Hungary","tier":1},
          {"id":"00000000-0000-0000-0000-000000000022","kickoff_at":"2026-11-21T20:00:00Z",
           "kickoff_precision":"exact","status":"scheduled","home":"C","away":"D",
           "home_logo":"/v1/asset/c","publish_state":"verified_publish",
           "blockers":[],"competition_name":"Teszt Liga","country_code":"HUN","country_name":"Hungary","tier":1}
        ]}
        """
        let matches = try decode(json)
        let ids = matches.filter { MatchFilter.missingLogo.matches($0) }.map(\.home)
        XCTAssertEqual(ids, ["C"])
    }
}
