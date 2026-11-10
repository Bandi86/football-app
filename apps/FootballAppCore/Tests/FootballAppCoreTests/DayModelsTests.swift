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
}
