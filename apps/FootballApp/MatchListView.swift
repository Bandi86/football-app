import SwiftUI
import Foundation
import FootballAppCore

/// Napi lista szűrője, darabszámmal.
///
/// A státusz-szűrők mellett külön eset van a lejárt, de még nyitott meccsekre
/// (`openPast`, a szerver `open_past` fogalma), és hiány-szűrők a hiány-panel
/// minden kattintható eleméhez. A hiányzó eredmény (`missingResult`) csak a
/// lezárt, de végeredmény nélküli meccseket jelenti — nem azonos a lejárt,
/// nyitott meccsekkel.
enum MatchFilter: String, CaseIterable, Identifiable {
    case all, upcoming, live, finished, openPast
    case missingKickoff, missingResult, missingOdds, missingLogo
    case missingCompetitionLogo, withheld

    var id: String { rawValue }

    /// A főoldali szűrősor egy helyen mutatja az összes szűrőt.
    static let homeFilters = allCases

    var title: String {
        switch self {
        case .all: return "Mind"
        case .upcoming: return "Következik"
        case .live: return "Élő"
        case .finished: return "Befejezett"
        case .openPast: return "Lejárt, nyitott"
        case .missingKickoff: return "Idő nélkül"
        case .missingResult: return "Eredmény nélkül"
        case .missingOdds: return "Odds nélkül"
        case .missingLogo: return "Logó nélkül"
        case .missingCompetitionLogo: return "Ligalogó nélkül"
        case .withheld: return "Visszatartva"
        }
    }

    /// A „Következik" tűréshatára a kezdési időnél: a forrás frissítése
    /// késhet néhány percet, de egy lejárt meccset nem szabad következőnek
    /// mutatni csak mert a státusza még mindig `scheduled` (S131).
    static let upcomingGrace: TimeInterval = 5 * 60

    /// Hiány-szűrő: nem státusz alapján dönt, ezért a státusz-alapú
    /// csoportosításból (kártya-szín, "Következik/Élő/Befejezett") kimarad.
    var isGapFilter: Bool {
        switch self {
        case .missingKickoff, .missingResult, .missingOdds, .missingLogo,
             .missingCompetitionLogo, .withheld:
            return true
        default:
            return false
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
        case .openPast, .missingKickoff, .missingResult, .missingOdds, .missingLogo,
             .missingCompetitionLogo, .withheld:
            return false
        }
    }

    /// A kártya szintű döntés, a hiány-szűrőket is beleértve.
    func matches(_ match: DayMatch, now: Date = Date()) -> Bool {
        switch self {
        case .upcoming:
            // S131: a státusz önmagában nem elég — a 16:15-ös frissítéskor
            // `scheduled` meccsek a 18:15-ös frissítésig elavultak lehetnek.
            guard matches(status: match.status) else { return false }
            return match.kickoffAt > now.addingTimeInterval(-Self.upcomingGrace)
        case .live:
            // „Élő" csak 150 percig: a backend elavult live státuszt hagyhat maga után.
            return MatchCardModel.isLiveNow(status: match.status,
                                            kickoffAt: match.kickoffAt, now: now)
        case .openPast:
            return MatchCardModel.isOpenPast(status: match.status,
                                             kickoffAt: match.kickoffAt,
                                             precision: match.kickoffPrecision, now: now)
        case .missingKickoff:
            return match.kickoffPrecision == "date_only"
        case .missingResult:
            let finalScore = MatchCardModel.finalScore(
                homeScore: match.homeScore, awayScore: match.awayScore,
                homeEt: match.homeEt, awayEt: match.awayEt)
            return MatchCardModel.isMissingResult(status: match.status, finalScore: finalScore)
        case .missingOdds:
            return match.odds == nil
        case .missingLogo:
            return (match.homeLogo ?? "").isEmpty || (match.awayLogo ?? "").isEmpty
        case .missingCompetitionLogo:
            return (match.competitionLogo ?? "").isEmpty
        case .withheld:
            return match.publishState == .withheld
        default:
            return matches(status: match.status)
        }
    }

    /// Ugyanaz a döntés a megjelenítési modellen (a csoportosítás ezt használja).
    func matches(_ card: MatchCardModel, now: Date = Date()) -> Bool {
        switch self {
        case .upcoming:
            guard matches(status: card.status) else { return false }
            return card.kickoffAt > now.addingTimeInterval(-Self.upcomingGrace)
        case .live:
            return MatchCardModel.isLiveNow(status: card.status,
                                            kickoffAt: card.kickoffAt, now: now)
        case .openPast:
            return MatchCardModel.isOpenPast(status: card.status,
                                             kickoffAt: card.kickoffAt,
                                             precision: card.kickoffPrecision, now: now)
        case .missingKickoff:
            return card.kickoffPrecision == "date_only"
        case .missingResult:
            return MatchCardModel.isMissingResult(status: card.status,
                                                  finalScore: card.finalScore)
        case .missingOdds:
            return card.odds == nil
        case .missingLogo:
            return (card.homeLogo ?? "").isEmpty || (card.awayLogo ?? "").isEmpty
        case .missingCompetitionLogo:
            return (card.competitionLogo ?? "").isEmpty
        case .withheld:
            return card.publishState == .withheld
        default:
            return matches(status: card.status)
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
    /// A kedvenc versenyek kulcsai (a szekciófejléc csillagához).
    var favorites: Set<String> = []
    /// A kedvenc be-/kikapcsolása; `nil` esetén nincs csillag a fejlécben.
    var onToggleFavorite: ((String) -> Void)? = nil
    /// Az összecsukott verseny-szekciók kulcsai.
    var collapsed: Set<String> = []
    /// A szekció összecsukása/kinyitása; `nil` esetén nincs nyíl a fejlécben.
    var onToggleCollapse: ((String) -> Void)? = nil
    /// A becsült óra időbélyege; 30 mp-enként újraértékeljük a kártyákat.
    @State private var now = Date()

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
                                    logos: logos,
                                    now: now,
                                    isWide: isWide,
                                    focusedMatch: focusedMatch,
                                    onSelect: onSelect,
                                    isFavorite: favorites.contains(league.id),
                                    onToggleFavorite: onToggleFavorite.map { toggle in
                                        { toggle(league.id) }
                                    },
                                    isCollapsed: collapsed.contains(league.id),
                                    onToggleCollapse: onToggleCollapse.map { toggle in
                                        { toggle(league.id) }
                                    }
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
                .onReceive(Timer.publish(every: 30, on: .main, in: .common).autoconnect()) {
                    now = $0
                }
            }
        }
    }
}

private struct LeagueSectionView: View {
    let league: LeagueGroup
    let logos: LogoStore
    let now: Date
    let isWide: Bool
    let focusedMatch: String?
    let onSelect: (String) -> Void
    let isFavorite: Bool
    let onToggleFavorite: (() -> Void)?
    let isCollapsed: Bool
    let onToggleCollapse: (() -> Void)?

    private var sortedMatches: [MatchCardModel] {
        // Azonos kezdésnél az id dönt, hogy a fókuszsorrend és a kirajzolt
        // sorrend ugyanaz legyen (a Swift rendezés nem stabil).
        league.matches.sorted { ($0.time, $0.id) < ($1.time, $1.id) }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 16) {
                if let logo = league.logo {
                    CompetitionLogo(path: logo, store: logos, fallbackFlag: league.countryCode, size: 40)
                } else {
                    Text(league.flag)
                        .font(.system(size: 25))
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(league.name)
                        .font(.appText(16, weight: .semibold))
                        .foregroundStyle(AppTheme.primaryText)
                    HStack(spacing: 8) {
                        Text(league.subtitle)
                            .font(.appLabel())
                            .tracking(1.2)
                            .foregroundStyle(AppTheme.secondaryText)
                        if let round = league.roundSummary {
                            Text("·")
                                .font(.appLabel())
                                .foregroundStyle(AppTheme.secondaryText.opacity(0.6))
                                .accessibilityHidden(true)
                            Text(round)
                                .font(.appLabel(10))
                                .foregroundStyle(AppTheme.secondaryText)
                        }
                    }
                }
                Spacer()
                Text("\(league.matches.count) meccs")
                    .font(.appLabel(10))
                    .foregroundStyle(AppTheme.secondaryText)
                if let onToggleCollapse {
                    Button(action: onToggleCollapse) {
                        Image(systemName: isCollapsed ? "chevron.right" : "chevron.down")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(AppTheme.secondaryText)
                            .frame(width: 18, height: 20)
                    }
                    .buttonStyle(.plain)
                    .help(isCollapsed ? "Kinyitás" : "Összecsukás")
                    .accessibilityLabel(isCollapsed ? "\(league.name) kinyitása" : "\(league.name) összecsukása")
                    .accessibilityIdentifier("league.collapse.\(league.id)")
                }
                if let onToggleFavorite {
                    Button(action: onToggleFavorite) {
                        Image(systemName: isFavorite ? "star.fill" : "star")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(isFavorite ? AppTheme.accent : AppTheme.secondaryText)
                            .frame(width: 20, height: 20)
                    }
                    .buttonStyle(.plain)
                    .help(isFavorite ? "Eltávolítás a kedvencek közül" : "Hozzáadás a kedvencekhez")
                    .accessibilityLabel(isFavorite
                        ? "\(league.name) eltávolítása a kedvencek közül"
                        : "\(league.name) hozzáadása a kedvencekhez")
                    .accessibilityIdentifier("league.favorite.\(league.id)")
                }
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 14)
            .background(AppTheme.surface)
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("league.section.\(league.id)")

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            if !isCollapsed {
                VStack(spacing: 0) {
                    ForEach(Array(sortedMatches.enumerated()), id: \.element.id) { index, match in
                    MatchItemRow(match: match, logos: logos, now: now, isWide: isWide,
                                 focusedMatch: focusedMatch, onSelect: onSelect)
                        .id(match.id)
                        if index < sortedMatches.count - 1 {
                            Rectangle().fill(AppTheme.hairline).frame(height: 1).padding(.leading, 18)
                        }
                    }
                }
            }
        }
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 19, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 19, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
        .clipShape(RoundedRectangle(cornerRadius: 19, style: .continuous))
    }
}

/// A napi lista kártyája: kezdés, két csapatnév, végeredmény és 1X2 odds.
/// Minden más adat (logó, pozíció, ELO, BTTS, 2,5, piacok, forrás, periódusok)
/// a részletes nézetben él.
private struct MatchItemRow: View {
    let match: MatchCardModel
    let logos: LogoStore
    let now: Date
    let isWide: Bool
    let focusedMatch: String?
    let onSelect: (String) -> Void
    @State private var hover = false

    /// A kezdés-oszlop szélessége: a dátum, az idő és az állapotjelzés alatt
    /// egy vonalban marad. Fix, mert ez az oszlop minden sornál ugyanaz.
    private let kickoffWidth: CGFloat = 96

    private var clock: MatchCardModel.ClockDisplay? { match.clock(at: now) }

    var body: some View {
        Group {
            if isWide {
                wideLayout
            } else {
                stackedLayout
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 18)
        // Az egér fölé vitelén a sor finoman kiemelkedik: enyhe borostyán
        // tónus, hajszál szegély és mikro-skálázás. A billentyűfókusz saját,
        // vastagabb borostyán kerete ettől függetlenül látszik.
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(hover ? AppTheme.accent.opacity(0.08) : Color.clear)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(hover ? AppTheme.accent.opacity(0.24) : Color.clear, lineWidth: 1)
        )
        .scaleEffect(hover ? 1.003 : 1)
        .animation(.easeOut(duration: 0.16), value: hover)
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture { onSelect(match.id) }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            AccessibilityLabels.matchCardLabel(
                home: match.home,
                away: match.away,
                time: match.time,
                status: match.displayStatus(at: now),
                homeScore: match.finalScore?.home,
                awayScore: match.finalScore?.away,
                publishState: match.publishState,
                dayLabel: match.dayLabel.isEmpty ? nil : match.dayLabel,
                kickoffNote: match.kickoffNote,
                round: match.roundText,
                odds: match.oddsAccessibilityText,
                clockText: clock?.text,
                clockIsEstimated: clock?.isEstimated ?? false
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

    /// Látható réteg tudatosan egyszerű (S207): csak a kezdési időpont,
    /// lezárt meccsnél `FT`, élőnél a becsült óra. A dátum, a fázis és a
    /// minuta a VoiceOver AX-címkében továbbra is megvan.
    private var kickoffColumn: some View {
        VStack(alignment: .leading, spacing: 2) {
            if match.isFinished {
                Text("FT")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(AppTheme.secondaryText)
                    .accessibilityIdentifier("match.ft.\(match.id)")
            } else if let clock {
                HStack(spacing: 4) {
                    if clock.isEstimated { LiveDot() }
                    Text(clock.text)
                        .font(.system(size: 13, weight: .semibold).monospacedDigit())
                        .foregroundStyle(AppTheme.liveGreen)
                        .accessibilityIdentifier("match.clock.\(match.id)")
                }
            } else if let note = match.kickoffNote {
                Text(note)
                    .font(.system(size: 9.5, weight: .medium))
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(2)
            } else {
                Text(match.timeDisplay)
                    .font(.system(size: 13, weight: .semibold).monospacedDigit())
                    .foregroundStyle(AppTheme.primaryText)
            }
        }
        .frame(width: kickoffWidth, alignment: .leading)
    }

    /// Két csapatnév a címerrel. A forduló/kupa-felirat kizárólag a liga
    /// szekciófejlécében jelenik meg láthatóan; a kártyán a felolvasott címke
    /// hordozza (`AccessibilityLabels.matchCardLabel`).
    private var teamsColumn: some View {
        VStack(alignment: .leading, spacing: 4) {
            teamLine(match.home, logo: match.homeLogo, weight: .semibold)
            teamLine(match.away, logo: match.awayLogo, weight: .medium)
        }
        .lineLimit(1)
        .minimumScaleFactor(0.8)
    }

    private func teamLine(_ name: String, logo: String?, weight: Font.Weight) -> some View {
        HStack(spacing: 12) {
            CrestLogo(name: name, path: logo, store: logos, size: 26)
            Text(name)
                .font(.appText(16, weight: weight))
                .foregroundStyle(AppTheme.primaryText.opacity(weight == .semibold ? 1 : 0.9))
        }
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
