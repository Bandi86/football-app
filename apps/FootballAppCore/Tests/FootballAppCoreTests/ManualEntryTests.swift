import Foundation
import Testing
@testable import FootballAppCore

/// A teljes meccsszerkesztő payload- és szabálytesztjei. A HTTP-stubot
/// igénylő esetek a `HTTPDayServiceTests` sorozatában futnak (a globális
/// `StubURLProtocol` miatt), itt csak tiszta kódolás/dekódolás/szabály van.
@Suite struct ManualEntryTests {
    private static let resultKeys = [
        "home_ft", "away_ft",
        "home_ht", "away_ht",
        "home_et", "away_et",
        "home_pens", "away_pens",
    ]

    private func makeDate(
        _ year: Int,
        _ month: Int,
        _ day: Int,
        _ hour: Int,
        _ minute: Int,
        timeZone identifier: String
    ) throws -> Date {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try #require(TimeZone(identifier: identifier))
        return try #require(
            calendar.date(
                from: DateComponents(
                    year: year,
                    month: month,
                    day: day,
                    hour: hour,
                    minute: minute
                )
            )
        )
    }

    private func json(_ request: ManualEditRequest) throws -> [String: Any] {
        let data = try JSONEncoder().encode(request)
        return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    // MARK: - Kódolás

    @Test func editRequestEncodesAllEightResultFieldsWhenResultEdited() throws {
        let kickoff = try makeDate(2026, 11, 21, 18, 0, timeZone: "Europe/Budapest")
        let request = ManualEditRequest(
            note: "javítás",
            kickoffAt: kickoff,
            kickoffPrecision: "exact",
            status: "finished",
            homeFt: 2,
            awayFt: 1,
            decidedBy: "ft"
        )
        let payload = try json(request)

        #expect(request.includesResult)
        #expect(payload["home_ft"] as? Int == 2)
        #expect(payload["away_ft"] as? Int == 1)
        #expect(payload["decided_by"] as? String == "ft")
        // Az eredmény-blokk cseréje miatt a hiányzó mezők explicit null-ok.
        for key in Self.resultKeys where key != "home_ft" && key != "away_ft" {
            #expect(payload[key] is NSNull, "\(key) nem explicit null")
        }
        #expect(payload["kickoff_at"] as? String == "2026-11-21T17:00:00Z")
        #expect(payload["kickoff_precision"] as? String == "exact")
        #expect(payload["status"] as? String == "finished")
        #expect(payload["note"] as? String == "javítás")
    }

    @Test func editRequestWithoutResultOmitsEveryResultKey() throws {
        let kickoff = try makeDate(2026, 11, 21, 18, 0, timeZone: "Europe/Budapest")
        let request = ManualEditRequest(
            note: "kezdés módosítása",
            kickoffAt: kickoff,
            kickoffPrecision: "date_only",
            status: "canceled"
        )
        let payload = try json(request)

        #expect(!request.includesResult)
        for key in Self.resultKeys + ["decided_by"] {
            #expect(payload[key] == nil, "\(key) nem maradhat a payloadban")
        }
        #expect(payload["kickoff_precision"] as? String == "date_only")
        #expect(payload["status"] as? String == "canceled")
        #expect(payload["note"] as? String == "kezdés módosítása")
    }

    @Test func legacyManualResultRequestStaysUnchanged() throws {
        let data = try JSONEncoder().encode(
            ManualResultRequest(homeFt: 3, awayFt: 3, note: "régi kliens")
        )
        let payload = try #require(
            try JSONSerialization.jsonObject(with: data) as? [String: Any]
        )
        #expect(payload.keys.sorted() == ["away_ft", "home_ft", "note"])
        #expect(payload["home_ft"] as? Int == 3)
        #expect(payload["away_ft"] as? Int == 3)
    }

    // MARK: - Kezdés

    @Test func dateOnlyPlaceholderIsNoonUTCOnTheUTCDay() throws {
        let kickoff = try makeDate(2026, 11, 21, 23, 30, timeZone: "Europe/Budapest")
        let placeholder = ManualEditRules.dateOnlyPlaceholder(for: kickoff)
        #expect(placeholder.formatted(.iso8601) == "2026-11-21T12:00:00Z")
    }

    @Test func makeRequestNormalizesDateOnlyKickoff() throws {
        let kickoff = try makeDate(2026, 11, 21, 18, 0, timeZone: "Europe/Budapest")
        let request = ManualEditRules.makeRequest(
            status: "scheduled",
            kickoffAt: kickoff,
            kickoffPrecision: "date_only",
            homeFt: "",
            awayFt: "",
            homeHt: "",
            awayHt: "",
            homeEt: "",
            awayEt: "",
            homePens: "",
            awayPens: "",
            note: "kezdés"
        )
        #expect(request.kickoffAt?.formatted(.iso8601) == "2026-11-21T12:00:00Z")
        #expect(!request.includesResult)
    }

    @Test func kickoffTimeZoneFollowsPrecisionAndCompetition() {
        #expect(
            ManualEditRules.kickoffTimeZone(
                competitionTimezone: "Europe/Budapest",
                precision: "exact"
            ) == TimeZone(identifier: "Europe/Budapest")
        )
        #expect(
            ManualEditRules.kickoffTimeZone(
                competitionTimezone: "Europe/Budapest",
                precision: "date_only"
            ) == .gmt
        )
        #expect(
            ManualEditRules.kickoffTimeZone(competitionTimezone: nil, precision: "exact") == .current
        )
    }

    // MARK: - Szerverhiba-dekódolás

    @Test func extractsServerErrorMessageFromErrorBody() {
        let body = Data(#"{"code":"BAD_REQUEST","message":"  A hazai gól nem lehet negatív.  "}"#.utf8)
        #expect(APIDecoding.serverErrorMessage(from: body) == "A hazai gól nem lehet negatív.")
        #expect(APIDecoding.serverError(from: body)?.code == "BAD_REQUEST")
    }

    @Test func serverErrorMessageIsNilWithoutUsableMessage() {
        #expect(APIDecoding.serverErrorMessage(from: Data(#"{"code":"BAD_REQUEST"}"#.utf8)) == nil)
        #expect(APIDecoding.serverErrorMessage(from: Data(#"{"code":"X","message":"   "}"#.utf8)) == nil)
        #expect(APIDecoding.serverErrorMessage(from: Data()) == nil)
        #expect(APIDecoding.serverErrorMessage(from: Data("nem json".utf8)) == nil)
    }

    // MARK: - Szabályok

    @Test func nineStatusesAndResultOnlyForFinishedOrAwarded() {
        #expect(ManualEditRules.statuses.count == 9)
        for status in ["finished", "awarded"] {
            #expect(ManualEditRules.allowsResult(for: status))
        }
        for status in ["scheduled", "confirmed", "live", "halftime", "postponed", "canceled", "abandoned"] {
            #expect(!ManualEditRules.allowsResult(for: status))
        }
    }

    @Test func awardedShowOnlyFullTime() {
        #expect(!ManualEditRules.showsHalftime(for: "awarded"))
        #expect(!ManualEditRules.showsExtraTime(for: "awarded", homeFt: 1, awayFt: 1))
        #expect(!ManualEditRules.showsPenalties(
            for: "awarded", homeFt: 1, awayFt: 1, homeEt: 2, awayEt: 2
        ))
    }

    @Test func extraTimeAndPenaltiesAppearOnlyAfterDraws() {
        #expect(ManualEditRules.showsHalftime(for: "finished"))
        #expect(ManualEditRules.showsExtraTime(for: "finished", homeFt: 1, awayFt: 1))
        #expect(!ManualEditRules.showsExtraTime(for: "finished", homeFt: 2, awayFt: 1))
        #expect(ManualEditRules.showsPenalties(
            for: "finished", homeFt: 1, awayFt: 1, homeEt: 2, awayEt: 2
        ))
        #expect(!ManualEditRules.showsPenalties(
            for: "finished", homeFt: 1, awayFt: 1, homeEt: 2, awayEt: 1
        ))
    }

    @Test func decidedByIsComputedClientSide() {
        #expect(ManualEditRules.decidedBy(
            status: "finished", homeFt: 2, awayFt: 1,
            homeEt: nil, awayEt: nil, homePens: nil, awayPens: nil
        ) == "ft")
        // Liga-döntetlen hosszabbítás nélkül: a döntés a rendes játékidőé.
        #expect(ManualEditRules.decidedBy(
            status: "finished", homeFt: 1, awayFt: 1,
            homeEt: nil, awayEt: nil, homePens: nil, awayPens: nil
        ) == "ft")
        #expect(ManualEditRules.decidedBy(
            status: "finished", homeFt: 1, awayFt: 1,
            homeEt: 2, awayEt: 1, homePens: nil, awayPens: nil
        ) == "et")
        #expect(ManualEditRules.decidedBy(
            status: "finished", homeFt: 1, awayFt: 1,
            homeEt: 2, awayEt: 2, homePens: 4, awayPens: 3
        ) == "pens")
        #expect(ManualEditRules.decidedBy(
            status: "awarded", homeFt: 3, awayFt: 0,
            homeEt: nil, awayEt: nil, homePens: nil, awayPens: nil
        ) == "ft")
        #expect(ManualEditRules.decidedBy(
            status: "scheduled", homeFt: nil, awayFt: nil,
            homeEt: nil, awayEt: nil, homePens: nil, awayPens: nil
        ) == nil)
        #expect(ManualEditRules.decidedByLabel("pens") == "Tizenegyesek")
    }

    // MARK: - Validáció

    private func validate(
        status: String,
        homeFt: String = "2",
        awayFt: String = "1",
        homeHt: String = "1",
        awayHt: String = "0",
        homeEt: String = "",
        awayEt: String = "",
        homePens: String = "",
        awayPens: String = "",
        note: String = "javítás"
    ) -> ManualEditRules.ManualEditValidation {
        ManualEditRules.validate(
            status: status,
            homeFt: homeFt,
            awayFt: awayFt,
            homeHt: homeHt,
            awayHt: awayHt,
            homeEt: homeEt,
            awayEt: awayEt,
            homePens: homePens,
            awayPens: awayPens,
            note: note
        )
    }

    @Test func validationRequiresNoteAndFullTime() {
        let empty = validate(status: "finished", homeFt: "", awayFt: "", homeHt: "", awayHt: "", note: "")
        #expect(!empty.isValid)
        #expect(empty.errors.contains("Az indoklás kötelező."))
        #expect(empty.errors.contains("Hazai gólok (rendes játékidő): kötelező."))
        #expect(empty.errors.contains("Vendég gólok (rendes játékidő): kötelező."))
    }

    @Test func validationRejectsNegativeAndPartialPairs() {
        let negative = validate(status: "finished", homeFt: "-1")
        #expect(!negative.isValid)

        let partialHalftime = validate(status: "finished", homeHt: "1", awayHt: "")
        #expect(partialHalftime.errors.contains(
            "A félidő eredményét mindkét csapatnál add meg, vagy egyiket sem."
        ))

        let drawn = validate(
            status: "finished",
            homeFt: "1", awayFt: "1",
            homeHt: "0", awayHt: "0",
            homeEt: "2", awayEt: "2",
            homePens: "4", awayPens: "4"
        )
        #expect(drawn.errors.contains("A tizenegyes-párbaj eredménye nem lehet döntetlen."))

        let missingPens = validate(
            status: "finished",
            homeFt: "1", awayFt: "1",
            homeHt: "0", awayHt: "0",
            homeEt: "2", awayEt: "2"
        )
        #expect(missingPens.errors.contains(
            "Hosszabbítás utáni döntetlennél a tizenegyesek eredménye kötelező."
        ))

        let validPens = validate(
            status: "finished",
            homeFt: "1", awayFt: "1",
            homeHt: "0", awayHt: "0",
            homeEt: "2", awayEt: "2",
            homePens: "4", awayPens: "3"
        )
        #expect(validPens.isValid)
        #expect(validPens.decidedBy == "pens")
    }

    @Test func hiddenSectionsAreIgnored() {
        // Nem döntetlen FT: a hosszabbítás/tizenegyes szekció rejtve van, a
        // benne maradt értékek nem hibáztathatnak és nem is küldhetők el.
        let validation = validate(
            status: "finished",
            homeFt: "2", awayFt: "1",
            homeEt: "2", awayEt: "2",
            homePens: "4", awayPens: "4"
        )
        #expect(validation.isValid)
    }

    @Test func stoppedStatusesSaveWithoutResultBlock() throws {
        let kickoff = try makeDate(2026, 11, 21, 18, 0, timeZone: "Europe/Budapest")
        for status in ["scheduled", "confirmed", "live", "halftime", "postponed", "canceled", "abandoned"] {
            let validation = validate(
                status: status,
                homeFt: "", awayFt: "", homeHt: "", awayHt: "",
                note: "halasztás"
            )
            #expect(validation.isValid, "\(status) eredmény nélkül menthető")
            #expect(validation.decidedBy == nil)

            let request = ManualEditRules.makeRequest(
                status: status,
                kickoffAt: kickoff,
                kickoffPrecision: "exact",
                homeFt: "2",
                awayFt: "1",
                homeHt: "1",
                awayHt: "0",
                homeEt: "",
                awayEt: "",
                homePens: "",
                awayPens: "",
                note: "halasztás"
            )
            #expect(!request.includesResult, "\(status) nem küldhet eredmény-blokkot")
        }
    }

    // MARK: - Payload-építés

    @Test func makeRequestSendsAwardedFullTimeOnly() {
        let kickoff = Date(timeIntervalSince1970: 1_800_000_000)
        let request = ManualEditRules.makeRequest(
            status: "awarded",
            kickoffAt: kickoff,
            kickoffPrecision: "exact",
            homeFt: "3",
            awayFt: "0",
            homeHt: "2",
            awayHt: "0",
            homeEt: "4",
            awayEt: "2",
            homePens: "5",
            awayPens: "4",
            note: "megítélt"
        )
        #expect(request.includesResult)
        #expect(request.homeFt == 3)
        #expect(request.awayFt == 0)
        #expect(request.homeHt == nil)
        #expect(request.homeEt == nil)
        #expect(request.homePens == nil)
        #expect(request.decidedBy == "ft")
    }

    @Test func makeRequestKeepsPenaltiesOnlyWhenExtraTimeDrawn() {
        let kickoff = Date(timeIntervalSince1970: 1_800_000_000)
        let request = ManualEditRules.makeRequest(
            status: "finished",
            kickoffAt: kickoff,
            kickoffPrecision: "exact",
            homeFt: "1",
            awayFt: "1",
            homeHt: "0",
            awayHt: "0",
            homeEt: "2",
            awayEt: "2",
            homePens: "4",
            awayPens: "3",
            note: "tizenegyesek"
        )
        #expect(request.homePens == 4)
        #expect(request.awayPens == 3)
        #expect(request.decidedBy == "pens")
        // A teljes blokk a JSON-ban is ott van.
        let payload = try? json(request)
        for key in Self.resultKeys {
            #expect(payload?[key] != nil, "\(key) hiányzik a payloadból")
        }
    }
}