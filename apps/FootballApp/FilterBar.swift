import SwiftUI
import FootballAppCore

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
            let tier = league.tier
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

struct FilterBar: View {
    @Binding var filter: MatchFilter
    @Binding var country: String?
    @Binding var division: DivisionFilter?
    @Binding var featuredOnly: Bool
    @Binding var womensOnly: Bool
    @Binding var favoritesOnly: Bool
    @Binding var searchText: String
    /// A napi lista ligáinak rendezési módja (S122).
    @Binding var sortMode: LeagueGroup.SortMode
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
            ForEach(MatchFilter.homeFilters) { candidate in
                FilterChip(
                    filter: candidate,
                    count: counts[candidate] ?? 0,
                    showsCount: candidate != .missingLogo && candidate != .missingCompetitionLogo,
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

            Menu {
                ForEach(LeagueGroup.SortMode.allCases, id: \.self) { mode in
                    Button(mode.title) { sortMode = mode }
                }
            } label: {
                menuLabel(sortMode.title)
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .accessibilityIdentifier("filter.sort")
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
    let showsCount: Bool
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
                if showsCount {
                    Text("\(count)")
                        .font(.system(size: 11, weight: .semibold).monospacedDigit())
                        .foregroundStyle(isActive ? AppTheme.background : AppTheme.secondaryText)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .background(isActive ? AppTheme.accent : AppTheme.surface, in: Capsule())
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(isActive ? AppTheme.surfaceElevated : Color.clear, in: Capsule())
            .overlay(Capsule().strokeBorder(isActive ? AppTheme.hairline : .clear))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(showsCount ? "\(filter.title): \(count)" : filter.title)
        .accessibilityIdentifier("filter.\(filter.rawValue)")
        .focusAccessibility(isFocused)
        .capsuleFocusRing(isFocused)
    }
}
