import Foundation
import FootballAppCore

enum CountryDisplay {
    private static let table: [String: (flag: String, name: String)] = [
        "GB": ("🏴󠁧󠁢󠁥󠁮󠁧󠁿", "Anglia"),
        "ES": ("🇪🇸", "Spanyolország"),
        "IT": ("🇮🇹", "Olaszország"),
        "DE": ("🇩🇪", "Németország"),
        "FR": ("🇫🇷", "Franciaország"),
        "HU": ("🇭🇺", "Magyarország"),
        // S9/S11: a Soccerway/nemzeti csapatok ISO kódjai (magyar név).
        "AG": ("🇦🇬", "Antigua és Barbuda"),
        "AR": ("🇦🇷", "Argentína"),
        "AW": ("🇦🇼", "Aruba"),
        "BR": ("🇧🇷", "Brazília"),
        "CM": ("🇨🇲", "Kamerun"),
        "CL": ("🇨🇱", "Chile"),
        "CO": ("🇨🇴", "Kolumbia"),
        "CD": ("🇨🇩", "Kongói DK"),
        "GR": ("🇬🇷", "Görögország"),
        "HK": ("🇭🇰", "Hongkong"),
        "IN": ("🇮🇳", "India"),
        "IR": ("🇮🇷", "Irán"),
        "XK": ("🇽🇰", "Koszovó"),
        "ML": ("🇲🇱", "Mali"),
        "MX": ("🇲🇽", "Mexikó"),
        "NG": ("🇳🇬", "Nigéria"),
        "PE": ("🇵🇪", "Peru"),
        "PT": ("🇵🇹", "Portugália"),
        "RS": ("🇷🇸", "Szerbia"),
        "SK": ("🇸🇰", "Szlovákia"),
        "UA": ("🇺🇦", "Ukrajna"),
        "UY": ("🇺🇾", "Uruguay"),
        "VE": ("🇻🇪", "Venezuela"),
        "FO": ("🇫🇴", "Feröer"),
        "MD": ("🇲🇩", "Moldova"),
        "KZ": ("🇰🇿", "Kazahsztán"),
        "LU": ("🇱🇺", "Luxemburg"),
        "ME": ("🇲🇪", "Montenegró"),
        "MK": ("🇲🇰", "Észak-Macedónia"),
        "BG": ("🇧🇬", "Bulgária"),
        "EE": ("🇪🇪", "Észtország"),
        "IS": ("🇮🇸", "Izland"),
        "LT": ("🇱🇹", "Litvánia"),
        "MT": ("🇲🇹", "Málta"),
        "GI": ("🇬🇮", "Gibraltár"),
        "LI": ("🇱🇮", "Liechtenstein"),
        "AD": ("🇦🇩", "Andorra"),
        "AZ": ("🇦🇿", "Azerbajdzsán"),
        "SO": ("🇸🇴", "Szomália"),
        "BJ": ("🇧🇯", "Benin"),
        "CF": ("🇨🇫", "Közép-afrikai Köztársaság"),
        "GQ": ("🇬🇶", "Egyenlítői-Guinea"),
        "ER": ("🇪🇷", "Eritrea"),
        "GN": ("🇬🇳", "Guinea"),
        "NA": ("🇳🇦", "Namíbia"),
        "SD": ("🇸🇩", "Szudán"),
        "ZW": ("🇿🇼", "Zimbabwe"),
        "ZA": ("🇿🇦", "Dél-afrikai Köztársaság"),
        "EG": ("🇪🇬", "Egyiptom"),
        "MA": ("🇲🇦", "Marokkó"),
        "DZ": ("🇩🇿", "Algéria"),
        "TN": ("🇹🇳", "Tunézia"),
        "US": ("🇺🇸", "Egyesült Államok"),
        "CA": ("🇨🇦", "Kanada"),
        "JP": ("🇯🇵", "Japán"),
        "KR": ("🇰🇷", "Dél-Korea"),
        "CN": ("🇨🇳", "Kína"),
        "AU": ("🇦🇺", "Ausztrália"),
        "BM": ("🇧🇲", "Bermuda"),
        "CU": ("🇨🇺", "Kuba"),
        "GD": ("🇬🇩", "Grenada"),
        "GP": ("🇬🇵", "Guadeloupe"),
        "GY": ("🇬🇾", "Guyana"),
        "PR": ("🇵🇷", "Puerto Rico"),
        "TO": ("🇹🇴", "Tonga"),
        "ID": ("🇮🇩", "Indonézia"),
        "KG": ("🇰🇬", "Kirgizisztán"),
        "UZ": ("🇺🇿", "Üzbegisztán"),
        "SZ": ("🇸🇿", "Eswatini"),
        "TZ": ("🇹🇿", "Tanzánia"),
        // Régi/demó kódok kompatibilitása.
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

    /// A fejléc második sora: nemzetközi versenynél nincs ország.
    var subtitle: String {
        if country == "Ismeretlen" || country.isEmpty {
            return tier <= 0 ? "NEMZETKÖZI · KUPA" : "NEMZETKÖZI"
        }
        return "\(country.uppercased()) · \(divisionLabel.uppercased())"
    }

    func filtering(_ filter: MatchFilter) -> LeagueGroup? {
        let kept = matches.filter { filter.matches(status: $0.status) }
        guard !kept.isEmpty else { return nil }
        return LeagueGroup(id: id, name: name, country: country, flag: flag,
                           tier: tier, priority: priority, matches: kept)
    }

    /// Kiemelt kör: a top-5 + NB I/II (priority ≤ 10) versenyei.
    static func featured(_ groups: [LeagueGroup]) -> [LeagueGroup] {
        groups.filter { $0.priority <= 10 }
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