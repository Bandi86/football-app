import Foundation
import Testing
@testable import FootballAppCore

@Suite struct CountryCatalogTests {
    @Test func iso3CodeResolvesToFlagAndHungarianName() {
        let indonesia = CountryCatalog.display(code: "IDN", name: nil)
        #expect(indonesia.flag == "🇮🇩")
        #expect(indonesia.name == "Indonézia")

        #expect(CountryCatalog.display(code: "HUN", name: nil).name == "Magyarország")
        #expect(CountryCatalog.display(code: "DEU", name: nil).name == "Németország")
        #expect(CountryCatalog.display(code: "ESP", name: nil).name == "Spanyolország")
    }

    @Test func uaeAliasHasEmiratiFlagAndHungarianName() {
        let display = CountryCatalog.display(code: "UAE", name: nil)
        #expect(display.flag == "🇦🇪")
        #expect(display.name == "Arab Emírségek")
        // A direkt ISO alpha-2 kód ugyanazt adja.
        #expect(CountryCatalog.display(code: "AE", name: nil) == display)
    }

    @Test func britishHomeNationsUseTheirOwnFlags() {
        let scotland = CountryCatalog.display(code: "GB-SCT", name: nil)
        #expect(scotland.flag == "🏴󠁧󠁢󠁳󠁣󠁴󠁿")
        #expect(scotland.name == "Skócia")

        let england = CountryCatalog.display(code: "GB-ENG", name: nil)
        #expect(england.flag == "🏴󠁧󠁢󠁥󠁮󠁧󠁿")
        #expect(england.name == "Anglia")

        #expect(CountryCatalog.display(code: "GB-WLS", name: nil).name == "Wales")
        #expect(CountryCatalog.display(code: "GB-WLS", name: nil).flag == "🏴󠁧󠁢󠁷󠁬󠁳󠁿")
        // Észak-Írországnak nincs Unicode-zászlaja: a brit zászló jelenik meg.
        #expect(CountryCatalog.display(code: "GB-NIR", name: nil).flag == "🇬🇧")
        #expect(CountryCatalog.display(code: "GB-NIR", name: nil).name == "Észak-Írország")

        // Labdarúgó-aliasok ugyanezekre a nemzetekre oldódnak.
        #expect(CountryCatalog.display(code: "SCO", name: nil) == scotland)
        #expect(CountryCatalog.display(code: "ENG", name: nil) == england)
        #expect(CountryCatalog.display(code: "WAL", name: nil).name == "Wales")
        #expect(CountryCatalog.display(code: "NIR", name: nil).name == "Észak-Írország")

        let unitedKingdom = CountryCatalog.display(code: "GB", name: nil)
        #expect(unitedKingdom.flag == "🇬🇧")
        #expect(unitedKingdom.name == "Egyesült Királyság")
    }

    @Test func unknownCodeFallsBackToWhiteFlagAndProvidedName() {
        let unknown = CountryCatalog.display(code: "XYZ", name: nil)
        #expect(unknown.flag == CountryCatalog.unknownFlag)
        #expect(unknown.flag == "🏳️")
        #expect(unknown.name == "XYZ")

        let withName = CountryCatalog.display(code: "XYZ", name: "Ismeretlen ország")
        #expect(withName.flag == "🏳️")
        #expect(withName.name == "Ismeretlen ország")

        let nothing = CountryCatalog.display(code: nil, name: nil)
        #expect(nothing.flag == "🏳️")
        #expect(nothing.name == "Ismeretlen")
    }

    @Test func footballSourceAliasesResolve() {
        #expect(CountryCatalog.display(code: "MAS", name: nil).name == "Malajzia")
        #expect(CountryCatalog.display(code: "GER", name: nil).flag == "🇩🇪")
        #expect(CountryCatalog.display(code: "NED", name: nil).name == "Hollandia")
        #expect(CountryCatalog.display(code: "SUI", name: nil).name == "Svájc")
        #expect(CountryCatalog.display(code: "SIN", name: nil).name == "Szingapúr")
        #expect(CountryCatalog.display(code: "TAN", name: nil).name == "Tanzánia")
        #expect(CountryCatalog.display(code: "CHI", name: nil).name == "Chile")
        #expect(CountryCatalog.display(code: "URU", name: nil).name == "Uruguay")
        #expect(CountryCatalog.display(code: "VIE", name: nil).name == "Vietnám")
        #expect(CountryCatalog.display(code: "NIG", name: nil).name == "Niger")
        #expect(CountryCatalog.display(code: "NGA", name: nil).name == "Nigéria")
    }

    @Test func addedIso3AliasesMatchTheirIso2Flag() {
        // A feed ISO3-at küld (pl. BUL), a katalógus ISO2-re oldja; a zászló
        // és a név ugyanaz kell legyen, mint a hivatalos párnál.
        let pairs: [(alias: String, canonical: String)] = [
            ("BUL", "BG"), ("ROM", "RO"), ("KSA", "SA"), ("KVX", "XK"),
            ("GEO", "GE"),
        ]
        for pair in pairs {
            let alias = CountryCatalog.display(code: pair.alias, name: nil)
            let canonical = CountryCatalog.display(code: pair.canonical, name: nil)
            #expect(alias.flag != CountryCatalog.unknownFlag, "\(pair.alias) fehér zászlót kapott")
            #expect(alias == canonical, "\(pair.alias) eltér \(pair.canonical)-tól")
            #expect(CountryCatalog.iso2(for: pair.alias) == pair.canonical)
        }

        // A konkrét hiba: Bulgária a feedben BUL, eddig fehér zászló volt.
        let bulgaria = CountryCatalog.display(code: "BUL", name: "Bulgária")
        #expect(bulgaria.flag == "🇧🇬")
        #expect(bulgaria.name == "Bulgária")
    }

    @Test func everyFeedAliasResolvesToTheSameDisplayAsItsCanonicalCode() {
        // A felhasználó által jelzett teljes alias-készlet: a feed ISO3/FIFA
        // alakjai ugyanazt a zászlót és nevet adják, mint a kanonikus kód.
        let pairs: [(alias: String, canonical: String)] = [
            ("BUL", "BG"), ("ROM", "RO"), ("CHI", "CL"), ("MAS", "MY"),
            ("IDN", "ID"), ("KSA", "SA"), ("UAE", "AE"), ("BIH", "BA"),
            ("GEO", "GE"), ("CYP", "CY"), ("MKD", "MK"), ("SMR", "SM"),
            ("KVX", "XK"), ("MNE", "ME"), ("NIG", "NE"), ("BOT", "BW"),
            ("BFA", "BF"), ("LBY", "LY"), ("TAN", "TZ"), ("VIE", "VN"),
            ("SIN", "SG"), ("HKG", "HK"), ("PAR", "PY"), ("URY", "UY"),
            ("DEN", "DK"), ("NED", "NL"), ("SUI", "CH"), ("GRE", "GR"),
            ("GER", "DE"), ("POR", "PT"), ("IRL", "IE"), ("SCO", "GB-SCT"),
            ("WAL", "GB-WLS"), ("ENG", "GB-ENG"), ("NIR", "GB-NIR"),
        ]
        for pair in pairs {
            let alias = CountryCatalog.display(code: pair.alias, name: nil)
            let canonical = CountryCatalog.display(code: pair.canonical, name: nil)
            #expect(alias.flag != CountryCatalog.unknownFlag, "\(pair.alias) fehér zászlót kapott")
            #expect(alias == canonical, "\(pair.alias) eltér \(pair.canonical)-tól")
        }
    }

    @Test func requiredCountryCodesAllResolve() {
        let codes = ["IDN", "MAS", "VIE", "ALB", "UAE", "GER", "DEU", "ESP", "HUN",
                     "BOT", "KEN", "SIN", "TAN", "CHI", "PAR", "NIG", "PAN", "PER",
                     "SMR", "VEN", "MDA", "RSA", "NED", "SUI", "POR", "GRE", "DEN",
                     "SWE", "NOR", "CRO", "SRB", "POL", "TUR", "BEL", "AUT", "CZE",
                     "SVK", "SVN", "UKR", "ARG", "BRA", "URU", "COL", "MEX", "USA",
                     "JPN", "KOR", "CHN", "AUS", "IRN", "IRQ", "SAU", "QAT", "EGY",
                     "MAR", "ALG", "TUN", "NGA", "GHA",
                     "ENG", "SCO", "WAL", "NIR"]
        for code in codes {
            let display = CountryCatalog.display(code: code, name: nil)
            #expect(display.flag != CountryCatalog.unknownFlag, "\(code) zászló nélkül maradt")
            #expect(display.name != code, "\(code) név nélkül maradt")
            #expect(CountryCatalog.iso2(for: code) != nil, "\(code) nem oldódik ISO2-re")
        }
    }

    @Test func flagIsComputedFromRegionalIndicators() {
        #expect(CountryCatalog.flag(forISO2: "HU") == "🇭🇺")
        #expect(CountryCatalog.flag(forISO2: "gb") == "🇬🇧")
        #expect(CountryCatalog.flag(forISO2: " hu ") == "🇭🇺")
        // Nem két latin betű: nincs zászló.
        #expect(CountryCatalog.flag(forISO2: "H") == "🏳️")
        #expect(CountryCatalog.flag(forISO2: "12") == "🏳️")
    }

    @Test func inputIsTrimmedAndCaseInsensitive() {
        let display = CountryCatalog.display(code: "  hun ", name: "  Hungary  ")
        #expect(display.flag == "🇭🇺")
        #expect(display.name == "Magyarország")
        #expect(CountryCatalog.hungarianName(forISO2: " hu ") == "Magyarország")
        #expect(CountryCatalog.hungarianName(forISO2: "ZZ") == nil)
    }

    @Test func catalogCoversTheWholeISOAlpha2Set() {
        // A teljes ISO 3166-1 készlet + Koszovó (XK); a zászló mindegyikre számolható.
        #expect(CountryCatalog.hungarianNames.count >= 250)
        for (iso2, name) in CountryCatalog.hungarianNames {
            #expect(iso2.count == 2)
            #expect(!name.isEmpty)
            #expect(CountryCatalog.flag(forISO2: iso2) != CountryCatalog.unknownFlag)
            #expect(CountryCatalog.hungarianName(forISO2: iso2) == name)
        }
    }
}