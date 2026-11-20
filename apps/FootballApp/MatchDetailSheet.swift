import SwiftUI
import FootballAppCore

/// Meccs-részletező sheet: betöltés → részletes, ellenőrzött nézet.
struct MatchDetailSheet: View {
    let matchId: UUID
    @State private var model = MatchDetailModel()
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Meccs")
                    .font(.system(size: 12, weight: .semibold))
                    .tracking(2)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Button("Bezárás") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                    .buttonStyle(.plain)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 14)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            content
        }
        .frame(minWidth: 640, minHeight: 560)
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

private struct MatchDetailContent: View {
    let detail: MatchDetail
    let logos: LogoStore

    private static let kickoff: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .full
        formatter.timeStyle = .short
        return formatter
    }()
    private static let captured: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                header
                metaRow
                resultSection
                if hasStats { statsSection }
                oddsSection
                trustSection
            }
            .padding(24)
        }
    }

    private var header: some View {
        VStack(spacing: 14) {
            HStack(spacing: 10) {
                StatusBadge(state: detail.gateState ?? .unknown)
                Text(MatchRowView.statusLabel(for: detail.status).uppercased())
                    .font(.system(size: 9.5, weight: .bold))
                    .tracking(1.2)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                if let publishDate = detail.publishDate {
                    Text("publikálva: \(publishDate)")
                        .font(.system(size: 10).monospacedDigit())
                        .foregroundStyle(AppTheme.secondaryText)
                }
            }

            HStack(alignment: .top, spacing: 16) {
                teamColumn(name: detail.homeName, logo: detail.homeLogo, elo: detail.homeElo, alignTrailing: false)
                scoreBlock
                teamColumn(name: detail.awayName, logo: detail.awayLogo, elo: detail.awayElo, alignTrailing: true)
            }
        }
        .padding(18)
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
    }

    private func teamColumn(name: String, logo: String?, elo: Double?, alignTrailing: Bool) -> some View {
        HStack(spacing: 10) {
            if alignTrailing { Spacer(minLength: 0) }
            if alignTrailing {
                teamText(name: name, elo: elo, alignTrailing: true)
                DetailLogo(path: logo, store: logos)
            } else {
                DetailLogo(path: logo, store: logos)
                teamText(name: name, elo: elo, alignTrailing: false)
            }
            if !alignTrailing { Spacer(minLength: 0) }
        }
        .frame(maxWidth: .infinity)
    }

    private func teamText(name: String, elo: Double?, alignTrailing: Bool) -> some View {
        VStack(alignment: alignTrailing ? .trailing : .leading, spacing: 3) {
            Text(name)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .lineLimit(2)
                .multilineTextAlignment(alignTrailing ? .trailing : .leading)
            if let elo {
                Text("ELO \(Int(elo.rounded()))")
                    .font(.system(size: 9.5, weight: .semibold).monospacedDigit())
                    .foregroundStyle(AppTheme.secondaryText)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(AppTheme.surface, in: Capsule())
            }
        }
    }

    private var scoreBlock: some View {
        VStack(spacing: 4) {
            Text(scoreText)
                .font(.system(size: 34, weight: .bold).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText)
            if let note = scoreNote {
                Text(note)
                    .font(.system(size: 9.5, weight: .semibold))
                    .tracking(1)
                    .foregroundStyle(AppTheme.secondaryText)
            }
        }
        .frame(minWidth: 110)
    }

    private var scoreText: String {
        guard let home = detail.homeFt ?? detail.homeEt, let away = detail.awayFt ?? detail.awayEt else {
            return "–"
        }
        return "\(home) – \(away)"
    }

    private var scoreNote: String? {
        switch detail.decidedBy {
        case "et": return "HOSSZABBÍTÁS UTÁN"
        case "pens": return "BÜNTETŐK UTÁN"
        default: return nil
        }
    }

    private var metaRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            infoLine("Verseny", "\(detail.competitionName) · \(detail.seasonLabel)")
            infoLine("Időpont", Self.kickoff.string(from: detail.kickoffAt) +
                     (detail.kickoffPrecision == "date_only" ? " (csak dátum)" : ""))
            if let round = detail.roundLabel {
                infoLine("Forduló", round)
            }
            infoLine("Játékvezető", detail.referee ?? "–")
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    private func infoLine(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(label.uppercased())
                .font(.system(size: 9.5, weight: .semibold))
                .tracking(1.2)
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 110, alignment: .leading)
            Text(value)
                .font(.system(size: 13))
                .foregroundStyle(AppTheme.primaryText)
        }
    }

    private var resultSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionTitle("Eredmény")
            HStack(spacing: 18) {
                if let hh = detail.homeHt, let ah = detail.awayHt {
                    scoreChip("Félidő", "\(hh)–\(ah)")
                }
                if let he = detail.homeEt, let ae = detail.awayEt {
                    scoreChip("120'", "\(he)–\(ae)")
                }
                if let hp = detail.homePens, let ap = detail.awayPens {
                    scoreChip("Büntetők", "\(hp)–\(ap)")
                }
                if detail.homeFt != nil {
                    scoreChip("90'", scoreText)
                }
            }
        }
    }

    private func scoreChip(_ label: String, _ value: String) -> some View {
        VStack(spacing: 3) {
            Text(value)
                .font(.system(size: 16, weight: .semibold).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText)
            Text(label.uppercased())
                .font(.system(size: 8.5, weight: .semibold))
                .tracking(1)
                .foregroundStyle(AppTheme.secondaryText)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private var hasStats: Bool {
        [
            detail.homeShots, detail.awayShots, detail.homeShotsOnTarget,
            detail.awayShotsOnTarget, detail.homeCorners, detail.awayCorners,
            detail.homeYellow, detail.awayYellow, detail.homeRed, detail.awayRed,
            detail.homeFouls, detail.awayFouls,
        ].contains { $0 != nil } || detail.homeXg != nil || detail.awayXg != nil
    }

    private var statsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                sectionTitle("Statisztika")
                Spacer()
                if let source = detail.statsSource {
                    Text(source.uppercased())
                        .font(.system(size: 9, weight: .semibold))
                        .tracking(1)
                        .foregroundStyle(AppTheme.secondaryText)
                }
            }
            VStack(spacing: 0) {
                statRow("Kapura lövés", detail.homeShotsOnTarget, detail.awayShotsOnTarget)
                statRow("Lövés", detail.homeShots, detail.awayShots)
                statRow("Szöglet", detail.homeCorners, detail.awayCorners)
                statRow("Szabálytalanság", detail.homeFouls, detail.awayFouls)
                statRow("Sárga", detail.homeYellow, detail.awayYellow)
                statRow("Piros", detail.homeRed, detail.awayRed)
                if detail.homeXg != nil || detail.awayXg != nil {
                    statRow("xG", detail.homeXg.map { String(format: "%.2f", $0) },
                            detail.awayXg.map { String(format: "%.2f", $0) })
                }
            }
            .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
    }

    private func statRow(_ label: String, _ home: Any?, _ away: Any?) -> some View {
        let h = home.map { "\($0)" } ?? "–"
        let a = away.map { "\($0)" } ?? "–"
        return HStack {
            Text(h).font(.system(size: 12.5, weight: .medium).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText).frame(width: 44, alignment: .leading)
            Text(label)
                .font(.system(size: 11))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(maxWidth: .infinity)
            Text(a).font(.system(size: 12.5, weight: .medium).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText).frame(width: 44, alignment: .trailing)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 6)
    }

    @ViewBuilder
    private var oddsSection: some View {
        if let odds = detail.odds {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    sectionTitle("Odds")
                    Spacer()
                    if let state = detail.oddsTrustState {
                        Text(state.uppercased())
                            .font(.system(size: 9, weight: .bold))
                            .tracking(1)
                            .foregroundStyle(state == "verified" ? .green : .orange)
                    }
                }
                HStack(spacing: 10) {
                    oddsChip("1", odds.home)
                    oddsChip("X", odds.draw)
                    oddsChip("2", odds.away)
                    oddsChip("BTTS I", odds.bttsYes)
                    oddsChip("BTTS N", odds.bttsNo)
                    oddsChip("O 2.5", odds.over25)
                    oddsChip("U 2.5", odds.under25)
                }
                if let source = detail.oddsSource {
                    Text("forrás: \(source)" + (detail.oddsCapturedAt.map { " · \(Self.captured.string(from: $0))" } ?? ""))
                        .font(.system(size: 9.5))
                        .foregroundStyle(AppTheme.secondaryText)
                }
            }
        }
    }

    private func oddsChip(_ label: String, _ price: Double?) -> some View {
        VStack(spacing: 2) {
            Text(price.map { String(format: "%.2f", $0) } ?? "–")
                .font(.system(size: 12.5, weight: .semibold).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText)
            Text(label)
                .font(.system(size: 8.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(AppTheme.secondaryText)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var trustSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionTitle("Ellenőrzés")
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    StatusBadge(state: detail.gateState ?? .unknown)
                    if let version = detail.gateRuleSetVersion {
                        Text("szabálykészlet \(version)")
                            .font(.system(size: 10))
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    if let evaluated = detail.gateEvaluatedAt {
                        Text("kiértékelve: \(Self.captured.string(from: evaluated))")
                            .font(.system(size: 10).monospacedDigit())
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                }
                if detail.gateBlockers.isEmpty {
                    Text("Nincs blokkoló megjegyzés.")
                        .font(.system(size: 11))
                        .foregroundStyle(AppTheme.secondaryText)
                } else {
                    ForEach(Array(detail.gateBlockers.enumerated()), id: \.offset) { _, blocker in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Text(blocker.severity.uppercased())
                                .font(.system(size: 8.5, weight: .bold))
                                .tracking(0.8)
                                .foregroundStyle(blocker.severity == "blocker" ? .red : .orange)
                                .frame(width: 58, alignment: .leading)
                            Text(blocker.message)
                                .font(.system(size: 11))
                                .foregroundStyle(AppTheme.primaryText)
                        }
                    }
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
    }

    private func sectionTitle(_ text: String) -> some View {
        Text(text.uppercased())
            .font(.system(size: 10, weight: .semibold))
            .tracking(1.4)
            .foregroundStyle(AppTheme.secondaryText)
    }
}

private struct DetailLogo: View {
    let path: String?
    let store: LogoStore
    @State private var image: NSImage?

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: .fit)
                    .padding(4)
            } else {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(AppTheme.surface)
            }
        }
        .frame(width: 56, height: 56)
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
        .task(id: path) {
            guard let path else { return }
            image = await store.image(for: path)
        }
    }
}
