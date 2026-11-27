import Foundation
import Testing
@testable import FootballAppCore

@Suite struct OpsIssueModelsTests {
    @Test func unknownEnumsDoNotThrow() throws {
        let json = """
        {"issues":[{"id":"00000000-0000-0000-0000-000000000001",
          "subject_kind":"uj_targy","subject_id":"00000000-0000-0000-0000-000000000002",
          "code":"uj_hiany","severity":"uj_suly",
          "first_seen_at":"2026-11-26T08:00:00Z","last_seen_at":"2026-11-26T09:00:00Z",
          "resolved_at":null,"source":"uj_forras"}],"count":1}
        """
        let response = try APIDecoding.makeDecoder().decode(
            IssuesResponseDTO.self, from: Data(json.utf8)
        )
        let issue = try #require(response.issues.first)
        #expect(issue.subjectKind == .unknown)
        #expect(issue.severity == .unknown)
        #expect(issue.code == .other("uj_hiany"))
        #expect(issue.code.key == "uj_hiany")
    }

    @Test func missingResultCodeDecodesAndHasAHungarianLabel() throws {
        let json = """
        {"issues":[{"id":"00000000-0000-0000-0000-000000000003",
          "subject_kind":"match","subject_id":"00000000-0000-0000-0000-000000000004",
          "code":"missing_result","severity":"hard",
          "first_seen_at":"2026-11-26T08:00:00Z","last_seen_at":"2026-11-26T09:00:00Z",
          "resolved_at":null,"source":"daily-check"}],"count":1}
        """
        let response = try APIDecoding.makeDecoder().decode(
            IssuesResponseDTO.self, from: Data(json.utf8)
        )
        let issue = try #require(response.issues.first)
        #expect(issue.code == .missingResult)
        #expect(issue.code.key == "missing_result")
        #expect(OpsFormatting.issueCodeLabel(issue.code) == "Eredmény nélkül")
    }

    @Test func hungarianLabelsCoverAllKnownValues() {
        #expect(OpsFormatting.issueCodeLabel(.missingOdds) == "Hiányzó odds")
        #expect(OpsFormatting.issueCodeLabel(.missingResult) == "Eredmény nélkül")
        #expect(OpsFormatting.issueCodeLabel(.identityReview) == "Identitás-review")
        #expect(OpsFormatting.severityLabel(.hard) == "Súlyos")
        #expect(OpsFormatting.severityLabel(.unknown) == "Ismeretlen súlyosság")
        #expect(OpsFormatting.sourceLabel("daily-check") == "Napi ellenőrzés")
        #expect(OpsFormatting.subjectLabel(.team) == "Csapat")
        #expect(OpsFormatting.noteKindLabel("fix") == "Javítás")
    }

    @Test func jsonValueReadsObjectsAndMissingData() throws {
        let json = """
        {"entries":[{"at":"2026-11-26T10:00:00Z","origin":"audit","actor":"ui","action":"edit",
         "field_key":"status","before":{"status":null,"golszam":2},"after":null,"source":null}]}
        """
        let entry = try APIDecoding.makeDecoder().decode(
            HistoryResponseDTO.self, from: Data(json.utf8)
        ).entries[0]
        #expect(entry.before?.displayText.contains("status: nincs adat") == true)
        #expect(entry.before?.displayText.contains("golszam: 2") == true)
        #expect(entry.after == nil, "a null JSON a DTO-ban nil, nem .null")
    }

    @Test func timestampUsesHungarianLocale() throws {
        let date = Date(timeIntervalSince1970: 1_793_851_200)
        let text = OpsFormatting.timestamp(date)
        #expect(text.contains("2026"))
        #expect(!text.contains("AM") && !text.contains("PM"))
    }
}
