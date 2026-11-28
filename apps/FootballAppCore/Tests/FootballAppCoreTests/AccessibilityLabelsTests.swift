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
            homeScore: 2, awayScore: 1, publishState: .verifiedPublish
        )
        #expect(withScore.contains("Ferencváros – Győr, 18:00"))
        #expect(withScore.contains("2 – 1"))
        #expect(withScore.contains("Lezárt"))

        let withoutScore = AccessibilityLabels.matchCardLabel(
            home: "Újpest", away: "Győr", time: "20:45", status: "scheduled",
            homeScore: nil, awayScore: nil, publishState: .provisionalPublish
        )
        #expect(withoutScore.contains("eredmény még nincs"))
        #expect(!withoutScore.contains("0 – 0"))
        #expect(withoutScore.contains("Ideiglenes publikáció"))
        #expect(withScore.contains("Ellenőrzött publikáció"))
    }

    @Test func matchCardLabelSpeaksDayAndOddsWhenTheCardShowsThem() {
        let label = AccessibilityLabels.matchCardLabel(
            home: "Ferencváros", away: "Újpest", time: "18:00", status: "finished",
            homeScore: 2, awayScore: 1, publishState: .verifiedPublish,
            dayLabel: "nov. 20.", odds: "1: 1,85, X: 3,40, 2: 4,20"
        )
        #expect(label.contains("Ferencváros – Újpest, nov. 20. 18:00"))
        #expect(label.contains("2 – 1"))
        #expect(label.contains("odds 1: 1,85, X: 3,40, 2: 4,20"))

        let withoutOdds = AccessibilityLabels.matchCardLabel(
            home: "Ferencváros", away: "Újpest", time: "18:00", status: "scheduled",
            homeScore: nil, awayScore: nil, publishState: .verifiedPublish
        )
        #expect(!withoutOdds.contains("odds"))
        #expect(withoutOdds.contains("Újpest, 18:00"))
    }

    @Test func formMatchLabelSpeaksActionTeamOpponentAndResult() {
        let id = UUID(uuidString: "10000000-0000-0000-0000-000000000001")!
        let entry = FormEntry(
            matchId: id,
            kickoffAt: kickoff,
            competitionName: "NB I",
            opponentName: "Ferencváros",
            isHome: true,
            teamGoals: 2,
            opponentGoals: 1,
            outcome: "W"
        )
        let label = AccessibilityLabels.formMatchLabel(teamName: "Debrecen", entry)
        #expect(label.contains("Meccs megnyitása"))
        #expect(label.contains("Debrecen"))
        #expect(label.contains("hazai Ferencváros ellen"))
        #expect(label.contains("2–1"))
        #expect(label.contains("győzelem"))
        #expect(AccessibilityLabels.formMatchIdentifier(for: id)
            == "match.detail.form.10000000-0000-0000-0000-000000000001")
    }

    @Test func headToHeadMatchLabelNeverInventsMissingScore() {
        let id = UUID(uuidString: "10000000-0000-0000-0000-000000000005")!
        let entry = HeadToHeadEntry(
            matchId: id,
            kickoffAt: kickoff,
            competitionName: "Magyar Kupa",
            homeName: "Paks",
            awayName: "Debrecen",
            homeGoals: nil,
            awayGoals: nil,
            homeCorners: nil, awayCorners: nil,
            homeYellow: nil, awayYellow: nil,
            homeRed: nil, awayRed: nil,
            homeXg: nil, awayXg: nil
        )
        let label = AccessibilityLabels.headToHeadMatchLabel(entry)
        #expect(label.contains("Meccs megnyitása"))
        #expect(label.contains("Paks – Debrecen"))
        #expect(label.contains("eredmény még nincs"))
        #expect(!label.contains("0 – 0"))
        #expect(!label.contains("0–0"))
        #expect(AccessibilityLabels.headToHeadMatchIdentifier(for: id)
            == "match.detail.h2h.10000000-0000-0000-0000-000000000005")
    }

    @Test func nextMatchLabelSpeaksTeamVenueOpponentAndCompetition() {
        let id = UUID(uuidString: "10000000-0000-0000-0000-000000000006")!
        let entry = NextMatchEntry(
            matchId: id,
            kickoffAt: kickoff,
            competitionName: "NB I",
            opponentName: "Győr",
            isHome: true,
            homeLogo: nil, awayLogo: nil
        )
        let label = AccessibilityLabels.nextMatchLabel(teamName: "Debrecen", entry)
        #expect(label.contains("Meccs megnyitása"))
        #expect(label.contains("Debrecen"))
        #expect(label.contains("hazai Győr ellen"))
        #expect(label.contains("NB I"))
        #expect(AccessibilityLabels.nextMatchIdentifier(for: id)
            == "match.detail.next.10000000-0000-0000-0000-000000000006")
    }

    @Test func matchCardLabelSpeaksEstimatedClockInsteadOfKickoffTime() {
        let label = AccessibilityLabels.matchCardLabel(
            home: "Ferencváros", away: "Újpest", time: "18:00", status: "live",
            homeScore: nil, awayScore: nil, publishState: .verifiedPublish,
            dayLabel: "nov. 20.", clockText: "10'", clockIsEstimated: true
        )
        #expect(label.contains("Ferencváros – Újpest, nov. 20., becsült 10. perc"))
        #expect(!label.contains("18:00"))
        #expect(label.contains("Élő"))
    }

    @Test func matchCardLabelSpeaksEstimatedHalftimeAndAddedTime() {
        let halftime = AccessibilityLabels.matchCardLabel(
            home: "Ferencváros", away: "Újpest", time: "18:00", status: "halftime",
            homeScore: nil, awayScore: nil, publishState: .verifiedPublish,
            dayLabel: "nov. 20.", clockText: "HT", clockIsEstimated: true
        )
        #expect(halftime.contains("nov. 20., becsült félidő"))
        #expect(halftime.contains("Félidő"))
        #expect(!halftime.contains("18:00"))

        let addedTime = AccessibilityLabels.matchCardLabel(
            home: "Ferencváros", away: "Újpest", time: "18:00", status: "live",
            homeScore: nil, awayScore: nil, publishState: .verifiedPublish,
            dayLabel: "nov. 20.", clockText: "90+", clockIsEstimated: true
        )
        #expect(addedTime.contains("becsült 90. perc vagy ráadás"))
        #expect(!addedTime.contains("18:00"))
    }

    @Test func matchCardLabelSpeaksReportedClockWithoutEstimatedQualifier() {
        let label = AccessibilityLabels.matchCardLabel(
            home: "Ferencváros", away: "Újpest", time: "18:00", status: "live",
            homeScore: nil, awayScore: nil, publishState: .verifiedPublish,
            clockText: "67'", clockIsEstimated: false
        )
        #expect(label.contains("67. perc"))
        #expect(!label.contains("becsült"))
        #expect(!label.contains("18:00"))
    }

    @Test func incidentTypeLabelsAreHungarian() {
        #expect(AccessibilityLabels.incidentTypeLabel(.goal) == "Gól")
        #expect(AccessibilityLabels.incidentTypeLabel(.card) == "Kártya")
        #expect(AccessibilityLabels.incidentTypeLabel(.substitution) == "Csere")
        #expect(AccessibilityLabels.incidentTypeLabel(.half) == "Félidő")
        #expect(AccessibilityLabels.incidentTypeLabel(.addedTime) == "Ráadásidő")
        #expect(AccessibilityLabels.incidentTypeLabel(.varReview) == "VAR-ellenőrzés")
        #expect(AccessibilityLabels.incidentTypeLabel(.missedPenalty) == "Kihagyott büntető")
        #expect(AccessibilityLabels.incidentTypeLabel(.penaltyShootout) == "Tizenegyespárbaj")
        #expect(
            AccessibilityLabels.incidentTypeLabel(.other("FanChant"))
                == "Egyéb esemény: FanChant"
        )
    }

    @Test func matchIncidentLabelSpeaksMinuteSidePlayerAndGoalScore() {
        let goal = AccessibilityLabels.matchIncidentLabel(
            minute: 34, kind: .goal, side: "home", playerName: "Varga Barnabás",
            relatedPlayerName: "Szoboszlai Dominik", detail: nil,
            scoreAfterHome: 1, scoreAfterAway: 0
        )
        #expect(
            goal == "34. perc, hazai, Gól, Varga Barnabás, másik játékos: Szoboszlai Dominik, 1 – 0"
        )

        let varReview = AccessibilityLabels.matchIncidentLabel(
            minute: nil, kind: .varReview, side: nil, playerName: nil,
            relatedPlayerName: nil, detail: "Goal check",
            scoreAfterHome: nil, scoreAfterAway: nil
        )
        #expect(varReview.contains("perc nélkül"))
        #expect(varReview.contains("VAR-ellenőrzés"))
        #expect(!varReview.contains("hazai"))
        #expect(!varReview.contains("vendég"))
    }

    @Test func timelineEmptyStateHasHungarianLabelAndStableIdentifier() {
        #expect(AccessibilityLabels.timelineEmptyIdentifier == "match.timeline.empty")
        #expect(AccessibilityLabels.timelineEmptyTitle == "Az események még nem érkeztek meg.")
        #expect(AccessibilityLabels.timelineEmptySubtitle == "A részletes adatok betöltése folyamatban lehet.")
        let label = AccessibilityLabels.timelineEmptyLabel()
        #expect(label.contains(AccessibilityLabels.timelineEmptyTitle))
        #expect(label.contains(AccessibilityLabels.timelineEmptySubtitle))
    }

    @Test func issueLabelSpeaksSeverityTimelineAndResolution() {
        let label = AccessibilityLabels.issueLabel(
            code: "Hiányzó odds", severity: "Figyelmeztetés", subject: "Meccs",
            firstSeen: "2026. nov. 26. 8:00", lastSeen: "2026. nov. 26. 9:30",
            source: "Napi ellenőrzés", resolved: false
        )
        #expect(label.contains("Hiányzó odds"))
        #expect(label.contains("Figyelmeztetés"))
        #expect(label.contains("először"))
        #expect(label.contains("utoljára"))
        #expect(label.contains("megoldatlan"))
        let resolved = AccessibilityLabels.issueLabel(
            code: "Hiányzó logó", severity: "Súlyos", subject: "Csapat",
            firstSeen: "tegnap", lastSeen: "ma", source: "Ügynök", resolved: true
        )
        #expect(resolved.contains("megoldva"))
    }

    @Test func issueIdentifiersAreStable() {
        let id = UUID(uuidString: "00000000-0000-0000-0000-000000000001")!
        #expect(AccessibilityLabels.issueIdentifier(for: id) == "ops.issue.00000000-0000-0000-0000-000000000001")
        #expect(AccessibilityLabels.historyEntryIdentifier(for: 3) == "ops.history.row.3")
        #expect(AccessibilityLabels.issuesEmptyIdentifier == "ops.issues.empty")
        #expect(AccessibilityLabels.issuesEmptyLabel().contains("Még nincs rögzített hiányjel."))
    }

    @Test func historyEntryLabelNamesActorActionFieldAndSource() {
        let label = AccessibilityLabels.historyEntryLabel(
            time: "2026. nov. 26. 9:30", actor: "ui", action: "kézi szerkesztés",
            field: "kickoff_at", source: "research"
        )
        #expect(label.contains("ui"))
        #expect(label.contains("kézi szerkesztés"))
        #expect(label.contains("mező: kickoff_at"))
        #expect(label.contains("forrás: research"))
    }
}
