import Foundation
import Testing
@testable import FootballAppCore

@Suite struct OpsSummaryModelsTests {
    @Test func summaryDecodesHungarianHeadlineAndActions() throws {
        let json = """
        {"date":"2026-11-25","open_total":2,"previous_day_total":3,
         "by_code":[{"code":"missing_odds","label_hu":"Hiányzó odds","count":2,"hard":1}],
         "top_actions":[{"kind":"open_issue","label_hu":"Jel megtekintése: Hiányzó odds",
           "issue_id":"00000000-0000-0000-0000-000000000001","subject_kind":"match",
           "subject_id":"00000000-0000-0000-0000-000000000002"}],
         "headline_hu":"2026. 11. 25.: 2 nyitott hiányjel."}
        """
        let dto = try APIDecoding.makeDecoder().decode(OpsSummaryDTO.self, from: Data(json.utf8))
        #expect(dto.openTotal == 2)
        #expect(dto.previousDayTotal == 3)
        #expect(dto.byCode[0].labelHu == "Hiányzó odds")
        #expect(dto.topActions[0].subjectId.uuidString == "00000000-0000-0000-0000-000000000002")
        #expect(dto.headlineHu.contains("nyitott hiányjel"))
    }

    @Test func auditFeedDecodesBothOrigins() throws {
        let json = """
        {"entries":[
          {"id":"00000000-0000-0000-0000-000000000001","actor":"ui",
           "action":"issues_detected","subject_kind":"day","subject_id":null,
           "created_at":"2026-11-26T12:00:00Z"},
          {"id":"00000000-0000-0000-0000-000000000002","actor":"detect",
           "action":"issue_resolved","subject_kind":"match",
           "subject_id":"00000000-0000-0000-0000-000000000003",
           "created_at":"2026-11-26T12:05:00Z"}],"count":2}
        """
        let dto = try APIDecoding.makeDecoder().decode(AuditResponseDTO.self, from: Data(json.utf8))
        #expect(dto.count == 2)
        #expect(dto.entries[0].subjectId == nil)
        #expect(dto.entries[1].subjectId?.uuidString == "00000000-0000-0000-0000-000000000003")
        #expect(dto.entries[1].createdAt > dto.entries[0].createdAt)
    }

    @Test func accessibilityLabelsCoverSummaryAuditAndResolve() {
        let label = AccessibilityLabels.auditEntryLabel(
            time: "2026. nov. 26. 12:00", actor: "ui", action: "issues_detected", subject: "nap"
        )
        #expect(label.contains("ui"))
        #expect(label.contains("issues_detected"))
        #expect(AccessibilityLabels.opsSummaryIdentifier == "ops.summary")
        #expect(AccessibilityLabels.opsAuditIdentifier == "ops.audit")
        let id = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
        #expect(AccessibilityLabels.auditEntryIdentifier(for: id).hasPrefix("ops.audit."))
        #expect(AccessibilityLabels.issueResolveIdentifier(for: id).hasPrefix("ops.issue.resolve."))
        #expect(AccessibilityLabels.issueResolveLabel == "Jel megoldottnak jelölése")
    }
}
