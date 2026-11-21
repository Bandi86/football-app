import SwiftUI
import FootballAppCore

/// Meccs-részletező panel: eredménytábla-fejléc, meta-rács, odds-mátrix,
/// statisztika-párbaj és ellenőrzési sáv. Minden szakasz adatvezérelt:
/// ami hiányzik, az nem jelenik meg, hamis értéket nem rajzolunk.
struct MatchDetailSheet: View {
    let matchId: UUID
    @Environment(\.dismiss) private var dismiss
    @State private var model = MatchDetailModel()

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Meccs")
                    .font(.appLabel(11))
                    .tracking(2.4)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Button("Bezárás") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                    .buttonStyle(.plain)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, AppTheme.Spacing.xl)
            .padding(.vertical, AppTheme.Spacing.m)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            content
        }
        .frame(minWidth: 720, minHeight: 620)
        .background(AppTheme.background)
        .accessibilityIdentifier("match.detail")
        .task(id: matchId) { await model.load(matchId: matchId) }
    }

    @ViewBuilder
    private var content: some View {
        switch model.state {
        case .idle, .loading:
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: {
                Task { await model.load(matchId: matchId) }
            })
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let detail):
            MatchDetailContent(detail: detail, logos: model.logos)
        }
    }
}

struct MatchDetailContent: View {
    let detail: MatchDetail
    let logos: LogoStore

    private static let kickoff: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy. MMMM d., EEEE HH:mm"
        return formatter
    }()

    private var isLive: Bool {
        ["live", "halftime"].contains(detail.status)
    }

    private var scoreText: String {
        guard let home = detail.homeFt ?? detail.homeEt, let away = detail.awayFt ?? detail.awayEt else {
            return "–"
        }
        return "\(home) – \(away)"
    }

    private var scoreNote: String? {
        switch detail.decidedBy {
        case "et": return "hosszabbítás után"
        case "pens": return "büntetők után"
        default: return nil
        }
    }

    private var hasStats: Bool {
        let counters: [Int?] = [
            detail.homeShots, detail.awayShots, detail.homeShotsOnTarget,
            detail.awayShotsOnTarget, detail.homeCorners, detail.awayCorners,
            detail.homeYellow, detail.awayYellow, detail.homeRed, detail.awayRed,
            detail.homeFouls, detail.awayFouls,
        ]
        return counters.contains { $0 != nil } || detail.homeXg != nil || detail.awayXg != nil
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: AppTheme.Spacing.l) {
                ScoreboardHero(
                    home: detail.homeName,
                    away: detail.awayName,
                    homeLogo: detail.homeLogo,
                    awayLogo: detail.awayLogo,
                    homeElo: detail.homeElo,
                    awayElo: detail.awayElo,
                    score: scoreText,
                    note: scoreNote,
                    statusLabel: MatchRowView.statusLabel(for: detail.status),
                    isLive: isLive,
                    gateState: detail.gateState,
                    logos: logos
                )

                metaGrid

                if detail.homeHt != nil || detail.homeEt != nil || detail.homePens != nil {
                    periodChips
                }

                if !(detail.homeForm ?? []).isEmpty || !(detail.awayForm ?? []).isEmpty {
                    FormStrip(
                        homeName: detail.homeName,
                        awayName: detail.awayName,
                        homeForm: detail.homeForm ?? [],
                        awayForm: detail.awayForm ?? []
                    )
                    .accessibilityIdentifier("match.detail.form")
                }

                if let headToHead = detail.headToHead, !headToHead.isEmpty {
                    HeadToHeadList(entries: headToHead)
                        .accessibilityIdentifier("match.detail.h2h")
                }

                if hasStats {
                    StatsDuel(
                        homeName: detail.homeName,
                        awayName: detail.awayName,
                        rows: statRows
                    )
                    .accessibilityIdentifier("match.detail.stats")
                }

                if let odds = detail.odds {
                    OddsMatrix(
                        home: odds.home,
                        draw: odds.draw,
                        away: odds.away,
                        bttsYes: odds.bttsYes,
                        bttsNo: odds.bttsNo,
                        over25: odds.over25,
                        under25: odds.under25,
                        source: detail.oddsSource,
                        capturedAt: detail.oddsCapturedAt
                    )
                    .accessibilityIdentifier("match.detail.odds")
                }

                VerificationStrip(
                    gateState: detail.gateState,
                    ruleSetVersion: detail.gateRuleSetVersion,
                    evaluatedAt: detail.gateEvaluatedAt,
                    blockers: detail.gateBlockers
                )
                .accessibilityIdentifier("match.detail.verification")
            }
            .padding(AppTheme.Spacing.xl)
            .frame(maxWidth: 860, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
    }

    private var metaGrid: some View {
        VStack(spacing: 0) {
            gridRow("Verseny", "\(detail.competitionName) · \(detail.seasonLabel)")
            gridRow("Időpont", Self.kickoff.string(from: detail.kickoffAt) +
                    (detail.kickoffPrecision == "date_only" ? " (csak dátum)" : ""))
            if let round = detail.roundLabel {
                gridRow("Forduló", round)
            }
            gridRow("Játékvezető", detail.referee ?? "–")
            if let source = detail.statsSource {
                gridRow("Statisztika forrása", source)
            }
        }
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func gridRow(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: AppTheme.Spacing.m) {
            Text(label)
                .font(.appLabel(9))
                .tracking(1.1)
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 150, alignment: .leading)
            Text(value)
                .font(.appText(12.5))
                .foregroundStyle(AppTheme.primaryText)
                .frame(maxWidth: .infinity, alignment: .leading)
            Divider().overlay(AppTheme.hairline)
        }
        .padding(.horizontal, AppTheme.Spacing.m)
        .padding(.vertical, 8)
    }

    private var periodChips: some View {
        HStack(spacing: AppTheme.Spacing.s) {
            if let hh = detail.homeHt, let ah = detail.awayHt {
                chip("Félidő", "\(hh)–\(ah)")
            }
            if let he = detail.homeEt, let ae = detail.awayEt {
                chip("120'", "\(he)–\(ae)")
            }
            if let hp = detail.homePens, let ap = detail.awayPens {
                chip("Büntetők", "\(hp)–\(ap)")
            }
            if detail.homeFt != nil {
                chip("90'", scoreText)
            }
            Spacer()
        }
    }

    private func chip(_ label: String, _ value: String) -> some View {
        VStack(spacing: 0) {
            Text(value).font(.scoreboard(16, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
            Text(label).sectionLabel()
        }
        .padding(.horizontal, AppTheme.Spacing.m)
        .padding(.vertical, 7)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.chip, style: .continuous))
    }

    private var statRows: [(label: String, home: Double?, away: Double?, format: String)] {
        var rows: [(String, Double?, Double?, String)] = [
            ("Kapura lövés", detail.homeShotsOnTarget.map(Double.init), detail.awayShotsOnTarget.map(Double.init), "%d"),
            ("Lövés", detail.homeShots.map(Double.init), detail.awayShots.map(Double.init), "%d"),
            ("Szöglet", detail.homeCorners.map(Double.init), detail.awayCorners.map(Double.init), "%d"),
            ("Szabálytalanság", detail.homeFouls.map(Double.init), detail.awayFouls.map(Double.init), "%d"),
            ("Sárga", detail.homeYellow.map(Double.init), detail.awayYellow.map(Double.init), "%d"),
            ("Piros", detail.homeRed.map(Double.init), detail.awayRed.map(Double.init), "%d"),
        ]
        if detail.homeXg != nil || detail.awayXg != nil {
            rows.append(("xG", detail.homeXg, detail.awayXg, "%.2f"))
        }
        return rows.filter { $0.1 != nil || $0.2 != nil }
    }
}


/// Utolsó öt meccs W/D/L chipjei csapatonként; minden chip az ellenfelet és
/// az eredményt is kimondja az akadálymentes felolvasásban.
struct FormStrip: View {
    let homeName: String
    let awayName: String
    let homeForm: [FormEntry]
    let awayForm: [FormEntry]

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("FORMA · UTOLSÓ 5 MECCS")
                .font(.appLabel(9))
                .tracking(1.4)
                .foregroundStyle(AppTheme.secondaryText)
            row(name: homeName, entries: homeForm)
            row(name: awayName, entries: awayForm)
        }
        .padding(AppTheme.Spacing.m)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func row(name: String, entries: [FormEntry]) -> some View {
        HStack(spacing: AppTheme.Spacing.s) {
            Text(name)
                .font(.appText(12, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .lineLimit(1)
                .frame(width: 190, alignment: .leading)
            if entries.isEmpty {
                Text("nincs korábbi lejátszott meccs")
                    .font(.appText(11))
                    .foregroundStyle(AppTheme.tertiaryText)
            } else {
                ForEach(entries) { entry in
                    Text(entry.outcome)
                        .font(.appText(11, weight: .bold))
                        .foregroundStyle(.black)
                        .frame(width: 20, height: 20)
                        .background(chipColor(entry.outcome), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                        .help("\(entry.isHome ? "hazai" : "vendég") \(entry.opponentName) ellen \(entry.teamGoals)–\(entry.opponentGoals)")
                        .accessibilityLabel(outcomeLabel(entry))
                }
            }
            Spacer()
        }
    }

    private func chipColor(_ outcome: String) -> Color {
        switch outcome {
        case "W": return AppTheme.predictionHome
        case "L": return AppTheme.warning
        default: return AppTheme.predictionDraw
        }
    }

    private func outcomeLabel(_ entry: FormEntry) -> String {
        let result = entry.outcome == "W" ? "győzelem" : (entry.outcome == "L" ? "vereség" : "döntetlen")
        return "\(entry.isHome ? "Hazai" : "Vendég") \(entry.opponentName) ellen \(entry.teamGoals)–\(entry.opponentGoals), \(result)"
    }
}

/// Korábbi egymás elleni meccsek tömör listája.
struct HeadToHeadList: View {
    let entries: [HeadToHeadEntry]

    private static let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy. MM. dd."
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("EGYMÁS ELLEN")
                .font(.appLabel(9))
                .tracking(1.4)
                .foregroundStyle(AppTheme.secondaryText)
            ForEach(entries) { entry in
                HStack(spacing: AppTheme.Spacing.m) {
                    Text(Self.dayFormatter.string(from: entry.kickoffAt))
                        .font(.appText(11))
                        .foregroundStyle(AppTheme.tertiaryText)
                        .frame(width: 96, alignment: .leading)
                    Text("\(entry.homeName) – \(entry.awayName)")
                        .font(.appText(12))
                        .foregroundStyle(AppTheme.primaryText)
                        .lineLimit(1)
                    Spacer()
                    Text(scoreText(entry))
                        .font(.scoreboard(13, weight: .semibold))
                        .foregroundStyle(AppTheme.primaryText)
                    Text(entry.competitionName)
                        .font(.appLabel(9))
                        .foregroundStyle(AppTheme.tertiaryText)
                        .lineLimit(1)
                        .frame(width: 150, alignment: .trailing)
                }
                .padding(.horizontal, AppTheme.Spacing.m)
                .padding(.vertical, 6)
                .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: AppTheme.Radius.chip, style: .continuous))
            }
        }
        .padding(AppTheme.Spacing.m)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func scoreText(_ entry: HeadToHeadEntry) -> String {
        guard let home = entry.homeGoals, let away = entry.awayGoals else { return "–" }
        return "\(home)–\(away)"
    }
}
