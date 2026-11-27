import SwiftUI
import AppKit
import FootballAppCore

/// A főoldal: felső navigáció → változó HEADER → szűrő → liga szerint csoportosított
/// napi lista, a footballd API valós válaszából.
struct SelectedMatch: Identifiable {
    let id: UUID
}

/// UI-teszt belépési pont: indításkor közvetlenül megnyitható egy játékoslap
/// anélkül, hogy a keresési sheet interactiójára kellene hagyatkozni.
struct SelectedPlayer: Identifiable {
    let id: UUID
}

/// Belső entitáslink célja: a keresés vagy a meccsrészletező csapat/verseny
/// hivatkozása. A név a hivatkozásról jön, a részletadatot a sheet tölti be.
struct EntityTarget: Identifiable {
    let id: UUID
    let name: String
    /// `"team"` vagy `"competition"` — a nested sheet melyik adatlapot nyissa.
    let kind: String
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
    /// A napi lista ligáinak rendezési módja (S122).
    @State private var sortMode: LeagueGroup.SortMode = .featured
    /// Összecsukott verseny-szekciók (UserDefaults-ból, hogy megmaradjon).
    @State private var collapsed: Set<String> = LeagueCollapse().load()
    @State private var favorites: Set<String> = LeagueFavorites().load()
    @State private var tab: AppTab = .day
    @State private var featuredOnly = false
    @State private var selected: SelectedMatch? = {
        // Valódi adatos ellenőrzési belépési pont: a futó API-ból olvas,
        // nem fixture-szerverről. Az UI-teszt a saját fixture-ét használja.
        guard let value = ProcessInfo.processInfo.environment["FOOTBALL_UI_MATCH_ID"],
              let id = UUID(uuidString: value)
        else { return nil }
        return SelectedMatch(id: id)
    }()
    @State private var autoPlayer: SelectedPlayer? = {
        guard let value = ProcessInfo.processInfo.environment["FOOTBALL_UI_PLAYER_ID"],
              let id = UUID(uuidString: value)
        else { return nil }
        return SelectedPlayer(id: id)
    }()
    @State private var focus: AppFocus?
    @State private var showCalendar = false
    /// ⌘F globális keresés (csapat / verseny / meccs).
    @State private var showSearch = false
    @State private var searchModel = SearchModel()
    /// A fejléc élő időjárása; a helyzet a szerkeszthető helyi konfigból jön,
    /// így a macOS soha nem kér helymeghatározási engedélyt. Hálózat híján
    /// napszak-alapú hangulatra esik vissza.
    @State private var weather = WeatherModel(location: SavedWeatherLocation.load())

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    /// Az utolsó ismert életciklus-állapot — az aktíválódás felismeréséhez.
    @State private var previousPhase: AppLifecyclePhase = .inactive
    @State private var appeared = false

    var body: some View {
        @Bindable var model = model
        GeometryReader { geometry in
            VStack(spacing: 0) {
                TopNavigationBar(selected: tab, focused: focus,
                                 compact: geometry.size.width < 760,
                                 onSearch: { showSearch = true }) { tab = $0 }

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
        .sheet(item: $autoPlayer) { selected in
            PlayerView(id: selected.id)
        }
        .sheet(isPresented: $showSearch) {
            SearchView(model: searchModel) { result in
                switch result.kind {
                case "match":
                    tab = .day
                    selected = SelectedMatch(id: result.id)
                default:
                    // A csapat/verseny/játékos adatlapját a SearchView saját,
                    // nested sheetje nyitja — l. ott a megjegyzést.
                    break
                }
            }
        }
        .onAppear {
            withAnimation(reduceMotion ? nil : .easeOut(duration: 0.4)) { appeared = true }
        }
        // A háttérben szuszpendált időzített kör helyett azonnal frissít, amikor
        // a felhasználó újra az app elé kerül — kézi Frissítés nélkül.
        .onChange(of: scenePhase) { _, newPhase in
            let current = AppLifecyclePhase(newPhase)
            if AutoRefreshPolicy.shouldRefreshOnActivation(from: previousPhase, to: current) {
                Task { await model.refresh() }
            }
            previousPhase = current
        }
        .task(id: model.selectedDate) { await model.load() }
        // Időjárás: azonnal, majd 15 percenként. A szolgáltatás rövid
        // időkorláttal és graceful fallbackgel dolgozik, ezért nem blokkol.
        .task {
            // A helyzet a `weather-location.json`-ból jön (lásd
            // SavedWeatherLocation): nem hívunk CoreLocation-t, így a rendszer
            // nem dob fel engedélyező ablakot. A szolgáltatás rövid időkorláttal
            // és graceful fallbackgel dolgozik, ezért nem blokkol.
            await weather.load()
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(900))
                if Task.isCancelled { return }
                await weather.load()
            }
        }
        // A napi lista háttérben is frissül: élő meccseknél percenként,
        // egyébként 5 percenként. A kör nem villantja fel a betöltés-jelzőt,
        // és hiba esetén megtartja a listát. Régen csak élő meccseknél indult,
        // ezért a tegnapi (vagy élő nélküli mai) nap elavult maradt.
        .task(id: autoRefreshKey) {
            let seconds = AutoRefreshPolicy.intervalSeconds(liveMatchCount: liveMatchCount)
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(seconds))
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
            case KeyCode.search where modifiers.contains(.command):
                showSearch = true
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
                liveCount: loadedLiveCount,
                oddsCount: loadedOddsCount,
                compact: width < 900,
                refreshedAt: loadedRefreshedAt,
                nextRefreshAt: loadedNextRefreshAt,
                refreshFailed: model.lastRefreshFailed,
                appeared: appeared,
                focused: focus,
                weather: weather,
                showCalendar: $showCalendar,
                onPrevious: { model.goToPreviousDay() },
                onToday: { model.goToToday() },
                onNext: { model.goToNextDay() },
                onRefresh: { Task { await model.load() } }
            )

            FilterBar(filter: $filter, country: $country, division: $division,
                      featuredOnly: $featuredOnly, womensOnly: $womensOnly,
                      favoritesOnly: $favoritesOnly,
                      searchText: $searchText, sortMode: $sortMode, focused: focus,
                      counts: filterCounts, countries: allCountries,
                      compact: width < 1_000)

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

    /// A következő tényleges adatfrissítés (research napi slot) a backendtől.
    private var loadedNextRefreshAt: Date? {
        if case .loaded(let feed) = model.state { return feed.nextRefreshAt }
        return nil
    }

    private var loadedCount: Int? {
        if case .loaded(let feed) = model.state { return feed.matches.count }
        return nil
    }

    private var loadedLiveCount: Int {
        guard case .loaded(let feed) = model.state else { return 0 }
        return feed.matches.filter { MatchFilter.live.matches($0) }.count
    }

    private var loadedOddsCount: Int {
        guard case .loaded(let feed) = model.state else { return 0 }
        return feed.matches.filter { $0.odds != nil }.count
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
        let filtered = LeagueGroup.fromDay(matches).filter { league in
            (!featured || LeagueGroup.featured([league]).isEmpty == false) &&
            (country == nil || league.country == country) &&
            (division == nil || division!.matches(league)) &&
            (!womensOnly || league.isWomens) &&
            (!favoritesOnly || favorites.contains(league.id))
        }
        return LeagueGroup.ordered(filtered, mode: sortMode)
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

private extension AppLifecyclePhase {
    init(_ phase: ScenePhase) {
        switch phase {
        case .active: self = .active
        case .inactive: self = .inactive
        case .background: self = .background
        @unknown default: self = .inactive
        }
    }
}

private struct TopNavigationBar: View {
    let selected: AppTab
    let focused: AppFocus?
    /// Keskeny ablakban az óra helyet ad a füleknek.
    let compact: Bool
    let onSearch: () -> Void
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
                pill("Főoldal", tab: .day, identifier: "day", active: selected == .day) { onSelect(.day) }
                pill("Versenyek", tab: .competitions, identifier: "competitions", active: selected == .competitions) { onSelect(.competitions) }
                pill("Csapatok", tab: .teams, identifier: "teams", active: selected == .teams) { onSelect(.teams) }
                pill("Üzemeltetés", tab: .ops, identifier: "ops", active: selected == .ops) { onSelect(.ops) }
            }

            Button {
                onSearch()
            } label: {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .buttonStyle(.plain)
            .help("Globális keresés (⌘F)")
            .accessibilityLabel("Globális keresés")
            .accessibilityIdentifier("nav.search")
            .keyboardShortcut("f", modifiers: .command)

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
