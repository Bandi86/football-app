import SwiftUI
import Foundation

/// Státusz-szűrő a napi listához, darabszámmal.
enum MatchFilter: String, CaseIterable, Identifiable {
    case all, upcoming, live, finished

    var id: String { rawValue }

    var title: String {
        switch self {
        case .all: return "Mind"
        case .upcoming: return "Következik"
        case .live: return "Élő"
        case .finished: return "Befejezett"
        }
    }

    func matches(status: String) -> Bool {
        switch self {
        case .all:
            return true
        case .upcoming:
            return ["scheduled", "confirmed", "postponed"].contains(status)
        case .live:
            return ["live", "halftime"].contains(status)
        case .finished:
            return ["finished", "awarded"].contains(status)
        }
    }

    static func group(of status: String) -> MatchFilter? {
        [MatchFilter.upcoming, .live, .finished].first { $0.matches(status: status) }
    }
}

/// Liga szerint csoportosított, prioritás szerint rendezett napi lista.
struct MatchListView: View {
    let leagues: [SampleLeague]

    var body: some View {
        if leagues.isEmpty {
            VStack(spacing: 12) {
                Image(systemName: "line.3.horizontal.decrease.circle")
                    .font(.largeTitle)
                    .foregroundStyle(AppTheme.secondaryText)
                Text("Ebben a szűrőben nincs meccs.")
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollView {
                LazyVStack(spacing: 16) {
                    ForEach(leagues) { league in
                        LeagueSectionView(league: league)
                    }
                }
                .padding(.horizontal, 48)
                .padding(.vertical, 16)
            }
        }
    }
}

private struct LeagueSectionView: View {
    let league: SampleLeague

    private var sortedMatches: [SampleMatch] {
        league.matches.sorted { $0.time < $1.time }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Text(league.flag)
                    .font(.system(size: 20))
                VStack(alignment: .leading, spacing: 2) {
                    Text(league.name)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(AppTheme.primaryText)
                    Text("\(league.country.uppercased()) · \(league.divisionLabel.uppercased())")
                        .font(.system(size: 10, weight: .semibold))
                        .tracking(1.2)
                        .foregroundStyle(AppTheme.secondaryText)
                }
                Spacer()
                Text("\(league.matches.count) meccs")
                    .font(.system(size: 11.5))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .background(AppTheme.surface)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            VStack(spacing: 0) {
                ForEach(Array(sortedMatches.enumerated()), id: \.element.id) { index, match in
                    MatchItemRow(match: match, showPositions: league.tier != 0)
                    if index < sortedMatches.count - 1 {
                        Rectangle().fill(AppTheme.hairline).frame(height: 1).padding(.leading, 18)
                    }
                }
            }
        }
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}

private struct MatchItemRow: View {
    let match: SampleMatch
    let showPositions: Bool
    @State private var hover = false

    private var group: MatchFilter? { MatchFilter.group(of: match.status) }

    private var leadingLabel: String {
        switch group {
        case .live: return "ÉLŐ"
        case .finished: return "FT"
        default: return match.time
        }
    }

    private var leadingColor: Color {
        switch group {
        case .live: return AppTheme.liveGreen
        case .finished: return AppTheme.secondaryText
        default: return AppTheme.primaryText
        }
    }

    /// Az állások időben visszafelé: 11-es → 120' → 90' → HT (amelyik van).
    private var periodResults: [(label: String, score: String, main: Bool)] {
        var items: [(String, String)] = []
        if let hp = match.homePens, let ap = match.awayPens {
            items.append(("11-es", "\(hp)–\(ap)"))
        }
        if let eh = match.homeEt, let ee = match.awayEt {
            items.append(("120'", "\(eh)–\(ee)"))
        }
        if let h = match.homeScore, let a = match.awayScore {
            let label = (match.homeEt != nil || match.homePens != nil) ? "90'" : ""
            items.append((label, "\(h)–\(a)"))
        }
        if let hh = match.homeHt, let ah = match.awayHt {
            items.append(("HT", "\(hh)–\(ah)"))
        }
        return items.enumerated().map { index, item in (item.0, item.1, index == 0) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 11) {
            HStack(alignment: .top, spacing: 16) {
                statusColumn

                VStack(alignment: .leading, spacing: 9) {
                    teamLine(match.home)
                    teamLine(match.away)
                }

                Spacer(minLength: 12)

                resultBlock
            }

            predictionBlock
            oddsRow
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .background(hover ? AppTheme.surface : Color.clear)
        .onHover { hover = $0 }
    }

    private var statusColumn: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 5) {
                if group == .live { LiveDot() }
                Text(leadingLabel)
                    .font(.system(size: 13, weight: .semibold).monospacedDigit())
                    .foregroundStyle(leadingColor)
            }
            if let minute = match.minute {
                Text("\(minute)'")
                    .font(.system(size: 11).monospacedDigit())
                    .foregroundStyle(AppTheme.liveGreen.opacity(0.85))
            }
        }
        .frame(width: 56, alignment: .leading)
    }

    private func teamLine(_ name: String) -> some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(AppTheme.surface)
                .frame(width: 24, height: 24)
                .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(AppTheme.hairline))

            Text(name)
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(AppTheme.primaryText)
                .lineLimit(1)

            if showPositions, let position = SampleData.position(of: name) {
                Text("(\(position))")
                    .font(.system(size: 11, weight: .medium).monospacedDigit())
                    .foregroundStyle(AppTheme.secondaryText)
            }
        }
    }

    @ViewBuilder
    private var predictionBlock: some View {
        if let prediction = match.prediction {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 12) {
                    Text("ELŐREJELZÉS")
                        .font(.system(size: 9, weight: .semibold))
                        .tracking(1)
                        .foregroundStyle(AppTheme.secondaryText.opacity(0.6))
                    Text("Hazai \(prediction.homePct)%")
                        .foregroundStyle(AppTheme.predictionHome)
                    Text("Döntetlen \(prediction.drawPct)%")
                        .foregroundStyle(AppTheme.predictionDraw)
                    Text("Vendég \(prediction.awayPct)%")
                        .foregroundStyle(AppTheme.predictionAway)
                }
                .font(.system(size: 11, weight: .medium))

                ProbabilityBar(home: prediction.homePct, draw: prediction.drawPct, away: prediction.awayPct)
                    .frame(height: 6)

                HStack(spacing: 14) {
                    predItem("Több 2,5", "\(prediction.over25Pct)%")
                    predItem("BTTS", "\(prediction.bttsPct)%")
                    predItem("Pontos", "\(prediction.exactHome)–\(prediction.exactAway) (\(prediction.exactPct)%)")
                }
            }
            .padding(.leading, 72)
        }
    }

    private func predItem(_ label: String, _ value: String) -> some View {
        HStack(spacing: 4) {
            Text(label)
                .foregroundStyle(AppTheme.secondaryText)
            Text(value)
                .foregroundStyle(AppTheme.primaryText)
                .monospacedDigit()
        }
        .font(.system(size: 11))
    }

    private var resultBlock: some View {
        VStack(alignment: .trailing, spacing: 2) {
            ForEach(Array(periodResults.enumerated()), id: \.offset) { _, item in
                HStack(spacing: 7) {
                    if !item.label.isEmpty {
                        Text(item.label)
                            .font(.system(size: 10.5, weight: .medium))
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    Text(item.score)
                        .font(.system(size: item.main ? 18 : 12.5, weight: item.main ? .bold : .medium).monospacedDigit())
                        .foregroundStyle(item.main ? AppTheme.primaryText : AppTheme.secondaryText)
                }
            }
        }
        .frame(minWidth: 64, alignment: .trailing)
    }

    private var oddsRow: some View {
        HStack(alignment: .center, spacing: 16) {
            oddsGroup("1X2", [
                ("1", match.odds?.home, match.winner1x2 == .home),
                ("X", match.odds?.draw, match.winner1x2 == .draw),
                ("2", match.odds?.away, match.winner1x2 == .away),
            ])
            oddsGroup("BTTS", [
                ("I", match.odds?.bttsYes, match.bttsYesWon == true),
                ("N", match.odds?.bttsNo, match.bttsYesWon == false),
            ])
            oddsGroup("2,5", [
                ("O", match.odds?.over25, match.over25Won == true),
                ("U", match.odds?.under25, match.over25Won == false),
            ])
            Spacer(minLength: 8)
            VStack(alignment: .trailing, spacing: 1) {
                Text("Piacok (\(match.markets)) ›")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(AppTheme.secondaryText.opacity(0.85))
                Text("forrás: \(SampleData.oddsSource)")
                    .font(.system(size: 9.5))
                    .foregroundStyle(AppTheme.secondaryText.opacity(0.6))
            }
        }
        .padding(.leading, 72)
    }

    private func oddsGroup(_ title: String, _ items: [(String, Double?, Bool)]) -> some View {
        HStack(spacing: 6) {
            Text(title)
                .font(.system(size: 9, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(AppTheme.secondaryText.opacity(0.65))
            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                oddsPill(item.0, item.1, isWinner: item.2)
            }
        }
    }

    private func oddsPill(_ label: String, _ value: Double?, isWinner: Bool) -> some View {
        HStack(spacing: 4) {
            Text(label)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(isWinner ? AppTheme.accent : AppTheme.secondaryText)
            Text(value.map { String(format: "%.2f", $0).replacingOccurrences(of: ".", with: ",") } ?? "–")
                .font(.system(size: 12.5, weight: isWinner ? .bold : .medium).monospacedDigit())
                .foregroundStyle(isWinner ? AppTheme.accent : AppTheme.primaryText.opacity(0.85))
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(
            isWinner ? AppTheme.accent.opacity(0.16) : AppTheme.surface,
            in: RoundedRectangle(cornerRadius: 7, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .strokeBorder(isWinner ? AppTheme.accent.opacity(0.5) : Color.clear)
        )
    }
}

private struct LiveDot: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var on = false

    var body: some View {
        Circle()
            .fill(AppTheme.liveGreen)
            .frame(width: 6, height: 6)
            .opacity(on ? 1 : 0.3)
            .onAppear {
                guard !reduceMotion else { return }
                withAnimation(.easeInOut(duration: 0.9).repeatForever(autoreverses: true)) {
                    on = true
                }
            }
    }
}

private struct ProbabilityBar: View {
    let home: Int
    let draw: Int
    let away: Int

    var body: some View {
        GeometryReader { geo in
            let gap: CGFloat = 2
            let total = CGFloat(max(1, home + draw + away))
            let content = max(0, geo.size.width - gap * 2)
            HStack(spacing: gap) {
                segment(content * CGFloat(home) / total, AppTheme.predictionHome)
                segment(content * CGFloat(draw) / total, AppTheme.predictionDraw)
                segment(content * CGFloat(away) / total, AppTheme.predictionAway)
            }
        }
    }

    private func segment(_ width: CGFloat, _ color: Color) -> some View {
        RoundedRectangle(cornerRadius: 3, style: .continuous)
            .fill(color)
            .frame(width: width)
    }
}
