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
    let focusedMatch: String?
    let onSelect: (String) -> Void

    /// A kártyasor elrendezésének váltópontja a rendelkezésre álló szélességben
    /// (a kártya a 48 pt oldalpaddel ennél keskenyebb). Ez nem a kártya szélessége:
    /// a kártya mindig kitölti a listapanelt, csak a belső elrendezés vált.
    private static let wideLayoutThreshold: CGFloat = 800

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
            GeometryReader { geometry in
                let isWide = geometry.size.width >= Self.wideLayoutThreshold
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 16) {
                            ForEach(leagues) { league in
                                LeagueSectionView(
                                    league: league,
                                    isWide: isWide,
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
}

private struct LeagueSectionView: View {
    let league: LeagueGroup
    let isWide: Bool
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
                    MatchItemRow(match: match, isWide: isWide,
                                 focusedMatch: focusedMatch, onSelect: onSelect)
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

/// A napi lista kártyája: kezdés, két csapatnév, végeredmény és 1X2 odds.
/// Minden más adat (logó, pozíció, ELO, BTTS, 2,5, piacok, forrás, periódusok)
/// a részletes nézetben él.
private struct MatchItemRow: View {
    let match: MatchCardModel
    let isWide: Bool
    let focusedMatch: String?
    let onSelect: (String) -> Void
    @State private var hover = false

    /// A kezdés-oszlop szélessége: a dátum, az idő és az állapotjelzés alatt
    /// egy vonalban marad. Fix, mert ez az oszlop minden sornál ugyanaz.
    private let kickoffWidth: CGFloat = 82

    var body: some View {
        Group {
            if isWide {
                wideLayout
            } else {
                stackedLayout
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
                homeScore: match.finalScore?.home,
                awayScore: match.finalScore?.away,
                publishState: match.publishState,
                dayLabel: match.dayLabel.isEmpty ? nil : match.dayLabel,
                odds: match.oddsAccessibilityText
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

    // MARK: - Elrendezések

    /// Széles: kezdés | csapatok | eredmény | 1X2 egy sorban; a csapatoszlop
    /// viszi a maradék helyet, így az eredmény és az odds jobbra igazodik.
    private var wideLayout: some View {
        HStack(alignment: .center, spacing: 16) {
            kickoffColumn
            teamsColumn
                .frame(maxWidth: .infinity, alignment: .leading)
            scoreColumn
            oddsBlock
        }
    }

    /// Szűk: az odds a csapatok és az eredmény alá tördelve.
    private var stackedLayout: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .center, spacing: 16) {
                kickoffColumn
                teamsColumn
                    .frame(maxWidth: .infinity, alignment: .leading)
                scoreColumn
            }
            if match.hasOdds {
                oddsBlock
                    .padding(.leading, kickoffWidth + 16)
            }
        }
    }

    // MARK: - Kártyaelemek

    private var kickoffColumn: some View {
        VStack(alignment: .leading, spacing: 3) {
            if !match.dayLabel.isEmpty {
                Text(match.dayLabel)
                    .font(.system(size: 11, weight: .medium).monospacedDigit())
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(1)
            }
            Text(match.time)
                .font(.system(size: 13, weight: .semibold).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText)
            if let phase = match.phaseLabel {
                HStack(spacing: 4) {
                    if match.isLive { LiveDot() }
                    Text(phase)
                        .font(.system(size: 9.5, weight: .bold))
                        .tracking(0.6)
                        .foregroundStyle(match.isLive ? AppTheme.liveGreen : AppTheme.secondaryText)
                    if let minute = match.minuteText {
                        Text(minute)
                            .font(.system(size: 9.5, weight: .bold).monospacedDigit())
                            .foregroundStyle(AppTheme.liveGreen)
                    }
                }
            }
        }
        .frame(width: kickoffWidth, alignment: .leading)
    }

    /// Csak a két csapatnév; logó, pozíció és ELO nem kerül a kártyára.
    private var teamsColumn: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(match.home)
                .font(.appText(15, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
            Text(match.away)
                .font(.appText(15, weight: .medium))
                .foregroundStyle(AppTheme.primaryText.opacity(0.9))
        }
        .lineLimit(1)
        .minimumScaleFactor(0.8)
    }

    private var scoreColumn: some View {
        Text(match.scoreText)
            .font(.scoreboard(19, weight: .bold))
            .foregroundStyle(match.finalScore == nil ? AppTheme.secondaryText : AppTheme.primaryText)
            .lineLimit(1)
            .frame(minWidth: 56, alignment: .trailing)
    }

    /// Csak az 1X2 három pillje; BTTS, 2,5 és a piacok a részletes nézetben.
    @ViewBuilder
    private var oddsBlock: some View {
        if match.hasOdds {
            HStack(spacing: 6) {
                ForEach(match.oddsPills) { pill in
                    oddsPill(pill)
                }
            }
            .fixedSize(horizontal: true, vertical: false)
        }
    }

    private func oddsPill(_ pill: OddsPill) -> some View {
        HStack(spacing: 4) {
            Text(pill.label)
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(pill.isWinner ? AppTheme.accent : AppTheme.secondaryText)
            Text(pill.value)
                .font(.system(size: 12.5, weight: pill.isWinner ? .bold : .medium).monospacedDigit())
                .foregroundStyle(pill.isWinner ? AppTheme.accent : AppTheme.primaryText.opacity(0.85))
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(
            pill.isWinner ? AppTheme.accent.opacity(0.16) : AppTheme.surface,
            in: RoundedRectangle(cornerRadius: 7, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .strokeBorder(pill.isWinner ? AppTheme.accent.opacity(0.5) : Color.clear)
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