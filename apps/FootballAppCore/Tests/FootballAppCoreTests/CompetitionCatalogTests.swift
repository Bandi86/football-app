import Testing
@testable import FootballAppCore

@Suite("CompetitionCatalog")
struct CompetitionCatalogTests {
    @Test("Japán top liga névvariánsai ugyanarra a névre és kulcsra oldódnak")
    func japaneseTopFlightVariants() {
        let a = CompetitionCatalog.canonical(
            name: "J. League", countryCode: "JP", countryName: "Japán")
        let b = CompetitionCatalog.canonical(
            name: "Japanese J1 League", countryCode: "JP", countryName: "Japán")
        #expect(a.name == "J1 League")
        #expect(a == b)
    }

    @Test("Az ország a kulcs része: azonos nevű liga két országban nem olvad össze")
    func sameNameDifferentCountry() {
        let tr = CompetitionCatalog.canonical(
            name: "1. Lig", countryCode: "TR", countryName: "Törökország")
        let other = CompetitionCatalog.canonical(
            name: "1. Lig", countryCode: "CZ", countryName: "Csehország")
        #expect(tr.key != other.key)
    }

    @Test("Török másodosztály két írásmódja egy csoport")
    func turkishSecondTier() {
        let a = CompetitionCatalog.canonical(
            name: "1st Lig", countryCode: "TR", countryName: "Törökország")
        let b = CompetitionCatalog.canonical(
            name: "1. Lig", countryCode: "TR", countryName: "Törökország")
        #expect(a.name == "1. Lig")
        #expect(a.key == b.key)
    }

    @Test("Walesi élvonal a GB kód alatt is Cymru Premier")
    func welshTopFlight() {
        let welsh = CompetitionCatalog.canonical(
            name: "Premier League", countryCode: "GB-WLS", countryName: "Wales")
        let cymru = CompetitionCatalog.canonical(
            name: "Cymru Premier", countryCode: "GB-WLS", countryName: "Wales")
        #expect(welsh.name == "Cymru Premier")
        #expect(welsh.key == cymru.key)
        // Az angol Premier League nem eshet ugyanabba a csoportba.
        let english = CompetitionCatalog.canonical(
            name: "Premier League", countryCode: "GB-ENG", countryName: "Anglia")
        #expect(english.name == "Premier League")
        #expect(english.key != welsh.key)
    }

    @Test("Spanyol harmadosztály csoportnevei egységesek")
    func spanishThirdTierGroups() {
        let a = CompetitionCatalog.canonical(
            name: "Primera RFEF - 2. csoport", countryCode: "ES", countryName: "Spanyolország")
        let b = CompetitionCatalog.canonical(
            name: "Primera Federacion - Group 2", countryCode: "ES", countryName: "Spanyolország")
        #expect(a.name == "Primera Federación: Group 2")
        #expect(a.key == b.key)
    }

    @Test("A magyar nevű indiai női liga angol névre oldódik")
    func hungarianWomensLeagueName() {
        let canonical = CompetitionCatalog.canonical(
            name: "IWL - női", countryCode: "IN", countryName: "India")
        #expect(canonical.name == "IWL Women")
    }

    @Test("Az ismeretlen név változatlanul, de stabil kulccsal megy tovább")
    func unknownNamePassesThrough() {
        let result = CompetitionCatalog.canonical(
            name: "Regionalliga West", countryCode: "DE", countryName: "Németország")
        #expect(result.name == "Regionalliga West")
        #expect(result.key.hasPrefix("DE|"))
    }

    @Test("Üres név az országnevet kapja, ismeretlen országnál Nemzetközi")
    func emptyNameFallbacks() {
        let named = CompetitionCatalog.canonical(
            name: "", countryCode: "HU", countryName: "Magyarország")
        #expect(named.name == "Magyarország")
        let international = CompetitionCatalog.canonical(
            name: nil, countryCode: nil, countryName: nil)
        #expect(international.name == "Nemzetközi")
    }

    @Test("A női liga egyszer, (női) utótaggal jelenik meg — angol és magyar jelöléssel is")
    func womensLeaguesGetAMarker() {
        let english = CompetitionCatalog.canonical(
            name: "Women's WE League", countryCode: "JP", countryName: "Japán")
        #expect(english.name == "WE League (női)")
        let hungarian = CompetitionCatalog.canonical(
            name: "A-Liga - női", countryCode: "DK", countryName: "Dánia")
        #expect(hungarian.name == "A-Liga (női)")
        let french = CompetitionCatalog.canonical(
            name: "Premiere Ligue Femenine", countryCode: "FR", countryName: "Franciaország")
        #expect(french.name == "Premiere Ligue (női)")
        let alreadyMarked = CompetitionCatalog.canonical(
            name: "Bundesliga (női)", countryCode: "DE", countryName: "Németország")
        #expect(alreadyMarked.name == "Bundesliga (női)")
    }

    @Test("A női versenyek széles köre felismerhető, a férfi ligák nem esnek bele")
    func womensCompetitionMarkersAreBroad() {
        let womensNames = [
            "Liga F", "Liga FPD", "Frauen-Bundesliga", "Women's Serie A",
            "Serie A Femminile", "NWSL", "WSL", "WE League", "NCAAW",
            "A-League Women", "Première Ligue (női)", "Toppserien",
            "Damallsvenskan", "Eredivisie Vrouwen", "Liga MX Femenil",
        ]
        for name in womensNames {
            #expect(CompetitionCatalog.isWomensCompetition(name), "\(name) női verseny")
            #expect(
                CompetitionCatalog.canonical(
                    name: name, countryCode: nil, countryName: nil).name.hasSuffix("(női)"),
                "\(name) (női) utótagot kap")
        }
        let mensNames = ["Liga", "Serie A", "Bundesliga", "Premier League", "La Liga", "Liga MX"]
        for name in mensNames {
            #expect(!CompetitionCatalog.isWomensCompetition(name), "\(name) férfi verseny")
        }
    }

    @Test("A női és a férfi liga külön szekció marad")
    func womensLeagueDoesNotMergeWithMens() {
        let mens = CompetitionCatalog.canonical(
            name: "Ligue 1", countryCode: "FR", countryName: "Franciaország")
        let womens = CompetitionCatalog.canonical(
            name: "Premiere Ligue Femenine", countryCode: "FR", countryName: "Franciaország")
        #expect(mens.key != womens.key)
    }
}
