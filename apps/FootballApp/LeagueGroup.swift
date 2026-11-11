import Foundation
import FootballAppCore

enum CountryDisplay {
    private static let table: [String: (flag: String, name: String)] = [
        "ENG": ("🏴󠁧󠁢󠁥󠁮󠁧󠁿", "Anglia"),
        "ESP": ("🇪🇸", "Spanyolország"),
        "ITA": ("🇮🇹", "Olaszország"),
        "GER": ("🇩🇪", "Németország"),
        "FRA": ("🇫🇷", "Franciaország"),
        "HUN": ("🇭🇺", "Magyarország"),
    ]

    static func display(code: String?, name: String?) -> (flag: String, name: String) {
        if let code, let known = table[code] { return known }
        return ("🏳️", name ?? code ?? "Ismeretlen")
    }
}

struct LeagueGroup: Identifiable {
    let id: String
    let name: String
    let country: String
    let flag: String
    let tier: Int
    let priority: Int
    let matches: [MatchCardModel]

    var divisionLabel: String {
        switch tier {
        case 1: return "1. osztály"
        case 2: return "2. osztály"
        default: return "Kupa"
        }
    }

    func filtering(_ filter: MatchFilter) -> LeagueGroup? {
        let kept = matches.filter { filter.matches(status: $0.status) }
        guard !kept.isEmpty else { return nil }
        return LeagueGroup(id: id, name: name, country: country, flag: flag,
                           tier: tier, priority: priority, matches: kept)
    }

    static func fromDay(_ matches: [DayMatch]) -> [LeagueGroup] {
        let grouped = Dictionary(grouping: matches) { $0.competitionId?.uuidString ?? "unknown" }
        return grouped.map { key, matches in
            let first = matches[0]
            let display = CountryDisplay.display(code: first.countryCode, name: first.countryName)
            return LeagueGroup(
                id: key,
                name: first.competitionName ?? "Verseny",
                country: display.name,
                flag: display.flag,
                tier: first.tier ?? 1,
                priority: first.priority ?? 100,
                matches: matches.map(MatchCardModel.from)
            )
        }
        .sorted {
            $0.priority != $1.priority ? $0.priority < $1.priority : $0.country < $1.country
        }
    }

    static func fromSample(_ leagues: [SampleLeague]) -> [LeagueGroup] {
        leagues.map { league in
            LeagueGroup(
                id: league.id, name: league.name, country: league.country, flag: league.flag,
                tier: league.tier, priority: league.priority,
                matches: league.matches.map(MatchCardModel.from)
            )
        }
    }
}