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

    /// S134: a research élő perce (`status.minute`) az API-n át a kártyáig jut,
    /// és felülírja a kezdésből számolt becslést.
    func testResearchMinuteReachesCardAndOverridesEstimate() throws {
        let kickoff = ISO8601DateFormatter().string(
            from: Date().addingTimeInterval(-20 * 60))
        let matches = try decode("""
        {"date":"2026-11-24","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000301","kickoff_at":"\(kickoff)",
           "status":"live","home":"Algeria U17","away":"Montenegro U17",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[],
           "minute":87,
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"U17-es VB",
           "country_code":null,"country_name":null,"tier":0,"priority":30}
        ]}
        """)
        XCTAssertEqual(matches[0].minute, 87)
        let card = MatchCardModel.from(matches[0])
        XCTAssertTrue(card.isLive)
        XCTAssertEqual(card.minuteText, "87'")
        XCTAssertEqual(card.livePhaseText, "2. félidő · 87'")
    }

    /// Régi válasz: nincs `minute` mező → a kártya a kezdésből becsül.
    func testMissingMinuteFallsBackToKickoffEstimate() throws {
        let kickoff = ISO8601DateFormatter().string(
            from: Date().addingTimeInterval(-20 * 60))
        let matches = try decode("""
        {"date":"2026-11-24","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000302","kickoff_at":"\(kickoff)",
           "status":"live","home":"A","away":"B",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000c1","competition_name":"Teszt",
           "country_code":null,"country_name":null,"tier":0,"priority":30}
        ]}
        """)
        XCTAssertNil(matches[0].minute)
        XCTAssertEqual(MatchCardModel.from(matches[0]).livePhaseText, "1. félidő · 20'")
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
    private func makeCard(status: String, kickoffAt: Date, minute: Int? = nil,
                          score: (home: Int, away: Int)? = (1, 0)) -> MatchCardModel {
        MatchCardModel(
            id: "test-card", home: "A", away: "B", kickoffAt: kickoffAt,
            dayLabel: "nov. 20.", time: "18:00", kickoffPrecision: "exact",
            roundLabel: nil, status: status, publishState: .provisionalPublish,
            homeScore: score?.home, awayScore: score?.away, homeHt: nil, awayHt: nil, homeEt: nil, awayEt: nil,
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
        XCTAssertEqual(card.livePhaseText, "1. félidő · 34'")
        XCTAssertEqual(card.displayStatus, "live")
        XCTAssertEqual(card.scoreText, "1 – 0")
    }

    /// S129: ha a forrás nem ad minute-ot, a kártya a kezdésből számol
    /// (45 perc félidő, 15 perc szünet, 45 perc, majd várakozás).
    func testLivePhaseFromKickoffWithoutMinute() {
        func card(startedMinutesAgo: Int) -> MatchCardModel {
            makeCard(status: "live",
                     kickoffAt: Date().addingTimeInterval(Double(-startedMinutesAgo * 60)),
                     minute: nil)
        }
        XCTAssertEqual(card(startedMinutesAgo: 20).livePhaseText, "1. félidő · 20'")
        XCTAssertEqual(card(startedMinutesAgo: 50).livePhaseText, "Félidő · 15' szünet")
        XCTAssertEqual(card(startedMinutesAgo: 80).livePhaseText, "2. félidő · 65'")
        XCTAssertEqual(card(startedMinutesAgo: 110).livePhaseText, "Várakozás az eredményre")
        // A második félidőben a forrás minute-ja felülírja a becslést.
        let withMinute = makeCard(status: "live",
                                  kickoffAt: Date().addingTimeInterval(-80 * 60),
                                  minute: 72)
        XCTAssertEqual(withMinute.livePhaseText, "2. félidő · 72'")
    }

    /// Elavult `live` státusz (a kezdés több mint 150 perccel ezelőtt): a kártya
    /// nem ír „ÉLŐ"-t, hanem azt, hogy az eredmény még várható (S130).
    func testStaleLiveCardShowsResultPending() {
        let card = makeCard(status: "live",
                            kickoffAt: Date().addingTimeInterval(-202 * 60),
                            minute: nil, score: nil)
        XCTAssertFalse(card.isLive)
        XCTAssertTrue(card.isResultPending)
        XCTAssertEqual(card.phaseLabel, "EREDMÉNY VÁRHATÓ")
        XCTAssertEqual(card.livePhaseText, "Várakozás az eredményre")
        XCTAssertNil(card.minuteText)
        XCTAssertEqual(card.displayStatus, "result_pending")
        XCTAssertEqual(card.scoreText, "–")

        // Ha közben megjön az eredmény, már nem „várható".
        let withScore = makeCard(status: "live",
                                 kickoffAt: Date().addingTimeInterval(-202 * 60),
                                 minute: nil, score: (2, 1))
        XCTAssertFalse(withScore.isResultPending)
        XCTAssertEqual(withScore.scoreText, "2 – 1")
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

    /// S152: a „Round 15" és a „15" ugyanazt a fordulót jelenti — a kliens
    /// korábban két külön címkének nézte és ezért elrejtette a fordulót.
    /// A „round of 16" viszont nyolcaddöntő marad.
    func testRoundPrefixIsFoldedOnlyBeforeDigits() {
        XCTAssertEqual(MatchCardModel.foldedRoundLabel("Round 15"), "15")
        XCTAssertEqual(MatchCardModel.foldedRoundLabel("15"), "15")
        XCTAssertEqual(MatchCardModel.roundText(for: "Round 15"), "15. forduló")
        XCTAssertEqual(MatchCardModel.foldedRoundLabel("Forduló 15"), "15")
        XCTAssertEqual(MatchCardModel.foldedRoundLabel("Round of 16"), "round of 16")
        XCTAssertEqual(MatchCardModel.roundText(for: "Round of 16"), "Nyolcaddöntő")
        XCTAssertEqual(MatchCardModel.foldedRoundLabel("Group A / 2"), "group a / 2")
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

    func testLiveWindowIs150Minutes() {
        let now = utcDate(2026, 11, 21, 12, 0)
        XCTAssertEqual(MatchCardModel.liveWindow, 150 * 60)
        XCTAssertTrue(MatchCardModel.isLiveNow(
            status: "live", kickoffAt: now.addingTimeInterval(-2 * 3600), now: now))
        XCTAssertTrue(MatchCardModel.isLiveNow(
            status: "halftime", kickoffAt: now.addingTimeInterval(-1 * 3600), now: now))
        // 202 perc eltelt (14:45-i meccs 18:07-kor) — már nem élő.
        XCTAssertFalse(MatchCardModel.isLiveNow(
            status: "live", kickoffAt: now.addingTimeInterval(-202 * 60), now: now))
        XCTAssertFalse(MatchCardModel.isLiveNow(
            status: "scheduled", kickoffAt: now.addingTimeInterval(-1 * 3600), now: now))
    }

}