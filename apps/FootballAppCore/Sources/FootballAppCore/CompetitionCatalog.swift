import Foundation

/// Versenynév-kanonizálás a napi listához.
///
/// A források ugyanazt a ligát több néven küldik (`J. League` /
/// `Japanese J1 League`, `1st Lig` / `1. Lig`, `Primera RFEF - 2. csoport` /
/// `Primera Federacion - Group 2`). A listán egy liga egy szekció legyen, ezért
/// a megjelenített nevet és a csoportosítási kulcsot egy rövid, tesztelt
/// aliastábla egységesíti. Az alias csak akkor érvényes, ha az ország is
/// egyezik: azonos nevű liga több országban is létezik (pl. `Premier League`).
public enum CompetitionCatalog {
    public struct Canonical: Equatable, Sendable {
        public let name: String
        /// Csoportosítási kulcs: ország + kanonikus név.
        public let key: String

        public init(name: String, key: String) {
            self.name = name
            self.key = key
        }
    }

    /// Kanonikus név + kulcs a nyers versenynévből és az országból.
    ///
    /// - Parameters:
    ///   - name: az API-ból kapott versenynév (lehet üres).
    ///   - countryCode: az API országkódja (ISO2/ISO3/alias, lehet `nil`).
    ///   - countryName: az ország megjelenített neve (a zászló-katalógusból).
    public static func canonical(
        name: String?,
        countryCode: String?,
        countryName: String?
    ) -> Canonical {
        let raw = name?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let rawCode = countryCode?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let iso2 = CountryCatalog.iso2(for: rawCode) ?? fold(rawCode)
        let displayName = countryName?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let normalizedName = fold(raw)
        let countryKeys = [fold(iso2), fold(rawCode), fold(displayName)].filter { !$0.isEmpty }
        let canonicalName = alias(for: normalizedName, countryKeys: countryKeys)
            ?? (raw.isEmpty ? (displayName.isEmpty ? "Nemzetközi" : displayName) : raw)
        let countryKey = iso2.isEmpty ? fold(displayName) : iso2
        return Canonical(name: canonicalName, key: "\(countryKey)|\(fold(canonicalName))")
    }

    // MARK: - Aliasok

    /// Ismert névvariánsok: (normalizált név, ország-szűrő, kanonikus név).
    /// Az ország-szűrő ISO2 kód vagy megjelenített országnév (kisbetűs);
    /// `nil` esetén a szabály országtól független.
    private static let aliases: [(name: String, country: String?, canonical: String)] = [
        // Japán: a J. League a J1 League régi neve.
        ("j league", "JP", "J1 League"),
        ("japanese j1 league", "JP", "J1 League"),
        ("j1 league", "JP", "J1 League"),
        // Török másodosztály.
        ("1st lig", "TR", "1. Lig"),
        // Lengyel másodosztály.
        ("1 osztaly", "PL", "I Liga"),
        // Dán másodosztály.
        ("1 osztaly", "DK", "1st Division"),
        ("1st division", "DK", "1st Division"),
        // Walesi élvonal: a GB-s országkód alatt „Premier League" néven jött.
        ("premier league", "wales", "Cymru Premier"),
        ("premier league", "gb wls", "Cymru Premier"),
        // Belga másodosztály 2022 óta Challenger Pro League.
        ("first division b", "BE", "Challenger Pro League"),
        // Ukrán élvonal.
        ("ukrainian premier league", "UA", "Premier League"),
        // Portugál kupa.
        ("taca de portugal", "PT", "Taça de Portugal"),
        ("portugal cup", "PT", "Taça de Portugal"),
        // Spanyol harmadosztály csoportjai.
        ("primera rfef 1 csoport", "ES", "Primera Federación: Group 1"),
        ("primera federacion group 1", "ES", "Primera Federación: Group 1"),
        ("primera division rfef group 1", "ES", "Primera Federación: Group 1"),
        ("primera rfef 2 csoport", "ES", "Primera Federación: Group 2"),
        ("primera federacion group 2", "ES", "Primera Federación: Group 2"),
        ("primera division rfef group 2", "ES", "Primera Federación: Group 2"),
        // Indiai női liga: a magyar nevű entitás maradt meg merge után.
        ("iwl noi", "IN", "IWL Women"),
        // Argentin bajnoki rájátszás.
        ("liga profesional clausura play off", "AR", "Liga Profesional: Clausura Playoff"),
        ("liga profesional clausura playoff", "AR", "Liga Profesional: Clausura Playoff"),
    ]

    private static func alias(for normalized: String, countryKeys: [String]) -> String? {
        for entry in aliases where entry.name == normalized {
            guard let constraint = entry.country.map(fold) else { return entry.canonical }
            if countryKeys.contains(constraint) {
                return entry.canonical
            }
        }
        return nil
    }

    // MARK: - Normalizálás

    /// Kisbetűs, ékezet nélküli, írásjelek nélküli alak a hasonlításhoz.
    static func fold(_ value: String) -> String {
        let stripped = value.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: nil)
        var out = ""
        for scalar in stripped.unicodeScalars {
            if CharacterSet.alphanumerics.contains(scalar) {
                out.unicodeScalars.append(scalar)
            } else {
                out.append(" ")
            }
        }
        return out.split(whereSeparator: { $0 == " " }).joined(separator: " ")
    }
}