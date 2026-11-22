import SwiftUI
import AppKit
import FootballAppCore

/// A főoldal: felső navigáció → változó HEADER → szűrő → liga szerint csoportosított
/// napi lista, a footballd API valós válaszából.
struct SelectedMatch: Identifiable {
    let id: UUID
}

enum AppTab: Equatable {
    case day
    case ops
    case competitions
    case teams
}

struct HomeView: View {
    @State private var model = DayModel()
    @State private var filter: MatchFilter = .all
    @State private var country: String? = nil
    @State private var division: DivisionFilter? = nil
    @State private var searchText = ""
    @State private var womensOnly = false
    @State private var favoritesOnly = false
    /// Összecsukott verseny-szekciók (UserDefaults-ból, hogy megmaradjon).
    @State private var collapsed: Set<String> = LeagueCollapse().load()
    @State private var favorites: Set<String> = LeagueFavorites().load()
    @State private var tab: AppTab = .day
    @State private var featuredOnly = false
    @State private var selected: SelectedMatch?
    @State private var focus: AppFocus?
    @State private var showCalendar = false

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var appeared = false

    var body: some View {
        @Bindable var model = model
        GeometryReader { geometry in
            VStack(spacing: 0) {
                TopNavigationBar(selected: tab, focused: focus,
                                 compact: geometry.size.width < 760) { tab = $0 }

                Rectangle().fill(AppTheme.hairline).frame(height: 1)

                switch tab {
                case .ops:
                    OperationsView()
                case .competitions:
                    CompetitionsView()
                case .teams:
                    TeamsView()
                case .day:
                    dayContent(model: model, width: geometry.size.width)
                }
            }
        }
        .background(AppTheme.background)
        .background(KeyboardMonitor { keyCode, modifiers in
            handle(keyCode: keyCode, modifiers: modifiers)
        })
        .onAppear {
            withAnimation(reduceMotion ? nil : .easeOut(duration: 0.4)) { appeared = true }
        }
        .task(id: model.selectedDate) { await model.load() }
        // Élő meccseknél percenként csendben frissítünk; a háttér-kör nem
        // villantja fel a betöltés-jelzőt, és hiba esetén megtartja a listát.
        .task(id: autoRefreshKey) {
            guard liveMatchCount > 0 else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(60))
                if Task.isCancelled { return }
                await model.refresh()
            }
        }
        .onChange(of: tab) { _, newTab in
            // Fülváltásnál a lista-állomás eltűnik, a navigációs fókusz marad.
            if case .match = focus { focus = .tab(newTab) }
        }
        .onChange(of: matchIDs) { _, ids in
            if case .match(let id) = focus, !ids.contains(id) { focus = nil }
        }
        .sheet(item: $selected) { item in
            MatchDetailSheet(matchId: item.id)
        }
    }

    // MARK: - App-szintű Tab-sorrend

    private var matchIDs: [String] {
        guard case .loaded(let feed) = model.state else { return [] }
        return filteredLeagues(feed.matches).flatMap { league in
            league.matches.sorted { ($0.time, $0.id) < ($1.time, $1.id) }.map(\.id)
        }
    }

    private var focusOrder: [AppFocus] {
        AppFocus.order(tab: tab, matchIDs: matchIDs)
    }

    private var focusedMatchID: String? {
        if case .match(let id) = focus { return id }
        return nil
    }

    private func handle(keyCode: UInt16, modifiers rawModifiers: NSEvent.ModifierFlags) -> Bool {
        // A nyíl- és Tab-billentyűk maguk is „function” jelzőt hordozhatnak.
        let modifiers = rawModifiers
            .intersection(.deviceIndependentFlagsMask)
            .subtracting([.function, .numericPad])
        // A naptár-popover nyitva saját ablakként viselkedik: ilyenkor nem nyúlunk a billentyűkhöz.
        guard !showCalendar else { return false }
        // ⌘/⌥ gyorsbillentyűk: nap-léptetés, mai nap, újratöltés.
        if modifiers.contains(.command) || modifiers.contains(.option) {
            switch keyCode {
            case KeyCode.left:
                model.goToPreviousDay()
                return true
            case KeyCode.right:
                model.goToNextDay()
                return true
            case KeyCode.today where modifiers.contains(.command):
                model.goToToday()
                return true
            case KeyCode.refresh where modifiers.contains(.command):
                Task { await model.load() }
                return true
            default:
                return false
            }
        }
        guard modifiers.isEmpty || modifiers == .shift else { return false }

        switch keyCode {
        case KeyCode.tab:
            let order = focusOrder
            guard !order.isEmpty else { return false }
            focus = focusedTarget(from: focus, step: modifiers.contains(.shift) ? -1 : 1, in: order)
            return true
        case KeyCode.down:
            return stepMatch(by: 1)
        case KeyCode.up:
            return stepMatch(by: -1)
        case KeyCode.returnKey, KeyCode.enter:
            guard let focus else { return false }
            activate(focus)
            return true
        case KeyCode.escape:
            guard focus != nil else { return false }
            focus = nil
            return true
        default:
            return false
        }
    }

    /// ↑ / ↓ csak a lista-állomáson belül lép (nem körbeérő, nem ugrik a fejlécbe).
    private func stepMatch(by delta: Int) -> Bool {
        let ids = matchIDs
        guard case .match(let current) = focus, let index = ids.firstIndex(of: current) else {
            return false
        }
        let next = index + delta
        if ids.indices.contains(next) { focus = .match(ids[next]) }
        return true
    }

    private func activate(_ target: AppFocus) {
        switch target {
        case .tab(let destination):
            tab = destination
        case .previousDay:
            model.goToPreviousDay()
        case .today:
            model.goToToday()
        case .nextDay:
            model.goToNextDay()
        case .calendar:
            showCalendar = true
        case .refresh:
            Task { await model.load() }
        case .filter(let candidate):
            filter = candidate
        case .featured:
            featuredOnly.toggle()
        case .match(let id):
            if let uuid = UUID(uuidString: id) {
                selected = SelectedMatch(id: uuid)
            }
        }
    }

    @ViewBuilder
    private func dayContent(model: DayModel, width: CGFloat) -> some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            HomeHeader(
                date: $model.selectedDate,
                count: loadedCount,
                refreshedAt: loadedRefreshedAt,
                appeared: appeared,
                focused: focus,
                showCalendar: $showCalendar,
                onPrevious: { model.goToPreviousDay() },
                onToday: { model.goToToday() },
                onNext: { model.goToNextDay() },
                onRefresh: { Task { await model.load() } }
            )

            FilterBar(filter: $filter, country: $country, division: $division,
                      featuredOnly: $featuredOnly, womensOnly: $womensOnly,
                      favoritesOnly: $favoritesOnly,
                      searchText: $searchText, focused: focus,
                      counts: filterCounts, countries: allCountries,
                      compact: width < 1_000)

            if case .loaded(let feed) = model.state, let gaps = feed.gaps, !gaps.isEmpty {
                DayGapsCard(gaps: gaps, active: filter) { filter = $0 }
            }

            content
        }
    }

    @ViewBuilder
    private var content: some View {
        switch model.state {
        case .idle, .loading:
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let feed):
            let leagues = filteredLeagues(feed.matches)
            if leagues.isEmpty, featuredOnly, !groups(feed.matches, featured: false).isEmpty {
                featuredEmptyHint
            } else {
                MatchListView(
                    leagues: leagues,
                    logos: model.logos,
                    focusedMatch: focusedMatchID,
                    onSelect: { id in
                        if let uuid = UUID(uuidString: id) {
                            selected = SelectedMatch(id: uuid)
                        }
                    },
                    favorites: favorites,
                    onToggleFavorite: toggleFavorite,
                    collapsed: collapsed,
                    onToggleCollapse: toggleCollapse
                )
            }
        case .empty:
            EmptyStateView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: { Task { await model.load() } })
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var loadedRefreshedAt: Date? {
        if case .loaded(let feed) = model.state { return feed.refreshedAt }
        return nil
    }

    private var loadedCount: Int? {
        if case .loaded(let feed) = model.state { return feed.matches.count }
        return nil
    }

    private var featuredEmptyHint: some View {
        VStack(spacing: 12) {
            Image(systemName: "star")
                .font(.largeTitle)
                .foregroundStyle(AppTheme.secondaryText)
            Text("A kiemelt körben nincs meccs ezen a napon.")
                .foregroundStyle(AppTheme.secondaryText)
            Button("Minden verseny mutatása") { featuredOnly = false }
                .buttonStyle(.borderedProminent)
                .tint(AppTheme.accent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func groups(_ matches: [DayMatch], featured: Bool) -> [LeagueGroup] {
        LeagueGroup.fromDay(matches).filter { league in
            (!featured || LeagueGroup.featured([league]).isEmpty == false) &&
            (country == nil || league.country == country) &&
            (division == nil || division!.matches(league)) &&
            (!womensOnly || league.isWomens) &&
            (!favoritesOnly || favorites.contains(league.id))
        }
    }

    private func baseGroups(_ matches: [DayMatch]) -> [LeagueGroup] {
        groups(matches, featured: featuredOnly)
    }

    /// Az élő meccsek száma a megjelenített napon; ez indítja a háttér-frissítést.
    /// Csak a mai napon és csak a 6 órás élő-ablakon belül számol: egy múltbeli
    /// nap elavult `live` státusza így nem polloz örökre.
    private var liveMatchCount: Int {
        guard case .loaded(let feed) = model.state else { return 0 }
        guard Calendar.current.isDateInToday(model.selectedDate) else { return 0 }
        let now = Date()
        return feed.matches.filter { MatchFilter.live.matches($0, now: now) }.count
    }

    /// A frissítő kör azonosítója: napváltás vagy az élő meccsek számának
    /// változása indítja újra (a `.task(id:)` a régit megszakítja).
    private var autoRefreshKey: String {
        "\(model.selectedDate.timeIntervalSince1970)-\(liveMatchCount)"
    }

    private var allCountries: [String] {
        guard case .loaded(let feed) = model.state else { return [] }
        let matches = feed.matches
        return Array(Set(LeagueGroup.fromDay(matches).map(\.country)))
            .sorted { $0.localizedStandardCompare($1) == .orderedAscending }
    }

    private var filterCounts: [MatchFilter: Int] {
        guard case .loaded(let feed) = model.state else { return [:] }
        let matches = feed.matches
        let all = baseGroups(matches).flatMap { $0.matches }
        var result: [MatchFilter: Int] = [:]
        let now = Date()
        for candidate in MatchFilter.allCases {
            result[candidate] = all.filter { candidate.matches($0, now: now) }.count
        }
        return result
    }

    private func filteredLeagues(_ matches: [DayMatch]) -> [LeagueGroup] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        return baseGroups(matches).compactMap { league -> LeagueGroup? in
            guard let filtered = league.filtering(filter) else { return nil }
            guard !query.isEmpty else { return filtered }
            let needle = query.folding(options: [.diacriticInsensitive, .caseInsensitive],
                                       locale: Locale(identifier: "hu_HU"))
            func hit(_ text: String) -> Bool {
                text.folding(options: [.diacriticInsensitive, .caseInsensitive],
                             locale: Locale(identifier: "hu_HU"))
                    .contains(needle)
            }
            if hit(filtered.name) || hit(filtered.country) { return filtered }
            let kept = filtered.matches.filter { hit($0.home) || hit($0.away) }
            guard !kept.isEmpty else { return nil }
            return LeagueGroup(id: filtered.id, name: filtered.name, country: filtered.country,
                               countryCode: filtered.countryCode, flag: filtered.flag,
                               logo: filtered.logo, tier: filtered.tier,
                               priority: filtered.priority, matches: kept)
        }
    }

    /// A szekciófejléc csillagának művelete: kedvenc be/ki, azonnal mentve.
    private func toggleCollapse(_ key: String) {
        collapsed = LeagueCollapse().toggle(key)
    }

    private func toggleFavorite(_ key: String) {
        favorites = LeagueFavorites().toggle(key)
        if favoritesOnly && favorites.isEmpty {
            favoritesOnly = false
        }
    }
}

private struct TopNavigationBar: View {
    let selected: AppTab
    let focused: AppFocus?
    /// Keskeny ablakban az óra helyet ad a füleknek.
    let compact: Bool
    let onSelect: (AppTab) -> Void
    private static let clock: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "HH:mm:ss"
        return formatter
    }()

    var body: some View {
        HStack(spacing: 16) {
            HStack(spacing: 8) {
                Circle().fill(AppTheme.accent).frame(width: 9, height: 9)
                Text("FOOTBALL APP")
                    .font(.system(size: 12, weight: .semibold))
                    .tracking(3)
                    .foregroundStyle(AppTheme.primaryText)
            }

            Spacer()

            HStack(spacing: 8) {
                pill("Ma", tab: .day, identifier: "day", active: selected == .day) { onSelect(.day) }
                pill("Versenyek", tab: .competitions, identifier: "competitions", active: selected == .competitions) { onSelect(.competitions) }
                pill("Csapatok", tab: .teams, identifier: "teams", active: selected == .teams) { onSelect(.teams) }
                pill("Üzemeltetés", tab: .ops, identifier: "ops", active: selected == .ops) { onSelect(.ops) }
            }

            Spacer()

            if !compact {
                HStack(spacing: 14) {
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        Text(Self.clock.string(from: context.date))
                            .font(.system(size: 12, weight: .medium).monospacedDigit())
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                }
            }
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
    }

    @ViewBuilder
    private func pill(_ title: String, active: Bool = false) -> some View {
        pill(title, active: active, action: nil)
    }

    private func pill(
        _ title: String,
        tab: AppTab? = nil,
        identifier: String? = nil,
        active: Bool,
        action: (() -> Void)?
    ) -> some View {
        let isFocused = tab.map { focused == .tab($0) } ?? false
        return Button {
            action?()
        } label: {
            Text(title)
                .font(.system(size: 12.5, weight: .medium))
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
                .foregroundStyle(active ? AppTheme.primaryText : AppTheme.secondaryText.opacity(0.45))
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(active ? AppTheme.surface : Color.clear, in: Capsule())
        }
        .buttonStyle(.plain)
        .disabled(action == nil)
        .help(title)
        .accessibilityIdentifier(identifier.map { "nav.tab.\($0)" } ?? "nav.tab")
        .focusAccessibility(isFocused)
        .capsuleFocusRing(isFocused)
    }
}

private struct HomeHeader: View {
    @Binding var date: Date
    let count: Int?
    let refreshedAt: Date?
    let appeared: Bool
    let focused: AppFocus?
    @Binding var showCalendar: Bool
    let onPrevious: () -> Void
    let onToday: () -> Void
    let onNext: () -> Void
    let onRefresh: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let bigDate: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "MMMM d."
        return formatter
    }()
    private static let year: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy"
        return formatter
    }()
    private static let weekday: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "EEEE"
        return formatter
    }()

    private var greeting: String {
        switch Calendar.current.component(.hour, from: Date()) {
        case 5..<10: return "Jó reggelt."
        case 10..<18: return "Jó napot."
        case 18..<22: return "Jó estét."
        default: return "Jó éjszakát."
        }
    }

    private var relativeLabel: String? {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) { return "MA" }
        if calendar.isDateInYesterday(date) { return "TEGNAP" }
        if calendar.isDateInTomorrow(date) { return "HOLNAP" }
        return nil
    }

    private static let refreshTime: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()
    private static let refreshDay: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "MMM d. HH:mm"
        return formatter
    }()

    private var refreshLabel: String? {
        guard let refreshedAt else { return nil }
        let formatter = Calendar.current.isDateInToday(refreshedAt)
            ? Self.refreshTime : Self.refreshDay
        return "frissítve \(formatter.string(from: refreshedAt))"
    }

    private var metaText: String {
        var parts = [Self.year.string(from: date), Self.weekday.string(from: date)]
        if let count { parts.append("\(count) meccs") }
        if let refreshLabel { parts.append(refreshLabel) }
        return parts.joined(separator: " · ")
    }

    private func step(_ delay: Double) -> Animation? {
        reduceMotion ? nil : .easeOut(duration: 0.45).delay(delay)
    }

    var body: some View {
        HStack(alignment: .bottom, spacing: 20) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 10) {
                    Text(greeting)
                        .font(.appText(15))
                        .foregroundStyle(AppTheme.secondaryText)
                    if let relativeLabel {
                        Text(relativeLabel)
                            .font(.system(size: 10.5, weight: .bold))
                            .tracking(1.5)
                            .foregroundStyle(AppTheme.accent)
                            .padding(.horizontal, 7)
                            .padding(.vertical, 2)
                            .background(AppTheme.accent.opacity(0.15), in: Capsule())
                    }
                }
                .opacity(appeared ? 1 : 0)
                .offset(y: appeared ? 0 : 8)
                .animation(step(0.04), value: appeared)

                Text(Self.bigDate.string(from: date))
                    .font(.scoreboard(56))
                    .tracking(-1.5)
                    .lineLimit(1)
                    .minimumScaleFactor(0.55)
                    .foregroundStyle(AppTheme.primaryText)
                    .opacity(appeared ? 1 : 0)
                    .offset(y: appeared ? 0 : 12)
                    .animation(step(0.1), value: appeared)

                Text(metaText)
                    .font(.appText(12.5))
                    .foregroundStyle(AppTheme.secondaryText)
                    .opacity(appeared ? 1 : 0)
                    .offset(y: appeared ? 0 : 12)
                    .animation(step(0.16), value: appeared)
            }

            Spacer()

            HStack(spacing: 8) {
                iconButton("chevron.left", help: "Előző nap",
                           focus: .previousDay, action: onPrevious)
                    .keyboardShortcut(.leftArrow, modifiers: .command)
                Button(action: onToday) {
                    Text("Ma")
                        .font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(AppTheme.primaryText)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 7)
                        .background(AppTheme.surface, in: Capsule())
                        .overlay(Capsule().strokeBorder(AppTheme.hairline))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Ma")
                .accessibilityIdentifier("nav.today")
                .focusAccessibility(focused == .today)
                .capsuleFocusRing(focused == .today)
                .keyboardShortcut("t", modifiers: .command)
                iconButton("chevron.right", help: "Következő nap",
                           focus: .nextDay, action: onNext)
                    .keyboardShortcut(.rightArrow, modifiers: .command)

                Rectangle().fill(AppTheme.hairline).frame(width: 1, height: 18).padding(.horizontal, 2)

                Button {
                    showCalendar = true
                } label: {
                    Image(systemName: "calendar")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(AppTheme.secondaryText)
                        .frame(width: 30, height: 30)
                        .background(AppTheme.surface, in: Circle())
                        .overlay(Circle().strokeBorder(AppTheme.hairline))
                }
                .buttonStyle(.plain)
                .help("Naptár")
                .accessibilityLabel("Naptár")
                .accessibilityIdentifier("nav.calendar")
                .focusAccessibility(focused == .calendar)
                .circleFocusRing(focused == .calendar)
                .popover(isPresented: $showCalendar, arrowEdge: .bottom) {
                    MonthCalendar(date: $date, onSelect: { showCalendar = false })
                }

                iconButton("arrow.clockwise", help: "Frissítés",
                           focus: .refresh, action: onRefresh)
                    .keyboardShortcut("r", modifiers: .command)
            }
            .opacity(appeared ? 1 : 0)
            .animation(step(0.16), value: appeared)
        }
        .padding(.horizontal, 48)
        .padding(.top, 22)
        .padding(.bottom, 16)
    }

    private func iconButton(
        _ systemName: String,
        help: String,
        focus target: AppFocus,
        action: @escaping () -> Void
    ) -> some View {
        let isFocused = focused == target
        return Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 30, height: 30)
                .background(AppTheme.surface, in: Circle())
                .overlay(Circle().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityLabel(help)
        .accessibilityIdentifier("nav.\(systemName)")
        .focusAccessibility(isFocused)
        .circleFocusRing(isFocused)
    }
}

/// Osztály-szűrő: a látható meccsek közt előforduló szintek, a kupa és a
/// mélyebb (harmadik osztály alatti / ismeretlen szintű) bajnokságok.
enum DivisionFilter: Hashable {
    case tier(Int)
    case deeper
    case cup

    var title: String {
        switch self {
        case .tier(let level): return "\(level). osztály"
        case .deeper: return "Mélyebb ligák"
        case .cup: return "Kupa"
        }
    }

    func matches(_ league: LeagueGroup) -> Bool {
        switch self {
        case .cup:
            return league.isCup
        case .deeper:
            guard !league.isCup else { return false }
            let tier = league.tier ?? 0
            return tier == 0 || tier >= 4
        case .tier(let level):
            guard !league.isCup else { return false }
            return league.tier == level
        }
    }

    /// A menüben felkínált szintek: 1–4, majd a mélyebb/egyéb és a kupa.
    static let menuOptions: [DivisionFilter] = [
        .tier(1), .tier(2), .tier(3), .tier(4), .deeper, .cup,
    ]
}

private struct FilterBar: View {
    @Binding var filter: MatchFilter
    @Binding var country: String?
    @Binding var division: DivisionFilter?
    @Binding var featuredOnly: Bool
    @Binding var womensOnly: Bool
    @Binding var favoritesOnly: Bool
    @Binding var searchText: String
    let focused: AppFocus?
    let counts: [MatchFilter: Int]
    let countries: [String]
    /// Keskeny ablakban két sorra tördelünk: szűrők, majd a legördülők.
    let compact: Bool

    var body: some View {
        // Két sor: felül a státusz- és hiány-szűrők (szükség esetén
        // vízszintesen görgethetők), alatta a kereső és a legördülők. Egy
        // sorban a hat szűrő + kereső + három menü már 1512 pt-on sem fért el.
        VStack(alignment: .leading, spacing: 8) {
            ScrollView(.horizontal, showsIndicators: false) {
                chipRow
            }
            HStack(spacing: 8) {
                searchField
                featuredButton
                womensButton
                favoritesButton
                Spacer(minLength: 8)
                menus
            }
        }
        .padding(.horizontal, 48)
        .padding(.bottom, 6)
    }

    private var chipRow: some View {
        HStack(spacing: 8) {
            ForEach(MatchFilter.allCases) { candidate in
                FilterChip(
                    filter: candidate,
                    count: counts[candidate] ?? 0,
                    isActive: filter == candidate,
                    isFocused: focused == .filter(candidate)
                ) {
                    filter = candidate
                }
            }
        }
        .fixedSize(horizontal: true, vertical: false)
    }

    private var searchField: some View {
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
            TextField("Csapat, liga, ország…", text: $searchText)
                .textFieldStyle(.plain)
                .font(.system(size: 12.5))
                .frame(width: compact ? 150 : 190)
            if !searchText.isEmpty {
                Button {
                    searchText = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 11))
                        .foregroundStyle(AppTheme.secondaryText)
                }
                .buttonStyle(.plain)
                .help("Keresés törlése")
                .accessibilityIdentifier("search.clear")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(AppTheme.surface, in: Capsule())
        .overlay(Capsule().strokeBorder(AppTheme.hairline))
        .accessibilityIdentifier("day.search")
    }

    private var featuredButton: some View {
        Button {
            featuredOnly.toggle()
        } label: {
            HStack(spacing: 6) {
                Image(systemName: featuredOnly ? "star.fill" : "star")
                    .font(.system(size: 10, weight: .semibold))
                Text(featuredOnly ? "Kiemelt kör" : "Minden verseny")
                    .font(.system(size: 12.5, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(featuredOnly ? AppTheme.accent : AppTheme.secondaryText)
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(
                (featuredOnly ? AppTheme.accent.opacity(0.12) : AppTheme.surface),
                in: Capsule()
            )
            .overlay(Capsule().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(featuredOnly
              ? "A kiemelt kör látszik (top-5 európai liga + NB I). Kattints a teljes naphoz."
              : "Minden beolvasott verseny látszik. Kattints a kiemelt körhöz.")
        .accessibilityIdentifier("filter.featured")
        .focusAccessibility(focused == .featured)
        .capsuleFocusRing(focused == .featured)
    }

    private var womensButton: some View {
        Button {
            womensOnly.toggle()
        } label: {
            HStack(spacing: 6) {
                Image(systemName: womensOnly ? "person.2.fill" : "person.2")
                    .font(.system(size: 10, weight: .semibold))
                Text("Női")
                    .font(.system(size: 12.5, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(womensOnly ? AppTheme.accent : AppTheme.secondaryText)
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(
                (womensOnly ? AppTheme.accent.opacity(0.12) : AppTheme.surface),
                in: Capsule()
            )
            .overlay(Capsule().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(womensOnly
              ? "Csak a női bajnokságok meccsei látszanak. Kattints a teljes naphoz."
              : "Csak a női bajnokságok meccseinek mutatása.")
        .accessibilityIdentifier("filter.womens")
    }

    private var favoritesButton: some View {
        Button {
            favoritesOnly.toggle()
        } label: {
            HStack(spacing: 6) {
                Image(systemName: favoritesOnly ? "star.fill" : "star")
                    .font(.system(size: 10, weight: .semibold))
                Text("Kedvencek")
                    .font(.system(size: 12.5, weight: .medium))
                    .lineLimit(1)
            }
            .foregroundStyle(favoritesOnly ? AppTheme.accent : AppTheme.secondaryText)
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(
                (favoritesOnly ? AppTheme.accent.opacity(0.12) : AppTheme.surface),
                in: Capsule()
            )
            .overlay(Capsule().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(favoritesOnly
              ? "Csak a kedvenc versenyek meccsei látszanak. Kattints a teljes naphoz."
              : "Csak a kedvenc versenyek meccseinek mutatása.")
        .accessibilityIdentifier("filter.favorites")
    }

    private var menus: some View {
        HStack(spacing: 8) {
            Menu {
                Button("Minden ország") { country = nil }
                Divider()
                ForEach(countries, id: \.self) { name in
                    Button(name) { country = name }
                }
            } label: {
                menuLabel(country ?? "Minden ország")
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .accessibilityIdentifier("filter.country")

            Menu {
                Button("Minden osztály") { division = nil }
                Divider()
                ForEach(DivisionFilter.menuOptions, id: \.self) { option in
                    Button(option.title) { division = option }
                }
            } label: {
                menuLabel(division?.title ?? "Minden osztály")
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .accessibilityIdentifier("filter.division")
        }
    }

    private func menuLabel(_ text: String) -> some View {
        HStack(spacing: 6) {
            Text(text)
                .font(.system(size: 12.5, weight: .medium))
                .foregroundStyle(AppTheme.secondaryText)
                .lineLimit(1)
            Image(systemName: "chevron.down")
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(AppTheme.surface, in: Capsule())
        .overlay(Capsule().strokeBorder(AppTheme.hairline))
    }
}

private struct FilterChip: View {
    let filter: MatchFilter
    let count: Int
    let isActive: Bool
    let isFocused: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(filter.title)
                    .font(.system(size: 12.5, weight: .medium))
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
                    .foregroundStyle(isActive ? AppTheme.primaryText : AppTheme.secondaryText)
                Text("\(count)")
                    .font(.system(size: 11, weight: .semibold).monospacedDigit())
                    .foregroundStyle(isActive ? AppTheme.background : AppTheme.secondaryText)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(isActive ? AppTheme.accent : AppTheme.surface, in: Capsule())
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(isActive ? AppTheme.surfaceElevated : Color.clear, in: Capsule())
            .overlay(Capsule().strokeBorder(isActive ? AppTheme.hairline : .clear))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("filter.\(filter.rawValue)")
        .focusAccessibility(isFocused)
        .capsuleFocusRing(isFocused)
    }
}

/// Nagy, jól olvasható havi naptár (magyar, hétfő-kezdés), a dark témához.
private struct MonthCalendar: View {
    @Binding var date: Date
    let onSelect: () -> Void

    @State private var month: Date

    private static let calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.firstWeekday = 2
        calendar.locale = Locale(identifier: "hu_HU")
        return calendar
    }()
    private static let monthTitle: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy. MMMM"
        return formatter
    }()
    private static let weekdaySymbols = ["H", "K", "Sze", "Cs", "P", "Szo", "V"]

    init(date: Binding<Date>, onSelect: @escaping () -> Void) {
        self._date = date
        self.onSelect = onSelect
        let calendar = MonthCalendar.calendar
        let components = calendar.dateComponents([.year, .month], from: date.wrappedValue)
        self._month = State(initialValue: calendar.date(from: components) ?? date.wrappedValue)
    }

    private var calendar: Calendar { Self.calendar }

    var body: some View {
        VStack(spacing: 14) {
            HStack(spacing: 8) {
                monthButton("chevron.left") { shiftMonth(-1) }
                Text(Self.monthTitle.string(from: month))
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                    .frame(maxWidth: .infinity)
                monthButton("chevron.right") { shiftMonth(1) }
            }

            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 4), count: 7), spacing: 4) {
                ForEach(Self.weekdaySymbols, id: \.self) { symbol in
                    Text(symbol)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(AppTheme.secondaryText)
                        .frame(maxWidth: .infinity)
                }
                ForEach(Array(cells.enumerated()), id: \.offset) { _, day in
                    if let day {
                        dayCell(day)
                    } else {
                        Color.clear.frame(height: 36)
                    }
                }
            }

            Button {
                date = Date()
                onSelect()
            } label: {
                Text("Ma")
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(AppTheme.primaryText)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                    .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(AppTheme.hairline))
            }
            .buttonStyle(.plain)
        }
        .padding(16)
        .frame(width: 320)
    }

    private func dayCell(_ day: Int) -> some View {
        let cellDate = dateFor(day)
        let isSelected = calendar.isDate(cellDate, inSameDayAs: date)
        let isToday = calendar.isDateInToday(cellDate)
        return Button {
            date = cellDate
            onSelect()
        } label: {
            Text("\(day)")
                .font(.system(size: 14, weight: isSelected ? .bold : .regular).monospacedDigit())
                .foregroundStyle(isSelected ? AppTheme.background : AppTheme.primaryText)
                .frame(maxWidth: .infinity, minHeight: 36)
                .background(
                    isSelected ? AppTheme.accent : (isToday ? AppTheme.surfaceElevated : Color.clear),
                    in: RoundedRectangle(cornerRadius: 8, style: .continuous)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(isToday && !isSelected ? AppTheme.accent.opacity(0.6) : Color.clear)
                )
        }
        .buttonStyle(.plain)
    }

    private var cells: [Int?] {
        guard let range = calendar.range(of: .day, in: .month, for: month),
              let first = calendar.date(from: calendar.dateComponents([.year, .month], from: month)) else {
            return []
        }
        let weekday = calendar.component(.weekday, from: first)
        let offset = (weekday + 5) % 7
        var result: [Int?] = Array(repeating: nil, count: offset)
        for day in range { result.append(day) }
        return result
    }

    private func dateFor(_ day: Int) -> Date {
        calendar.date(
            from: DateComponents(
                year: calendar.component(.year, from: month),
                month: calendar.component(.month, from: month),
                day: day
            )
        ) ?? month
    }

    private func shiftMonth(_ delta: Int) {
        month = calendar.date(byAdding: .month, value: delta, to: month) ?? month
    }

    private func monthButton(_ systemName: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 28, height: 28)
                .background(AppTheme.surface, in: Circle())
                .overlay(Circle().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
    }
}

/// A nap hiány-mutatói: mi hiányzik ahhoz, hogy a nap teljes legyen.
private struct DayGapsCard: View {
    let gaps: DayGaps
    let active: MatchFilter
    let onSelect: (MatchFilter) -> Void

    /// A hiány-chipek szűrője. Minden chip kattintható, és a saját hiányát
    /// aktiválja: a „lejárt, nyitott" az `openPast`, a valódi „eredmény nélkül"
    /// a `missingResult` szűrőre mutat.
    private static let filterFor: [String: MatchFilter] = [
        "kickoff": .missingKickoff,
        "open": .openPast,
        "result": .missingResult,
        "teamlogo": .missingLogo,
        "complogo": .missingCompetitionLogo,
        "odds": .missingOdds,
        "withheld": .withheld,
    ]

    private struct Item: Identifiable {
        let id: String
        let label: String
        let count: Int
    }

    private var items: [Item] {
        var list: [Item] = []
        func add(_ key: String, _ label: String, _ count: Int) {
            if count > 0 { list.append(Item(id: key, label: label, count: count)) }
        }
        add("kickoff", "kezdésidő nélkül", gaps.missingKickoff)
        add("open", "lejárt, nyitott", gaps.openPast)
        add("result", "eredmény nélkül", gaps.missingResult)
        add("teamlogo", "csapatlogó nélkül", gaps.missingTeamLogo)
        add("complogo", "ligalogó nélkül", gaps.missingCompetitionLogo)
        add("odds", "odds nélkül", gaps.missingOdds)
        add("withheld", "visszatartva", gaps.withheld)
        return list
    }

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(AppTheme.accent)
            Text("Hiányzó adatok")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(items) { item in
                        let filter = Self.filterFor[item.id]
                        let isActive = filter.map { $0 == active } ?? false
                        Button {
                            if let filter {
                                onSelect(active == filter ? .all : filter)
                            }
                        } label: {
                            HStack(spacing: 4) {
                                Text("\(item.count)")
                                    .font(.system(size: 11, weight: .bold, design: .rounded))
                                    .foregroundStyle(isActive ? Color.black : AppTheme.primaryText)
                                Text(item.label)
                                    .font(.system(size: 11))
                                    .foregroundStyle(isActive ? Color.black.opacity(0.85) : AppTheme.secondaryText)
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(isActive ? AppTheme.accent : AppTheme.surface, in: Capsule())
                        }
                        .buttonStyle(.plain)
                        .disabled(filter == nil)
                        .accessibilityLabel("\(item.label): \(item.count) meccs")
                        .accessibilityHint(filter == nil
                            ? "Ehhez a hiányhoz nincs szűrő."
                            : "Szűrés erre: \(item.label)")
                        .accessibilityIdentifier("day.gaps.\(item.id)")
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 24)
        .padding(.bottom, 8)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("day.gaps")
        .accessibilityLabel("Hiányzó adatok: " + items.map { "\($0.count) \($0.label)" }.joined(separator: ", "))
    }
}
