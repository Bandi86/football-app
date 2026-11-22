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

    /// Közvetlen kártyaépítés az időfüggő állapotokhoz: a napi feed nem hoz
    /// `minute` mezőt, ezért a perc-kijelzést így tudjuk determinisztikusan
    /// vizsgálni.
    private func makeCard(status: String, kickoffAt: Date, minute: Int? = nil) -> MatchCardModel {
        MatchCardModel(
            id: "test-card", home: "A", away: "B", kickoffAt: kickoffAt,
            dayLabel: "nov. 20.", time: "18:00", kickoffPrecision: "exact",
            roundLabel: nil, status: status, publishState: .provisionalPublish,
            homeScore: 1, awayScore: 0, homeHt: nil, awayHt: nil, homeEt: nil, awayEt: nil,
            homePens: nil, awayPens: nil, minute: minute, odds: nil, markets: 0,
            oddsSource: "–", homePosition: nil, awayPosition: nil, homeLogo: nil,
            awayLogo: nil, competitionLogo: nil, homeElo: nil, awayElo: nil
        )
    }

    func testLiveCardShowsLivePhaseAndRunningScore() {
        let card = makeCard(status: "live",
                            kickoffAt: Date().addingTimeInterval(-1 * 60 * 60),
                            minute: 34)
        XCTAssertTrue(card.isLive)
        XCTAssertFalse(card.isFinished)
        XCTAssertEqual(card.phaseLabel, "ÉLŐ")
        XCTAssertEqual(card.minuteText, "34'")
        XCTAssertEqual(card.displayStatus, "live")
        XCTAssertEqual(card.scoreText, "1 – 0")
    }

    /// Elavult `live` státusz (a kezdés több mint 6 órával ezelőtt): a kártya
    /// nem ír „ÉLŐ"-t és nem mutat percet, csak a szokásos kezdést/eredményt.
    func testStaleLiveCardHidesLivePhaseAndMinute() {
        let card = makeCard(status: "live",
                            kickoffAt: Date().addingTimeInterval(-7 * 60 * 60),
                            minute: 87)
        XCTAssertFalse(card.isLive)
        XCTAssertNil(card.phaseLabel)
        XCTAssertNil(card.minuteText)
        // A felolvasott állapot sem „Élő", hanem a szokásos nem-élő kijelzés.
        XCTAssertEqual(card.displayStatus, "scheduled")
        XCTAssertFalse(card.time.isEmpty)
        XCTAssertEqual(card.timeDisplay, card.time)
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

    // MARK: - Hiány- és élő-állapot szabályai

    func testMissingResultMeansFinishedWithoutFinalScore() {
        XCTAssertTrue(MatchCardModel.isMissingResult(status: "finished", finalScore: nil))
        XCTAssertFalse(MatchCardModel.isMissingResult(
            status: "finished", finalScore: MatchScore(home: 1, away: 0)))
        XCTAssertFalse(MatchCardModel.isMissingResult(status: "scheduled", finalScore: nil))
        XCTAssertFalse(MatchCardModel.isMissingResult(status: "live", finalScore: nil))
    }

    func testOpenPastMeansOpenStatusOlderThanThreeHours() {
        let now = utcDate(2026, 11, 21, 12, 0)
        XCTAssertTrue(MatchCardModel.isOpenPast(
            status: "scheduled", kickoffAt: now.addingTimeInterval(-4 * 3600), now: now))
        XCTAssertTrue(MatchCardModel.isOpenPast(
            status: "live", kickoffAt: now.addingTimeInterval(-3.5 * 3600), now: now))
        XCTAssertFalse(MatchCardModel.isOpenPast(
            status: "scheduled", kickoffAt: now.addingTimeInterval(-2 * 3600), now: now))
        XCTAssertFalse(MatchCardModel.isOpenPast(
            status: "finished", kickoffAt: now.addingTimeInterval(-4 * 3600), now: now))
        XCTAssertFalse(MatchCardModel.isOpenPast(
            status: "canceled", kickoffAt: now.addingTimeInterval(-4 * 3600), now: now))
    }

    func testLiveWindowIsSixHours() {
        let now = utcDate(2026, 11, 21, 12, 0)
        XCTAssertEqual(MatchCardModel.liveWindow, 6 * 3600)
        XCTAssertTrue(MatchCardModel.isLiveNow(
            status: "live", kickoffAt: now.addingTimeInterval(-5 * 3600), now: now))
        XCTAssertTrue(MatchCardModel.isLiveNow(
            status: "halftime", kickoffAt: now.addingTimeInterval(-1 * 3600), now: now))
        XCTAssertFalse(MatchCardModel.isLiveNow(
            status: "live", kickoffAt: now.addingTimeInterval(-7 * 3600), now: now))
        XCTAssertFalse(MatchCardModel.isLiveNow(
            status: "scheduled", kickoffAt: now.addingTimeInterval(-1 * 3600), now: now))
    }

}