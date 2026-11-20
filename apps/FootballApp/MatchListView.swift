import SwiftUI
import Foundation
import FootballAppCore

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
///
/// Billentyűzet: a Tab-sorrendet az app szintje (`HomeView` + `AppFocus`)
/// tartja, ide csak a fókuszált meccs azonosítója érkezik; ↑ / ↓ a kártyák
/// között lép, Return megnyitja a részletezőt, Escape törli a fókuszt.
/// A fókusz vizuális jelzése a kártya körüli borostyán keret.
struct MatchListView: View {
    let leagues: [LeagueGroup]
    let logos: LogoStore
    let focusedMatch: String?
    let onSelect: (String) -> Void

    private var orderedIDs: [String] {
        leagues.flatMap { league in
            league.matches.sorted { ($0.time, $0.id) < ($1.time, $1.id) }.map(\.id)
        }
    }

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
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Nincs meccs a kiválasztott szűrőben.")
            .accessibilityIdentifier("match.list.empty")
        } else {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 16) {
                        ForEach(leagues) { league in
                            LeagueSectionView(
                                league: league,
                                logos: logos,
                                focusedMatch: focusedMatch,
                                onSelect: onSelect
                            )
                        }
                    }
                    .padding(.horizontal, 48)
                    .padding(.vertical, 16)
                }
                .onChange(of: focusedMatch) { _, newValue in
                    guard let newValue else { return }
                    withAnimation(.easeOut(duration: 0.12)) {
                        proxy.scrollTo(newValue, anchor: .center)
                    }
                }
            }
        }
    }

}

private struct LeagueSectionView: View {
    let league: LeagueGroup
    let logos: LogoStore
    let focusedMatch: String?
    let onSelect: (String) -> Void

    private var sortedMatches: [MatchCardModel] {
        // Azonos kezdésnél az id dönt, hogy a fókuszsorrend és a kirajzolt
        // sorrend ugyanaz legyen (a Swift rendezés nem stabil).
        league.matches.sorted { ($0.time, $0.id) < ($1.time, $1.id) }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Text(league.flag)
                    .font(.system(size: 20))
                VStack(alignment: .leading, spacing: 2) {
                    Text(league.name)
                        .font(.appText(15, weight: .semibold))
                        .foregroundStyle(AppTheme.primaryText)
                    Text(league.subtitle)
                        .font(.appLabel())
                        .tracking(1.2)
                        .foregroundStyle(AppTheme.secondaryText)
                }
                Spacer()
                Text("\(league.matches.count) meccs")
                    .font(.appLabel(10))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .background(AppTheme.surface)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("\(league.name), \(league.subtitle), \(league.matches.count) meccs")
            .accessibilityIdentifier("league.section.\(league.id)")

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            VStack(spacing: 0) {
                ForEach(Array(sortedMatches.enumerated()), id: \.element.id) { index, match in
                    MatchItemRow(match: match, showPositions: league.tier != 0,
                                 logos: logos, focusedMatch: focusedMatch, onSelect: onSelect)
                        .id(match.id)
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
    let match: MatchCardModel
    let showPositions: Bool
    let logos: LogoStore
    let focusedMatch: String?
    let onSelect: (String) -> Void
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
                    teamLine(match.home, position: match.homePosition,
                             logo: match.homeLogo, elo: match.homeElo)
                    teamLine(match.away, position: match.awayPosition,
                             logo: match.awayLogo, elo: match.awayElo)
                }

                Spacer(minLength: 12)

                resultBlock
            }

            if match.odds != nil {
                oddsRow
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .background(hover ? AppTheme.surface : Color.clear)
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture { onSelect(match.id) }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            AccessibilityLabels.matchCardLabel(
                home: match.home,
                away: match.away,
                time: match.time,
                status: match.status,
                homeScore: match.homeScore,
                awayScore: match.awayScore,
                publishState: match.publishState
            )
        )
        .accessibilityAddTraits(focusedMatch == match.id ? [.isButton, .isSelected] : [.isButton])
        .accessibilityIdentifier("match.card.\(match.id)")
        .overlay {
            if focusedMatch == match.id {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(AppTheme.accent, lineWidth: 2)
                    .padding(3)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
            }
        }
    }

    private var statusColumn: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 5) {
                if group == .live { LiveDot() }
                Text(leadingLabel)
                    .font(.scoreboard(13, weight: .semibold))
                    .foregroundStyle(leadingColor)
            }
            if let minute = match.minute {
                Text("\(minute)'")
                    .font(.scoreboard(11, weight: .medium))
                    .foregroundStyle(AppTheme.liveGreen.opacity(0.85))
            }
        }
        .frame(width: 56, alignment: .leading)
    }

    private func teamLine(_ name: String, position: Int?, logo: String?, elo: Int?) -> some View {
        HStack(spacing: 8) {
            TeamLogo(name: name, path: logo, store: logos)

            Text(name)
                .font(.appText(15, weight: .medium))
                .foregroundStyle(AppTheme.primaryText)
                .lineLimit(1)

            if showPositions, let position {
                Text("(\(position))")
                    .font(.system(size: 11, weight: .medium).monospacedDigit())
                    .foregroundStyle(AppTheme.secondaryText)
            }

            if let elo {
                Text("ELO \(elo)")
                    .font(.appLabel(8.5).monospacedDigit())
                    .foregroundStyle(AppTheme.secondaryText)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(AppTheme.surface, in: Capsule())
            }
        }
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
                        .font(.scoreboard(item.main ? 19 : 13, weight: item.main ? .bold : .medium))
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
                Text("forrás: \(match.oddsSource)")
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

struct LiveDot: View {
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

/// Csapat-logó: a helyi API-ból (tokenes) vagy külső URL-ről; betöltés alatt
/// és hiba esetén a korábbi semleges helyőrző látszik.
private struct TeamLogo: View {
    /// A címer nélküli csapat monogramja: a név első betűje(i), hogy a sor
    /// üres keret helyett is kész megjelenést kapjon.
    var name: String = ""
    let path: String?
    let store: LogoStore
    @State private var image: NSImage?

    private var monogram: String {
        let words = name.split(separator: " ")
        if words.count >= 2 {
            let letters = words.prefix(2).compactMap { $0.first }
            return String(letters).uppercased()
        }
        guard let word = words.first else { return "?" }
        return String(word.prefix(3)).uppercased()
    }

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: .fit)
                    .padding(1.5)
            } else {
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(AppTheme.surface)
                    .overlay(
                        Text(monogram)
                            .font(.appLabel(8.5))
                            .foregroundStyle(AppTheme.secondaryText)
                    )
            }
        }
        .frame(width: 24, height: 24)
        .overlay(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
        .task(id: path) {
            guard let path else { return }
            image = await store.image(for: path)
        }
    }
}
