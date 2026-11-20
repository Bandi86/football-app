import Foundation
import Testing
@testable import FootballAppCore

@Suite struct AccessibilityLabelsTests {
    private var kickoff: Date {
        Date(timeIntervalSince1970: 1_793_851_200) // 2026-11-09 18:00 UTC
    }

    @Test func knownStatusHasHungarianVoiceOverLabel() {
        #expect(AccessibilityLabels.statusLabel(for: "finished") == "Lezárt")
        #expect(AccessibilityLabels.statusLabel(for: "postponed") == "Elhalasztott")
        #expect(AccessibilityLabels.statusLabel(for: "valami_uj") == "valami_uj")
    }

    @Test func publishStateHasSpokenLabel() {
        #expect(AccessibilityLabels.publishLabel(.verifiedPublish) == "Ellenőrzött publikáció")
        #expect(AccessibilityLabels.publishLabel(.withheld) == "Visszatartott publikáció")
        #expect(AccessibilityLabels.publishLabel(.unknown) == "Ismeretlen publikáció")
    }

    @Test func matchRowLabelIncludesTeamsStatusScoreAndQuality() {
        let label = AccessibilityLabels.matchRowLabel(
            home: "Ferencváros",
            away: "Győr",
            kickoffAt: kickoff,
            status: "finished",
            homeScore: 2,
            awayScore: 1,
            publishState: .verifiedPublish
        )
        #expect(label.contains("Ferencváros"))
        #expect(label.contains("Győr"))
        #expect(label.contains("Lezárt"))
        #expect(label.contains("2 – 1"))
        #expect(label.contains("Ellenőrzött publikáció"))
    }

    @Test func matchRowWithoutResultNeverReadsAsZeroZero() {
        let label = AccessibilityLabels.matchRowLabel(
            home: "Újpest",
            away: "Győr",
            kickoffAt: kickoff,
            status: "scheduled",
            homeScore: nil,
            awayScore: nil,
            publishState: .provisionalPublish
        )
        #expect(label.contains("eredmény még nincs"))
        #expect(!label.contains("0 – 0"))
    }

    @Test func matchRowIdentifierIsStableAndUnique() {
        let id = UUID(uuidString: "00000000-0000-0000-0000-000000000101")!
        #expect(
            AccessibilityLabels.matchIdentifier(for: id)
                == "match.row.00000000-0000-0000-0000-000000000101"
        )
    }

    @Test func matchCardLabelUsesDisplayTimeAndNeverReadsMissingResultAsZero() {
        let withScore = AccessibilityLabels.matchCardLabel(
            home: "Ferencváros", away: "Győr", time: "18:00", status: "finished",
            homeScore: 2, awayScore: 1
        )
        #expect(withScore.contains("Ferencváros – Győr, 18:00"))
        #expect(withScore.contains("2 – 1"))
        #expect(withScore.contains("Lezárt"))

        let withoutScore = AccessibilityLabels.matchCardLabel(
            home: "Újpest", away: "Győr", time: "20:45", status: "scheduled",
            homeScore: nil, awayScore: nil
        )
        #expect(withoutScore.contains("eredmény még nincs"))
        #expect(!withoutScore.contains("0 – 0"))
    }
}
