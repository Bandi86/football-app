import Foundation
import Testing
@testable import FootballAppCore

@Suite struct DayModelsTests {
    @Test func decodesDayResponseFixture() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let response = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json)
        #expect(response.date == "2026-11-09")
        #expect(response.timezone == "Europe/Budapest")
        #expect(response.matches.count == 1)
        let match = response.matches[0]
        #expect(match.home == "Újpest")
        #expect(match.away == "Győr")
        #expect(match.homeScore == nil)
        #expect(match.awayScore == nil)
        #expect(match.publishState == .verifiedPublish)
        #expect(match.blockers.isEmpty)
    }

    @Test func unknownPublishStateMapsToUnknown() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000102","kickoff_at":"2026-11-09T19:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"valami_uj","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let response = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json)
        #expect(response.matches[0].publishState == .unknown)
    }

    @Test func decodesKickoffWithFractionalSeconds() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000104","kickoff_at":"2026-11-09T18:00:00.500Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]

        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try #require(TimeZone(identifier: "UTC"))
        let components = calendar.dateComponents(
            [.year, .month, .day, .hour, .minute, .second, .nanosecond],
            from: match.kickoffAt
        )
        #expect(components.year == 2026)
        #expect(components.month == 11)
        #expect(components.day == 9)
        #expect(components.hour == 18)
        #expect(components.minute == 0)
        #expect(components.second == 0)
        #expect(match.kickoffAt.timeIntervalSince1970 > 0)
        // A törtrész pontosabban kell dekódolódjon, mint egész másodpercre kerekítve.
        #expect(components.nanosecond == 500_000_000)
    }

    @Test func decodesResultAndBlocker() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000103","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"finished","home":"A","away":"B","home_score":2,"away_score":1,
           "publish_state":"withheld","blockers":[
             {"rule_key":"MATERIAL_SOURCE_CONFLICT","severity":"blocker","field":"kickoff_at","message":"Források eltérnek."}
           ]}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.homeScore == 2)
        #expect(match.awayScore == 1)
        #expect(match.publishState == .withheld)
        #expect(match.blockers.first?.ruleKey == "MATERIAL_SOURCE_CONFLICT")
        #expect(match.blockers.first?.severity == "blocker")
    }

    @Test func decodesDisplayFields() throws {
        let json = """
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000201","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"finished","home":"Újpest","away":"Győr","home_score":2,"away_score":1,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-000000000001",
           "competition_name":"NB I","country_code":"HUN","country_name":"Hungary",
           "tier":1,"priority":6,
           "home_ht":1,"away_ht":0,"home_et":null,"away_et":null,"home_pens":null,"away_pens":null,
           "home_position":8,"away_position":6,
           "odds":{"home":2.2,"draw":3.3,"away":3.2,"btts_yes":1.85,"btts_no":1.95,"over25":2.15,"under25":1.7},
           "market_count":18,"odds_source":"Demo odds feed","odds_captured_at":"2026-11-11T09:00:00Z"}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.competitionName == "NB I")
        #expect(match.countryCode == "HUN")
        #expect(match.tier == 1)
        #expect(match.priority == 6)
        #expect(match.homeHt == 1)
        #expect(match.awayHt == 0)
        #expect(match.homeEt == nil)
        #expect(match.homePosition == 8)
        #expect(match.awayPosition == 6)
        #expect(match.odds?.home == 2.2)
        #expect(match.odds?.bttsYes == 1.85)
        #expect(match.odds?.under25 == 1.7)
        #expect(match.marketCount == 18)
        #expect(match.oddsSource == "Demo odds feed")
        #expect(match.oddsCapturedAt != nil)
    }

    @Test func missingDisplayFieldsDecodeAsNil() throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.competitionId == nil)
        #expect(match.odds == nil)
        #expect(match.homePosition == nil)
        #expect(match.marketCount == nil)
    }

    @Test func decodesOddsTrustState() throws {
        let json = """
        {"date":"2026-11-13","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000301","kickoff_at":"2026-11-13T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "odds":{"home":2.2,"draw":3.3,"away":3.2,"btts_yes":1.85,"btts_no":1.95,"over25":2.15,"under25":1.7},
           "market_count":18,"odds_source":"Demó","odds_captured_at":"2026-11-13T09:00:00Z",
           "odds_trust_state":"verified"}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.oddsTrustState == "verified")
        #expect(match.odds?.home == 2.2)
    }

    @Test func missingOddsTrustStateIsNil() throws {
        // Régi API válasz odds_trust_state nélkül: a dekódolás nem bukhat, a mező nil.
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000302","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.oddsTrustState == nil)
    }

    @Test func nullOddsTrustStateIsNil() throws {
        // Odds nélküli meccsnél az API explicit null-t küld.
        let json = """
        {"date":"2026-11-13","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000303","kickoff_at":"2026-11-13T19:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],"odds_trust_state":null}
        ]}
        """.data(using: .utf8)!
        let match = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches[0]
        #expect(match.oddsTrustState == nil)
    }
}
