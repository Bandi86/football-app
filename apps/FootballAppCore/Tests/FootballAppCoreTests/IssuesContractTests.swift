import Foundation
import Testing
@testable import FootballAppCore

@Suite struct IssuesContractTests {
    private static var fixtureURL: URL {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = directory.appendingPathComponent(
                "tests/fixtures/contract/issues-response.json"
            )
            if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
            directory.deleteLastPathComponent()
        }
        fatalError("a kontrakt-fixture nem található a #filePath-ból indulva")
    }

    @Test func sharedIssuesFixtureDecodesIntoDTO() throws {
        let data = try Data(contentsOf: Self.fixtureURL)
        let response = try APIDecoding.makeDecoder().decode(IssuesResponseDTO.self, from: data)
        #expect(response.count == 1)
        let issue = try #require(response.issues.first)
        #expect(issue.code == .missingOdds)
        #expect(issue.severity == .warning)
        #expect(issue.subjectKind == .match)
        #expect(issue.source == "daily-check")
        #expect(issue.resolvedAt == nil)
        #expect(issue.firstSeenAt < issue.lastSeenAt)
    }

    @Test func historyPayloadDecodesBothOrigins() throws {
        let json = """
        {"entries":[
          {"at":"2026-11-26T10:00:00Z","origin":"audit","actor":"ui",
           "action":"manual_match_edit","field_key":null,
           "before":{"status":"scheduled"},"after":{"status":"finished"},"source":null},
          {"at":"2026-11-26T11:00:00Z","origin":"provenance","actor":"forrás",
           "action":"mező kiválasztás","field_key":"kickoff_at",
           "before":null,"after":null,"source":"research"}
        ]}
        """
        let response = try APIDecoding.makeDecoder().decode(
            HistoryResponseDTO.self, from: Data(json.utf8)
        )
        #expect(response.entries.count == 2)
        #expect(response.entries[0].origin == .audit)
        #expect(response.entries[0].before?.displayText.contains("scheduled") == true)
        #expect(response.entries[0].after?.displayText.contains("finished") == true)
        #expect(response.entries[1].origin == .provenance)
        #expect(response.entries[1].fieldKey == "kickoff_at")
        #expect(response.entries[1].source == "research")
    }
}
