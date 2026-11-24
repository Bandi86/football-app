import XCTest
import FootballAppCore
@testable import FootballApp

final class LeagueGroupTests: XCTestCase {
    private func decode(_ json: String) throws -> [DayMatch] {
        let data = json.data(using: .utf8)!
        return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data).matches
    }

    func testGroupsSortByPriorityThenCountry() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6},
          {"id":"00000000-0000-0000-0000-000000000002","kickoff_at":"2026-11-11T16:00:00Z",
           "status":"scheduled","home":"C","away":"D","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a2","competition_name":"Premier League",
           "country_code":"ENG","country_name":"England","tier":1,"priority":1},
          {"id":"00000000-0000-0000-0000-000000000003","kickoff_at":"2026-11-11T19:00:00Z",
           "status":"scheduled","home":"E","away":"F","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a2","competition_name":"Premier League",
           "country_code":"ENG","country_name":"England","tier":1,"priority":1}
        ]}
        """)
        let groups = LeagueGroup.fromDay(matches)
        XCTAssertEqual(groups.count, 2)
        XCTAssertEqual(groups[0].name, "Premier League")
        XCTAssertEqual(groups[0].matches.count, 2)
        XCTAssertEqual(groups[0].country, "Anglia")
        XCTAssertEqual(groups[1].name, "NB I")
        XCTAssertEqual(groups[1].country, "Magyarország")
    }

    /// S122: a választható rendezési módok (kiemelt / ország A–Z / név A–Z)
    /// determinisztikusan átrendezik ugyanazokat a ligákat.
    func testSortModesReorderTheSameGroups() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6},
          {"id":"00000000-0000-0000-0000-000000000002","kickoff_at":"2026-11-11T16:00:00Z",
           "status":"scheduled","home":"C","away":"D","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a2","competition_name":"Premier League",
           "country_code":"ENG","country_name":"England","tier":1,"priority":1},
          {"id":"00000000-0000-0000-0000-000000000003","kickoff_at":"2026-11-11T19:00:00Z",
           "status":"scheduled","home":"E","away":"F","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a3","competition_name":"Bundesliga",
           "country_code":"GER","country_name":"Germany","tier":1,"priority":3}
        ]}
        """)
        let groups = LeagueGroup.fromDay(matches)

        let featured = LeagueGroup.ordered(groups, mode: .featured)
        XCTAssertEqual(featured.map(\.name), ["Premier League", "Bundesliga", "NB I"])

        let byCountry = LeagueGroup.ordered(groups, mode: .country)
        XCTAssertEqual(byCountry.map(\.country), ["Anglia", "Magyarország", "Németország"])

        let byName = LeagueGroup.ordered(groups, mode: .name)
        XCTAssertEqual(byName.map(\.name), ["Bundesliga", "NB I", "Premier League"])
    }

    func testCardModelCarriesDisplayFields() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"finished","home":"Újpest","away":"Győr","home_score":2,"away_score":1,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000a1","competition_name":"NB I",
           "country_code":"HUN","country_name":"Hungary","tier":1,"priority":6,
           "home_ht":1,"away_ht":0,"home_position":8,"away_position":6,
           "odds":{"home":2.2,"draw":3.3,"away":3.2,"btts_yes":1.85,"btts_no":1.95,"over25":2.15,"under25":1.7},
           "market_count":18,"odds_source":"Demo odds feed"}
        ]}
        """)
        let card = MatchCardModel.from(matches[0])
        XCTAssertEqual(card.home, "Újpest")
        XCTAssertEqual(card.homeScore, 2)
        XCTAssertEqual(card.homeHt, 1)
        XCTAssertEqual(card.homePosition, 8)
        XCTAssertEqual(card.awayPosition, 6)
        XCTAssertEqual(card.odds?.home, 2.2)
        XCTAssertEqual(card.markets, 18)
        XCTAssertEqual(card.oddsSource, "Demo odds feed")
        XCTAssertEqual(card.winner1x2, .home)
        XCTAssertEqual(card.bttsYesWon, true)
        XCTAssertEqual(card.over25Won, true)
        XCTAssertNil(card.minute)
    }

    func testWelshLeagueShowsWalesNotUnitedKingdom() {
        let display = CountryDisplay.display(code: "GB", name: "Wales")
        XCTAssertEqual(display.name, "Wales")
        XCTAssertEqual(display.flag, "🏴󠁧󠁢󠁷󠁬󠁳󠁿")
        // A vegyes nyelvű név is felismerhető.
        let scottish = CountryDisplay.display(code: nil, name: "Scotland")
        XCTAssertEqual(scottish.name, "Skócia")
    }

    func testUnknownCountryFallsBackToName() {
        let display = CountryDisplay.display(code: "XYZ", name: "Ismeretlen ország")
        XCTAssertEqual(display.name, "Ismeretlen ország")
        XCTAssertEqual(display.flag, "🏳️")
    }

    func testFeaturedFilterKeepsOnlyPriorityTenOrLess() throws {
        let matches = try decode("""
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000011","kickoff_at":"2026-11-11T18:00:00Z",
           "status":"scheduled","home":"A","away":"B","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000b1","competition_name":"Premier League",
           "country_code":"GB","country_name":"England","tier":1,"priority":6},
          {"id":"00000000-0000-0000-0000-000000000012","kickoff_at":"2026-11-11T16:00:00Z",
           "status":"scheduled","home":"C","away":"D","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000b2","competition_name":"Liga X",
           "country_code":"ES","country_name":"Spanyolország","tier":1,"priority":50}
        ]}
        """)
        let groups = LeagueGroup.fromDay(matches)
        XCTAssertEqual(groups.count, 2)
        let featured = LeagueGroup.featured(groups)
        XCTAssertEqual(featured.count, 1)
        XCTAssertEqual(featured[0].name, "Premier League")
    }

    // MARK: - Segédek a csoportosítási tesztekhez

    private func matchJSON(
        id: String,
        kickoff: String,
        competitionId: String,
        competitionName: String?,
        countryCode: String?,
        countryName: String?,
        priority: Int,
        tier: Int = 1
    ) -> String {
        func literal(_ value: String?) -> String {
            value.map { "\"\($0)\"" } ?? "null"
        }
        return """
        {"id":"\(id)","kickoff_at":"\(kickoff)","status":"scheduled","home":"A","away":"B",
         "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[],
         "competition_id":"\(competitionId)","competition_name":\(literal(competitionName)),
         "country_code":\(literal(countryCode)),"country_name":\(literal(countryName)),
         "tier":\(tier),"priority":\(priority)}
        """
    }

    private func roundMatchJSON(
        id: String,
        competitionId: String,
        competitionName: String,
        countryCode: String?,
        roundLabel: String?
    ) -> String {
        func literal(_ value: String?) -> String {
            value.map { "\"\($0)\"" } ?? "null"
        }
        return """
        {"id":"\(id)","kickoff_at":"2026-11-11T18:00:00Z","status":"scheduled","home":"A","away":"B",
         "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[],
         "competition_id":"\(competitionId)","competition_name":"\(competitionName)",
         "country_code":\(literal(countryCode)),"country_name":null,
         "tier":1,"priority":6,"round_label":\(literal(roundLabel))}
        """
    }

    private func groups(_ matches: [String]) throws -> [LeagueGroup] {
        let json = """
        {"date":"2026-11-11","timezone":"Europe/Budapest","matches":[\(matches.joined(separator: ","))]}
        """
        return LeagueGroup.fromDay(try decode(json))
    }

    // MARK: - Csoport-sorrend

    func testGroupsSortByPriorityThenCountryThenLeagueName() throws {
        let groups = try groups([
            matchJSON(id: "00000000-0000-0000-0000-000000000021",
                      kickoff: "2026-11-11T16:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000c1",
                      competitionName: "NB II", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000022",
                      kickoff: "2026-11-11T12:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000c2",
                      competitionName: "Liga 1", countryCode: "IDN", countryName: "Indonesia",
                      priority: 5),
            matchJSON(id: "00000000-0000-0000-0000-000000000023",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000c3",
                      competitionName: "NB I", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
        ])
        XCTAssertEqual(groups.map(\.name), ["Liga 1", "NB I", "NB II"])
        XCTAssertEqual(groups.map(\.country), ["Indonézia", "Magyarország", "Magyarország"])
    }

    func testGroupOrderTieBreaksOnCanonicalLeagueName() throws {
        // Azonos ország és priority mellett a kanonikus liganev dönt, nem a
        // verseny-entitás azonosítója.
        let groups = try groups([
            matchJSON(id: "00000000-0000-0000-0000-000000000031",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000f2",
                      competitionName: "B Liga", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000032",
                      kickoff: "2026-11-11T17:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000f1",
                      competitionName: "A Liga", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
        ])
        XCTAssertEqual(groups.map(\.name), ["A Liga", "B Liga"])
    }

    func testGroupsWithDifferentCanonicalNamesStaySeparate() throws {
        // Ugyanaz az ország, de eltérő liga: két szekció marad.
        let groups = try groups([
            matchJSON(id: "00000000-0000-0000-0000-000000000033",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000f3",
                      competitionName: "Korábbi Név", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000034",
                      kickoff: "2026-11-11T16:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000f3",
                      competitionName: "Későbbi Név", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
        ])
        XCTAssertEqual(groups.count, 2)
        XCTAssertEqual(groups.map(\.name), ["Korábbi Név", "Későbbi Név"])
    }

    // MARK: - Liga-fejléc név és zászló

    func testLeagueHeaderUsesCatalogFlagAndHungarianCountryName() throws {
        let groups = try groups([
            matchJSON(id: "00000000-0000-0000-0000-000000000041",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000d1",
                      competitionName: "Liga 1", countryCode: "IDN", countryName: "Indonesia",
                      priority: 20),
            matchJSON(id: "00000000-0000-0000-0000-000000000042",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000d2",
                      competitionName: "UAE Pro League", countryCode: "UAE",
                      countryName: "United Arab Emirates", priority: 30),
        ])
        XCTAssertEqual(groups[0].flag, "🇮🇩")
        XCTAssertEqual(groups[0].country, "Indonézia")
        XCTAssertEqual(groups[1].flag, "🇦🇪")
        XCTAssertEqual(groups[1].country, "Arab Emírségek")
        XCTAssertEqual(groups[1].name, "UAE Pro League")
    }

    func testLeagueNameFallsBackWhenBlankOrOnlyTheCountryCode() throws {
        let groups = try groups([
            matchJSON(id: "00000000-0000-0000-0000-000000000051",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000e1",
                      competitionName: "   ", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000052",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000e2",
                      competitionName: " hun ", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000053",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000e3",
                      competitionName: nil, countryCode: nil, countryName: nil,
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000054",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000e4",
                      competitionName: "  Bajnokok Ligája  ", countryCode: nil,
                      countryName: nil, priority: 6),
        ])
        // A két használhatatlan nevű magyar sor ugyanabba a szekcióba olvad,
        // a fejléc a magyar országnevet kapja.
        XCTAssertEqual(groups.map(\.name),
                       ["Bajnokok Ligája", "Nemzetközi", "Magyarország"])
    }

    // MARK: - Csoporton belüli sorrend

    func testGroupMatchesSortByKickoffThenIdentifier() throws {
        let groups = try groups([
            matchJSON(id: "00000000-0000-0000-0000-000000000063",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000a9",
                      competitionName: "NB I", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000062",
                      kickoff: "2026-11-11T18:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000a9",
                      competitionName: "NB I", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
            matchJSON(id: "00000000-0000-0000-0000-000000000061",
                      kickoff: "2026-11-11T14:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000a9",
                      competitionName: "NB I", countryCode: "HUN", countryName: "Hungary",
                      priority: 6),
        ])
        XCTAssertEqual(groups.count, 1)
        XCTAssertEqual(groups[0].matches.map(\.id), [
            "00000000-0000-0000-0000-000000000061",
            "00000000-0000-0000-0000-000000000062",
            "00000000-0000-0000-0000-000000000063",
        ])
    }

    func testGroupMetadataComesFromTheEarliestMatch() throws {
        // Két entitás, de ugyanaz a kanonikus liga: egy szekció, és a
        // csoport a legjobb tier/priority értékeket örökli.
        let groups = try groups([
            matchJSON(id: "00000000-0000-0000-0000-000000000071",
                      kickoff: "2026-11-11T20:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000a8",
                      competitionName: "1. osztály", countryCode: "PL",
                      countryName: "Poland", priority: 30, tier: 2),
            matchJSON(id: "00000000-0000-0000-0000-000000000072",
                      kickoff: "2026-11-11T16:00:00Z",
                      competitionId: "00000000-0000-0000-0000-0000000000a9",
                      competitionName: "I Liga", countryCode: "PL",
                      countryName: "Poland", priority: 20, tier: 1),
        ])
        XCTAssertEqual(groups.count, 1)
        XCTAssertEqual(groups[0].name, "I Liga")
        XCTAssertEqual(groups[0].tier, 1)
        XCTAssertEqual(groups[0].priority, 20)
    }
    func testSectionSubtitleIsOnlyTheCountryAndCarriesTheCompetitionLogo() throws {
        let matches = try decode("""
        {"date":"2026-11-20","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000401","kickoff_at":"2026-11-20T17:00:00Z",
           "status":"scheduled","home":"Ath Bilbao","away":"Espanol",
           "home_score":null,"away_score":null,"publish_state":"verified_publish","blockers":[],
           "competition_id":"00000000-0000-0000-0000-0000000000b1","competition_name":"La Liga",
           "country_code":"ES","country_name":"Spain","tier":1,"priority":6,
           "competition_logo":"/v1/asset/assets/aa/bb/cccc.webp","round_label":"17"}
        ]}
        """)
        let groups = LeagueGroup.fromDay(matches)
        XCTAssertEqual(groups.count, 1)
        // A hamis „1. osztály" helyett csak az ország marad a fejlécben.
        XCTAssertEqual(groups[0].subtitle, "SPANYOLORSZÁG")
        XCTAssertEqual(groups[0].logo, "/v1/asset/assets/aa/bb/cccc.webp")
        XCTAssertEqual(groups[0].matches[0].roundText, "17. forduló")
        XCTAssertEqual(groups[0].roundSummary, "17. forduló")
    }

    // MARK: - Forduló a szekciófejlécben

    func testRoundSummaryAppearsOnceForUniformRound() throws {
        let leagueId = "00000000-0000-0000-0000-0000000000d1"
        let groups = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d11",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "13"),
            // Trimmelt, azonos forduló: a foldolás után ugyanaz.
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d12",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: " 13 "),
            // Az ismeretlen címke nem rontja el az egyértelmű fordulót.
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d13",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: nil),
        ])
        XCTAssertEqual(groups.count, 1)
        XCTAssertEqual(groups[0].roundSummary, "13. forduló")
        // A kártya saját forduló-felirata megmarad (az accessibility labelhez).
        XCTAssertEqual(groups[0].matches[0].roundText, "13. forduló")
        XCTAssertEqual(groups[0].matches[2].roundText, nil)
    }

    func testRoundSummaryPrefersNumericRoundWhenMixedWithOtherLabels() throws {
        let leagueId = "00000000-0000-0000-0000-0000000000d2"
        let groups = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d21",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "13"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d22",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "final"),
            // La Liga esete: a női és a hiányzó címke nem blokkolja a fordulót.
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d23",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "női"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d24",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: nil),
        ])
        XCTAssertEqual(groups.count, 1)
        // A leggyakoribb numerikus címke nyer; a női marker nem forduló.
        XCTAssertEqual(groups[0].roundSummary, "13. forduló")
        XCTAssertEqual(groups[0].matches.map(\.roundText), ["13. forduló", "Döntő", nil, nil])
    }

    /// A tisztán női-marker címke („női", „women", „(W)") nem forduló: a
    /// fejléc akkor sem ír elő semmit, ha nincs numerikus címke.
    func testRoundSummaryTreatsWomenOnlyMarkerAsMissing() throws {
        let leagueId = "00000000-0000-0000-0000-0000000000d7"
        let groups = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d71",
                           competitionId: leagueId, competitionName: "La Liga",
                           countryCode: "ES", roundLabel: "női"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d72",
                           competitionId: leagueId, competitionName: "La Liga",
                           countryCode: "ES", roundLabel: "women"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d73",
                           competitionId: leagueId, competitionName: "La Liga",
                           countryCode: "ES", roundLabel: "(W)"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d74",
                           competitionId: leagueId, competitionName: "La Liga",
                           countryCode: "ES", roundLabel: nil),
        ])
        XCTAssertEqual(groups.count, 1)
        XCTAssertNil(groups[0].roundSummary)
        XCTAssertEqual(groups[0].matches.map(\.roundText), [nil, nil, nil, nil])
    }

    /// A mai (2026-11-22) La Liga-csoport valós alakja: „13" és hiányzó címke.
    func testRoundSummaryShowsRoundWhenOnlyNumericAndMissingLabels() throws {
        let leagueId = "00000000-0000-0000-0000-0000000000d8"
        let groups = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d81",
                           competitionId: leagueId, competitionName: "La Liga",
                           countryCode: "ES", roundLabel: "13"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d82",
                           competitionId: leagueId, competitionName: "La Liga",
                           countryCode: "ES", roundLabel: nil),
        ])
        XCTAssertEqual(groups.count, 1)
        XCTAssertEqual(groups[0].roundSummary, "13. forduló")
    }

    func testRoundSummaryTieBreaksOnSmallestRoundNumber() throws {
        let leagueId = "00000000-0000-0000-0000-0000000000d6"
        let groups = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d61",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "14"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d62",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "13"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d63",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "14"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d64",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "13"),
        ])
        XCTAssertEqual(groups[0].roundSummary, "13. forduló")
    }

    func testRoundSummaryIsNilWithoutRoundLabels() throws {
        let leagueId = "00000000-0000-0000-0000-0000000000d3"
        let groups = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d31",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: nil),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d32",
                           competitionId: leagueId, competitionName: "NB I",
                           countryCode: "HUN", roundLabel: "   "),
        ])
        XCTAssertEqual(groups.count, 1)
        XCTAssertNil(groups[0].roundSummary)
    }

    func testRoundSummaryFoldsCupLabelsAndUsesTheSharedMapping() throws {
        let leagueId = "00000000-0000-0000-0000-0000000000d4"
        let grouped = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d41",
                           competitionId: leagueId, competitionName: "Kupadöntő",
                           countryCode: "HUN", roundLabel: "Final"),
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d42",
                           competitionId: leagueId, competitionName: "Kupadöntő",
                           countryCode: "HUN", roundLabel: "final"),
        ])
        XCTAssertEqual(grouped[0].roundSummary, "Döntő")

        let last16 = try groups([
            roundMatchJSON(id: "00000000-0000-0000-0000-000000000d43",
                           competitionId: "00000000-0000-0000-0000-0000000000d5",
                           competitionName: "Kupa", countryCode: "HUN", roundLabel: "1/8"),
        ])
        XCTAssertEqual(last16[0].roundSummary, "Nyolcaddöntő")
    }

}
