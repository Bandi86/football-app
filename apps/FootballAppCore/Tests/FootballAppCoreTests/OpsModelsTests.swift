import Foundation
import Testing
@testable import FootballAppCore

struct OpsModelsTests {
    @Test func decodesOpsStatus() throws {
        let json = """
        {"pending_jobs":2,"leased_jobs":0,"quarantined":1,"pending_reviews":3,
         "sources":[
           {"source_slug":"footballdata","mode":"live","enabled":true,
            "attempts_today":10,"daily_budget":25,"breaker_state":"closed"},
           {"source_slug":"soccerway","mode":"canary","enabled":true,
            "attempts_today":14,"daily_budget":14,"breaker_state":"open"}
         ]}
        """.data(using: .utf8)!
        let status = try APIDecoding.makeDecoder().decode(OpsStatus.self, from: json)
        #expect(status.pendingJobs == 2)
        #expect(status.pendingReviews == 3)
        #expect(status.sources.count == 2)
        #expect(status.sources[1].sourceSlug == "soccerway")
        #expect(status.sources[1].breakerState == "open")
        #expect(status.sources[1].attemptsToday == 14)
    }

    @Test func decodesIntegritySnapshot() throws {
        let json = """
        {"pending_jobs":0,"leased_jobs":0,"quarantined":0,"pending_reviews":0,
         "sources":[],
         "integrity":{"verdict":"OK","duplicate_kickoff_groups":0,
          "duplicate_fixture_groups":0,"orphan_provenance":0,"active_refs_to_merged":0,
          "pending_reviews":0,"withheld_gates":0,"failed_jobs":2,"open_breakers":0}}
        """.data(using: .utf8)!
        let status = try APIDecoding.makeDecoder().decode(OpsStatus.self, from: json)
        #expect(status.integrity?.verdict == "OK")
        #expect(status.integrity?.failedJobs == 2)
        #expect(status.integrity?.duplicateFixtureGroups == 0)
    }
}
