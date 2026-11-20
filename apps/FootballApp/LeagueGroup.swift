import Foundation
import FootballAppCore

/// A verseny- és csapatnézetek ország-megjelenítése. A zászló és a magyar
/// országnév egyetlen, tesztelt katalógusból jön (`CountryCatalog`), hogy a
/// kézi kód-tábla ne csússzon el a valós ISO-készlettől.
enum CountryDisplay {
    /// A brit hazai nemzeteket a forrás néha `GB` kóddal, de walesi/skót
    /// névvel adja (pl. Cymru Premier). Ilyenkor a hazai nemzet zászlaját és
    /// nevét mutatjuk, hogy a liga országa felismerhető legyen.
    private static let homeNations: [(String, String)] = [
        ("wales", "GB-WLS"),
        ("scotland", "GB-SCT"),
        ("england", "GB-ENG"),
        ("northern ireland", "GB-NIR"),
        ("észak-írország", "GB-NIR"),
    ]

    static func display(code: String?, name: String?) -> (flag: String, name: String) {
        let trimmedCode = code?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let trimmedName = name?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if trimmedCode.isEmpty || trimmedCode.uppercased() == "GB" || trimmedCode.uppercased() == "GBR" {
            let foldedName = trimmedName.folding(options: [.diacriticInsensitive, .caseInsensitive],
                                                 locale: Locale(identifier: "hu_HU"))
            if let nation = homeNations.first(where: { $0.0 == foldedName })?.1 {
                let resolved = CountryCatalog.display(code: nation, name: trimmedName)
                return (resolved.flag, resolved.name)
            }
        }
        let resolved = CountryCatalog.display(code: code, name: name)
        return (resolved.flag, resolved.name)
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

    /// A liga-fejléc neve az API-ból jön, csak a használhatatlan értékeket
    /// cseréljük: üres név vagy az országkód maga helyett a magyar országnevet
    /// (ismeretlen országnál „Nemzetközi"-t) mutatjuk.
    static func canonicalName(raw: String?, countryCode: String?, countryName: String) -> String {
        let trimmedRaw = raw?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let trimmedCountry = countryName.trimmingCharacters(in: .whitespacesAndNewlines)
        let fallback = (trimmedCountry.isEmpty || trimmedCountry == "Ismeretlen")
            ? "Nemzetközi"
            : trimmedCountry
        if trimmedRaw.isEmpty { return fallback }
        if let code = countryCode?.trimmingCharacters(in: .whitespacesAndNewlines),
           !code.isEmpty,
           trimmedRaw.caseInsensitiveCompare(code) == .orderedSame {
            return fallback
        }
        return trimmedRaw
    }

    /// Csoportosítás verseny szerint. A csoport metaadatai a kezdés + id szerint
    /// determinisztikusan kiválasztott első meccsből jönnek, a csoportokon belüli
    /// sorrend pedig kezdés, majd id szerint rendezett.
    static func fromDay(_ matches: [DayMatch]) -> [LeagueGroup] {
        // Nem verseny-id szerint csoportosítunk: ugyanaz a liga több
        // verseny-entitás alatt is jöhet (pl. „J. League" / „Japanese J1
        // League"). A kulcs a kanonikus ország + kanonikus versenynév.
        let grouped = Dictionary(grouping: matches) { match -> String in
            let display = CountryDisplay.display(code: match.countryCode, name: match.countryName)
            // A használhatatlan versenynevet (üres, csak az országkód) itt
            // cseréljük országnévre, különben a csoport neve „hun" lenne.
            let name = canonicalName(raw: match.competitionName,
                                     countryCode: match.countryCode,
                                     countryName: display.name)
            return CompetitionCatalog.canonical(
                name: name,
                countryCode: match.countryCode,
                countryName: display.name
            ).key
        }
        return grouped.map { key, matches in
            let ordered = matches.sorted {
                if $0.kickoffAt != $1.kickoffAt { return $0.kickoffAt < $1.kickoffAt }
                return $0.id.uuidString < $1.id.uuidString
            }
            let first = ordered[0]
            let display = CountryDisplay.display(code: first.countryCode, name: first.countryName)
            let canonical = CompetitionCatalog.canonical(
                name: canonicalName(raw: first.competitionName,
                                    countryCode: first.countryCode,
                                    countryName: display.name),
                countryCode: first.countryCode,
                countryName: display.name
            )
            return LeagueGroup(
                id: key,
                name: canonical.name,
                country: display.name,
                flag: display.flag,
                // Egy liga két entitása eltérő tier/priority értékkel jöhet:
                // a csoport a legjobb (legkisebb) értékeket örökli.
                tier: ordered.compactMap(\.tier).min() ?? 1,
                priority: ordered.compactMap(\.priority).min() ?? 100,
                matches: ordered.map(MatchCardModel.from)
            )
        }
        .sorted { lhs, rhs in
            if lhs.priority != rhs.priority { return lhs.priority < rhs.priority }
            if lhs.country != rhs.country { return lhs.country < rhs.country }
            if lhs.name != rhs.name { return lhs.name < rhs.name }
            return lhs.id < rhs.id
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