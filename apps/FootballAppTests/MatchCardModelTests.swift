import XCTest
import FootballAppCore
@testable import FootballApp

final class MatchCardModelTests: XCTestCase {
    private func decode(_ json: String) throws -> [DayMatch] {
        let data = json.data(using: .utf8)!
        return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data).matches
    }

    private var budapest: TimeZone { TimeZone(identifier: "Europe/Budapest")! }

    private func utcDate(_ year: Int, _ month: Int, _ day: Int, _ hour: Int, _ minute: Int) -> Date {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "UTC")!
        let components = DateComponents(year: year, month: month, day: day,
                                        hour: hour, minute: minute)
        return calendar.date(from: components)!
    }

    // MARK: - Kezdés formázása

    func testKickoffFormattingIsHungarianDayAndTime() {
        let kickoff = utcDate(2026, 11, 20, 17, 0)
        XCTAssertEqual(MatchCardModel.dayLabel(for: kickoff, timeZone: budapest), "nov. 20.")
        XCTAssertEqual(MatchCardModel.timeLabel(for: kickoff, timeZone: budapest), "18:00")
    }

    func testTimeLabelIsZeroPadded() {
        let kickoff = utcDate(2026, 1, 5, 8, 5)
        XCTAssertEqual(MatchCardModel.timeLabel(for: kickoff, timeZone: budapest), "09:05")
        XCTAssertEqual(MatchCardModel.dayLabel(for: kickoff, timeZone: budapest), "jan. 5.")
    }

    func testCardCarriesDisplayableKickoff() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000201","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"scheduled","home":"Ferencváros","away":"Újpest",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertEqual(card.dayLabel, MatchCardModel.dayLabel(for: matches[0].kickoffAt))
        XCTAssertEqual(card.time, MatchCardModel.timeLabel(for: matches[0].kickoffAt))
        XCTAssertTrue(card.dayLabel.hasSuffix("."))
        XCTAssertNotNil(card.time.range(of: #"^\d{2}:\d{2}$"#, options: .regularExpression))
    }

    // MARK: - Eredmény

    func testFinalScorePrefersExtraTimeLikeDomainFinalScore() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000202","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"finished","home":"A","away":"B",
           "home_score":1,"away_score":1,"home_ht":0,"away_ht":0,
           "home_et":2,"away_et":1,"publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"Kupa",
           "country_code":"HUN","country_name":"Hungary","tier":0,"priority":3}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertEqual(card.finalScore, MatchScore(home: 2, away: 1))
        XCTAssertEqual(card.scoreText, "2 – 1")
    }

    func testFinalScoreFallsBackToFullTimeAndUsesDashWhenMissing() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000203","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"finished","home":"A","away":"B",
           "home_score":3,"away_score":0,"publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6},
          {"id":"00000000-0000-0000-0000-000000000204","kickoff_at":"2026-11-20T18:00:00Z",
           "status":"scheduled","home":"C","away":"D",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c2","competition_name":"NB II",
           "country_code":"HUN","country_name":"Hungary","tier":2,"priority":7}
        ]}
        """)
        let played = MatchCardModel.from(matches[0])
        XCTAssertEqual(played.finalScore, MatchScore(home: 3, away: 0))
        XCTAssertEqual(played.scoreText, "3 – 0")

        let upcoming = MatchCardModel.from(matches[1])
        XCTAssertNil(upcoming.finalScore)
        XCTAssertEqual(upcoming.scoreText, "–")
    }

    // MARK: - 1X2 odds

    func testOddsPillsAreOnlyOneXTwoWithHungarianDecimalComma() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000205","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"finished","home":"A","away":"B","home_score":2,"away_score":0,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6,
           "odds":{"home":1.85,"draw":3.4,"away":4.2,
                   "btts_yes":1.9,"btts_no":1.9,"over25":2.1,"under25":1.7},
           "market_count":18,"odds_source":"Demo odds feed"}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertTrue(card.hasOdds)
        XCTAssertEqual(card.oddsPills.map(\.label), ["1", "X", "2"])
        XCTAssertEqual(card.oddsPills.map(\.value), ["1,85", "3,40", "4,20"])
        XCTAssertEqual(card.oddsPills.map(\.isWinner), [true, false, false])
        XCTAssertEqual(card.oddsAccessibilityText, "1: 1,85, X: 3,40, 2: 4,20")
    }

    func testMissingOddsHideTheWholeBlock() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000206","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertFalse(card.hasOdds)
        XCTAssertTrue(card.oddsPills.isEmpty)
        XCTAssertNil(card.oddsAccessibilityText)
    }

    // MARK: - Állapot

    func testLiveCardShowsLivePhaseAndRunningScore() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000207","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"live","home":"A","away":"B","home_score":1,"away_score":0,
           "publish_state":"provisional_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertTrue(card.isLive)
        XCTAssertFalse(card.isFinished)
        XCTAssertEqual(card.phaseLabel, "ÉLŐ")
        XCTAssertEqual(card.scoreText, "1 – 0")
    }

    func testFinishedCardShowsFullTimeMarker() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000208","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"finished","home":"A","away":"B","home_score":0,"away_score":0,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertTrue(card.isFinished)
        XCTAssertEqual(card.phaseLabel, "FT")
        XCTAssertEqual(card.scoreText, "0 – 0")
    }
    func testDateOnlyKickoffHidesTheTimeAndShowsANote() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000301","kickoff_at":"2026-11-20T12:00:00Z",
           "kickoff_precision":"date_only",
           "status":"scheduled","home":"Fluminense","away":"Sao Paulo",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[]}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertFalse(card.hasExactKickoff)
        XCTAssertEqual(card.timeDisplay, "")
        XCTAssertEqual(card.kickoffNote, "időpont egyeztetés alatt")
    }

    func testNumericRoundLabelBecomesHungarianRound() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000302","kickoff_at":"2026-11-20T17:00:00Z",
           "round_label":"17",
           "status":"scheduled","home":"A","away":"B",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[]},
          {"id":"00000000-0000-0000-0000-000000000303","kickoff_at":"2026-11-20T18:00:00Z",
           "round_label":"final",
           "status":"scheduled","home":"C","away":"D",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[]}
        ]}
        """)
        XCTAssertEqual(MatchCardModel.from(matches[0]).roundText, "17. forduló")
        XCTAssertEqual(MatchCardModel.from(matches[1]).roundText, "Döntő")
    }

}