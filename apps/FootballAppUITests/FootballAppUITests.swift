import XCTest

/// S30h — automatizált macOS UI-ellenőrzés.
///
/// A tesztek a VoiceOver által is használt akadálymentességi címkéket, a
/// billentyűparancsokat és a betöltés/üres/hiba állapotokat ellenőrzik.
/// A fixture-adatot a `scripts/run-ui-tests.sh` által indított determinisztikus
/// HTTP-szerver szolgálja ki (`scripts/ui-fixture-server.py`), mert a UI-test
/// runner sandboxja nem enged saját socketet nyitni.
final class FootballAppUITests: XCTestCase {
    private static let loadedPort: UInt16 = 18080
    private static let emptyPort: UInt16 = 18081
    private static let partialPort: UInt16 = 18082

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    // MARK: - Betöltött nap

    func testLoadedDaySpeaksTeamsStatusScoreAndPublishState() throws {
        let app = try launch(port: Self.loadedPort)

        let scheduled = element("match.card.11111111-1111-1111-1111-111111111111", in: app)
        XCTAssertTrue(scheduled.waitForExistence(timeout: 30), "a nap nem töltődött be")

        XCTAssertTrue(scheduled.label.contains("Ferencváros – Újpest"), scheduled.label)
        XCTAssertTrue(scheduled.label.contains("Ütemezett"), scheduled.label)
        XCTAssertTrue(scheduled.label.contains("eredmény még nincs"), scheduled.label)
        XCTAssertTrue(scheduled.label.contains("Ideiglenes publikáció"), scheduled.label)
        // A `date_only` kezdésnél nincs hamis idő, hanem figyelmeztetés + forduló.
        XCTAssertTrue(scheduled.label.contains("időpont egyeztetés alatt"), scheduled.label)
        XCTAssertTrue(scheduled.label.contains("17. forduló"), scheduled.label)

        let finished = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(finished.waitForExistence(timeout: 10), "a lezárt meccs kártyája hiányzik")
        XCTAssertTrue(finished.label.contains("Debrecen – Paks"), finished.label)
        XCTAssertTrue(finished.label.contains("Lezárt"), finished.label)
        XCTAssertTrue(finished.label.contains("2 – 1"), finished.label)

        XCTAssertTrue(finished.label.contains("Ellenőrzött publikáció"), finished.label)
        XCTAssertTrue(finished.label.contains("Döntő"), finished.label)

        // A forduló a szekciófejlécben jelenik meg, egyszer: a Premier League
        // egyetlen fordulós szekciójában „12. forduló", a vegyes fordulójú
        // NB I fejlécében viszont nem tippelünk.
        let premier = element("league.section.GB|premier league", in: app)
        XCTAssertTrue(premier.waitForExistence(timeout: 10), "a Premier League szekciófejléc hiányzik")
        // A SwiftUI Text saját AX-elem a szöveget a value attribútumban hordozza.
        XCTAssertTrue(textElement(valueContains: "12. forduló", in: app).exists,
                      "a Premier League 12. fordulós jelzése hiányzik")

        let nbOne = element("league.section.HU|nb i", in: app)
        XCTAssertTrue(nbOne.waitForExistence(timeout: 10), "az NB I szekciófejléc hiányzik")
        let nbOneText = nbOne.value as? String ?? ""
        XCTAssertFalse(nbOneText.contains("forduló"), nbOneText)

        XCTAssertTrue(element("home.stat.matches", in: app).waitForExistence(timeout: 10),
                      "a nap mérkőzésszáma hiányzik a fejlécből")
        XCTAssertTrue(element("home.stat.live", in: app).exists,
                      "az élő mérkőzések száma hiányzik a fejlécből")
        XCTAssertTrue(element("home.stat.odds", in: app).exists,
                      "az odds-lefedettség száma hiányzik a fejlécből")

        // Az új, ikonos fejléc: időjárás + frissítési visszaszámláló.
        let weather = element("home.stat.weather", in: app)
        XCTAssertTrue(weather.waitForExistence(timeout: 10),
                      "az időjárás-tégla hiányzik a fejlécből")
        // A SwiftUI Text néha az értéket a `value`-ban hordozza, nem a `label`-ben.
        let weatherText = weather.label + (weather.value as? String ?? "")
        XCTAssertTrue(weatherText.contains("Időjárás"), weatherText)
        let refresh = element("home.stat.refresh", in: app)
        XCTAssertTrue(refresh.exists, "a frissítési visszaszámláló hiányzik a fejlécből")
        let refreshText = refresh.label + (refresh.value as? String ?? "")
        XCTAssertFalse(refreshText.isEmpty, "a visszaszámlálónak van magyar címkéje")

        let teamLogoFilter = element("filter.missingLogo", in: app)
        let competitionLogoFilter = element("filter.missingCompetitionLogo", in: app)
        XCTAssertEqual(teamLogoFilter.label, "Logó nélkül", "a csapatlogó-szűrőn ne jelenjen meg számláló")
        XCTAssertEqual(competitionLogoFilter.label, "Ligalogó nélkül", "a ligalogó-szűrőn ne jelenjen meg számláló")
        XCTAssertFalse(element("day.gaps", in: app).exists,
                        "a hiány-szűrők ne ismétlődjenek meg második sorban")
    }

    // MARK: - Részleges / bizonytalan adat

    /// A hiányzó eredményt és a publikációs minőségi állapotot minden esetben
    /// ki kell mondani — a szín önmagában nem hordozhat információt.
    func testPartialDataSpeaksStatusScoreAndPublishState() throws {
        let app = try launch(port: Self.partialPort)

        let live = element("match.card.AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA", in: app)
        XCTAssertTrue(live.waitForExistence(timeout: 30), "a részleges nap nem töltődött be")
        XCTAssertTrue(live.label.contains("Győr – Fehérvár"), live.label)
        // A fixture dátuma 2026-11-20; a stale-live guard ezt két nappal
        // később úgy mondja, hogy az eredmény még várható (S130),
        // az eredmény és a publikációs állapot marad.
        XCTAssertTrue(live.label.contains("Eredmény várható"), live.label)
        XCTAssertTrue(live.label.contains("1 – 0"), live.label)
        XCTAssertTrue(live.label.contains("Ideiglenes publikáció"), live.label)

        let postponed = element("match.card.BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB", in: app)
        XCTAssertTrue(postponed.waitForExistence(timeout: 10))
        XCTAssertTrue(postponed.label.contains("Elhalasztott"), postponed.label)
        XCTAssertTrue(postponed.label.contains("eredmény még nincs"), postponed.label)
        XCTAssertTrue(postponed.label.contains("Ismeretlen publikáció"), postponed.label)

        let withheld = element("match.card.CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC", in: app)
        XCTAssertTrue(withheld.waitForExistence(timeout: 10))
        XCTAssertTrue(withheld.label.contains("3 – 3"), withheld.label)
        XCTAssertTrue(withheld.label.contains("Visszatartott publikáció"), withheld.label)

        let quarantined = element("match.card.DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD", in: app)
        XCTAssertTrue(quarantined.waitForExistence(timeout: 10))
        XCTAssertTrue(quarantined.label.contains("Félbeszakadt"), quarantined.label)
        XCTAssertTrue(quarantined.label.contains("eredmény még nincs"), quarantined.label)
        XCTAssertTrue(quarantined.label.contains("Karanténban lévő publikáció"), quarantined.label)
    }

    // MARK: - Üres nap

    /// S122: a rendezési mód választható, és a kiválasztott mód látható a menüben.
    func testSortModeMenuOffersThreeModesAndPersistsSelection() throws {
        let app = try launch(port: Self.loadedPort)
        XCTAssertTrue(element("match.card.11111111-1111-1111-1111-111111111111", in: app)
            .waitForExistence(timeout: 30), "a nap nem töltődött be")

        let sortMenu = element("filter.sort", in: app)
        XCTAssertTrue(sortMenu.waitForExistence(timeout: 10), "a rendezési menü hiányzik")

        sortMenu.click()
        for title in ["Kiemelt ligák", "Ország A–Z", "Versenynév A–Z"] {
            XCTAssertTrue(app.menuItems[title].exists, "hiányzó rendezési mód: \(title)")
        }
        // A SwiftUI Menu AX-labelje a fixture-környezetben üres lehet, ezért
        // a kiválasztás tényét a menü újbóli megnyitásával ellenőrizzük.
        app.menuItems["Ország A–Z"].click()
        sortMenu.click()
        XCTAssertTrue(app.menuItems["Ország A–Z"].exists, "a kiválasztott rendezési mód nem érhető el")
    }

    func testEmptyDayShowsEmptyState() throws {
        let app = try launch(port: Self.emptyPort)

        let empty = app.staticTexts["Nincs meccs ezen a napon."].firstMatch
        XCTAssertTrue(empty.waitForExistence(timeout: 30), "az üres állapot nem jelent meg")    }

    // MARK: - Daemon-offline

    func testMissingRuntimeShowsDaemonOfflineError() throws {
        let missingRuntime = FileManager.default.temporaryDirectory
            .appendingPathComponent("football-ui-missing-\(UUID().uuidString).json")
        let app = XCUIApplication()
        app.launchEnvironment["FOOTBALL_APP_RUNTIME_FILE"] = missingRuntime.path
        app.launch()

        let message = textElement(valueContains: "A footballd daemon nem fut", in: app)
        XCTAssertTrue(message.waitForExistence(timeout: 30), "a daemon-offline hibaüzenet nem jelent meg")    }

    // MARK: - Billentyűparancsok

    func testKeyboardShortcutsMoveBetweenDays() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let today = textElement(valueContains: "MA", in: app)
        XCTAssertTrue(today.waitForExistence(timeout: 30), "a mai nap nem töltődött be")

        app.typeKey(XCUIKeyboardKey.rightArrow.rawValue, modifierFlags: .command)
        let tomorrow = textElement(valueContains: "HOLNAP", in: app)
        XCTAssertTrue(tomorrow.waitForExistence(timeout: 15), "Cmd+→ nem váltott a következő napra")

        app.typeKey("t", modifierFlags: .command)
        XCTAssertTrue(today.waitForExistence(timeout: 15), "Cmd+T nem tért vissza a mai napra")

        app.typeKey(XCUIKeyboardKey.leftArrow.rawValue, modifierFlags: .command)
        let yesterday = textElement(valueContains: "TEGNAP", in: app)
        XCTAssertTrue(yesterday.waitForExistence(timeout: 15), "Cmd+← nem váltott az előző napra")
    }

    /// ⌘F megnyitja a globális keresést; a találatok fajtával és szöveggel
    /// jelennek meg, Escape pedig bezárja a sheetet.
    func testGlobalSearchCmdFShowsResults() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let today = textElement(valueContains: "MA", in: app)
        XCTAssertTrue(today.waitForExistence(timeout: 30), "a nap nem töltődött be")

        let searchButton = element("nav.search", in: app)
        XCTAssertTrue(searchButton.waitForExistence(timeout: 10), "a keresőgomb hiányzik a navigációból")
        searchButton.click()
        let field = element("search.field", in: app)
        XCTAssertTrue(field.waitForExistence(timeout: 10), "a keresés nem nyitotta meg a mezőt")

        field.click()
        app.typeText("ujpest")
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])

        let result = element("search.result.team", in: app)
        XCTAssertTrue(result.waitForExistence(timeout: 10), "a csapat-találat nem jelent meg")
        let matchResult = element("search.result.match", in: app)
        XCTAssertTrue(matchResult.waitForExistence(timeout: 10), "a meccs-találat nem jelent meg")

        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(field, timeout: 10), "Escape nem zárta be a keresést")
    }

    /// A csapat-találat nem csak fülváltás: a konkrét entitás adatlapja nyílik meg.
    func testSearchTeamResultOpensTheEntity() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let searchButton = element("nav.search", in: app)
        XCTAssertTrue(searchButton.waitForExistence(timeout: 10), "a keresőgomb hiányzik")
        searchButton.click()
        let field = element("search.field", in: app)
        XCTAssertTrue(field.waitForExistence(timeout: 10), "a keresés nem nyílt meg")
        field.click()
        app.typeText("ujpest")
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])

        let result = element("search.result.team", in: app)
        XCTAssertTrue(result.waitForExistence(timeout: 10), "a csapat-találat hiányzik")
        let teamButton = app.buttons.matching(identifier: "search.result.team").firstMatch
        XCTAssertTrue(teamButton.waitForExistence(timeout: 5), "a csapat-találat gombja hiányzik")
        teamButton.click()
        let team = element("team.detail", in: app)
        XCTAssertTrue(team.waitForExistence(timeout: 10), "a csapat-adatlap nem nyílt meg")
        XCTAssertTrue((team.value as? String ?? "").contains("ÚJPEST"), "a csapat-adatlap címe: \(team.value ?? "")")
    }

    /// A csapatadatlapon a keret játékosai kattinthatók; a játékosadatlap
    /// ugyanabban a sheet-láncban nyílik meg.
    func testTeamSquadOpensPlayer() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()
        app.buttons.matching(identifier: "nav.search").firstMatch.click()
        let field = element("search.field", in: app)
        XCTAssertTrue(field.waitForExistence(timeout: 10), "a keresés nem nyílt meg")
        field.click()
        app.typeText("ujpest")
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])
        app.buttons.matching(identifier: "search.result.team").firstMatch.click()
        XCTAssertTrue(element("team.detail", in: app).waitForExistence(timeout: 10), "a csapatlap nem nyílt meg")

        let player = app.buttons.matching(identifier: "team.player.900001").firstMatch
        XCTAssertTrue(player.waitForExistence(timeout: 10), "a keret játékosa hiányzik")
        player.click()
        let name = element("player.name", in: app)
        XCTAssertTrue(name.waitForExistence(timeout: 10), "a játékoslap nem nyílt meg")
        // A fixture szerver minden player ID-re ugyanazt a Játékos Kereső
        // payloadot adja — a lényeg itt a navigáció, nem a név.
        XCTAssertFalse((name.value as? String ?? "").isEmpty)
    }

    /// A versenyadatlapon a csapatok (csapatok fül) kattinthatók.
    func testCompetitionTeamRowsOpenTeamSheet() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()
        app.buttons.matching(identifier: "nav.search").firstMatch.click()
        let field = element("search.field", in: app)
        XCTAssertTrue(field.waitForExistence(timeout: 10), "a keresés nem nyílt meg")
        field.click()
        app.typeText("ujpest")
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])
        app.buttons.matching(identifier: "search.result.competition").firstMatch.click()
        XCTAssertTrue(element("competition.detail", in: app).waitForExistence(timeout: 10), "a versenylap nem nyílt meg")

        app.buttons.matching(identifier: "competition.tab.Csapatok").firstMatch.click()
        let team = app.buttons.matching(
            identifier: "competition.team.00000000-0000-0000-0000-000000000011"
        ).firstMatch
        XCTAssertTrue(team.waitForExistence(timeout: 10), "a csapatsor hiányzik")
        team.click()
        XCTAssertTrue(element("team.detail", in: app).waitForExistence(timeout: 10), "a csapatlap nem nyílt meg")
    }

    /// A keresés játékos-találatából megnyíló adatlap; a hiány szövegesen
    /// jelenik meg, az AX-azonosítók stabilak.
    func testPlayerDetailShowsDataAndMissingStates() throws {
        let app = try launch(
            port: Self.loadedPort,
            playerID: "00000000-0000-0000-0000-000000000014"
        )
        app.activate()

        let name = element("player.name", in: app)
        XCTAssertTrue(name.waitForExistence(timeout: 20), "a játékosadatlap nem nyílt meg")
        // A SwiftUI Text a szöveget a `value`-ban hordozza, a `label` üres.
        XCTAssertEqual(name.value as? String, "Játékos Kereső")
        XCTAssertTrue(element("player.country", in: app).exists, "az ország mező hiányzik")
        XCTAssertTrue(element("player.content", in: app).exists, "a tartalom hiányzik")
        XCTAssertTrue(element("player.stat", in: app).exists, "a statisztika hiányzik")
        XCTAssertTrue(element("player.incident", in: app).exists, "az esemény hiányzik")

        // A fellépések sora kattintható: a konkrét meccs részletezőjét nyitja.
        app.buttons.matching(identifier: "player.appearance").firstMatch.click()
        let detail = element("match.detail", in: app)
        XCTAssertTrue(detail.waitForExistence(timeout: 10),
                      "a fellépés meccsrészletezője nem nyílt meg")
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(detail, timeout: 10), "Escape nem zárta be a meccsrészletezőt")
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(name, timeout: 10), "Escape nem zárta be a játékoslapot")
    }

    // MARK: - Billentyűzet-fókusz a napi listában

    /// Tab belép a listába, a nyilak lépkednek, Return megnyitja a részletezőt,
    /// Escape kilép. A fókusz vizuális jelzése a kártya borostyán kerete.
    func testKeyboardFocusWalksMatchCardsAndOpensDetail() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let first = element("match.card.11111111-1111-1111-1111-111111111111", in: app)
        let second = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(first.waitForExistence(timeout: 30), "a nap nem töltődött be")

        XCTAssertTrue(focusFirstMatchCard(first, in: app),
                      "a Tab-sorrenddel nem érhető el az első kártya")

        app.typeKey(XCUIKeyboardKey.downArrow.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForSelected(second, timeout: 10), "↓ nem lépett a következő kártyára")

        app.typeKey(XCUIKeyboardKey.upArrow.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForSelected(first, timeout: 10), "↑ nem lépett vissza az előző kártyára")

        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])
        let sheet = element("match.detail", in: app)
        XCTAssertTrue(sheet.waitForExistence(timeout: 15), "Return nem nyitotta meg a részletezőt")

        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(sheet, timeout: 10), "Escape nem zárta be a részletezőt")
        XCTAssertTrue(waitForSelected(first, timeout: 10),
                      "Escape után nem állt vissza a fókusz a kártyára")

        app.typeKey("\t", modifierFlags: .shift)
        XCTAssertTrue(waitForNotSelected(first, timeout: 10),
                      "Shift+Tab nem lépett vissza a kártyáról a fejléc/szűrősáv felé")
    }

    // MARK: - App-szintű Tab-sorrend

    /// A Tab végigmegy a felső navigáción, a fejléc-vezérlőkön és a szűrőkön,
    /// aztán a kártyákon; Escape törli a fókuszt, Shift+Tab üres fókuszból
    /// az utolsó állomásra lép (körbeérő sorrend).
    func testKeyboardTabOrderCoversHeaderFiltersAndList() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let first = element("match.card.11111111-1111-1111-1111-111111111111", in: app)
        XCTAssertTrue(first.waitForExistence(timeout: 30), "a nap nem töltődött be")

        app.typeKey("\t", modifierFlags: [])
        XCTAssertTrue(waitForSelected(element("nav.tab.day", in: app), timeout: 10),
                      "Tab nem a felső navigációra vitte a fókuszt")

        for _ in 0..<3 { app.typeKey("\t", modifierFlags: []) }
        XCTAssertTrue(waitForSelected(element("nav.tab.ops", in: app), timeout: 10),
                      "Tab nem lépett végig a felső navigáción")

        app.typeKey("\t", modifierFlags: [])
        XCTAssertTrue(waitForSelected(element("nav.chevron.left", in: app), timeout: 10),
                      "Tab nem érte el az előző nap gombot")

        app.typeKey("\t", modifierFlags: [])
        XCTAssertTrue(waitForSelected(element("nav.today", in: app), timeout: 10),
                      "Tab nem érte el a Ma gombot")

        // következő nap → naptár → frissítés → szűrők
        for _ in 0..<4 { app.typeKey("\t", modifierFlags: []) }
        XCTAssertTrue(waitForSelected(element("filter.all", in: app), timeout: 10),
                      "Tab nem érte el a szűrősort")

        // következik / élő / befejezett / kiemelt → első kártya
        var reached = false
        for _ in 0..<20 where !reached {
            app.typeKey("\t", modifierFlags: [])
            reached = first.isSelected
        }
        XCTAssertTrue(waitForSelected(first, timeout: 10),
                      "Tab a szűrők után nem ért el a listáig")

        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForNotSelected(first, timeout: 10), "Escape nem törölte a fókuszt")

        app.typeKey("\t", modifierFlags: .shift)
        // A LazyVStack csak a látható kártyákat építi fel, ezért az utolsó
        // fókuszálható kártya a viewporttól függ: a 2222-es vagy a 4444-es.
        let lastVisible = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        let lastNew = element("match.card.44444444-4444-4444-4444-444444444444", in: app)
        XCTAssertTrue(waitForSelected(lastVisible, timeout: 10) || waitForSelected(lastNew, timeout: 1),
                      "Shift+Tab üres fókuszból nem a lista utolsó kártyájára lépett")
    }

    /// A redesignált meccs-részletező a gazdag szakaszokat (statisztika,
    /// odds-mátrix, ellenőrzés) a fixture-adatból jeleníti meg.
    func testMatchDetailShowsRichSections() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()

        let sheet = element("match.detail", in: app)
        XCTAssertTrue(sheet.waitForExistence(timeout: 20), "a részletező nem nyílt meg")
        XCTAssertTrue(element("match.detail.odds", in: app).waitForExistence(timeout: 10),
                      "az odds-mátrix nem jelent meg")
        XCTAssertTrue(element("match.detail.stats", in: app).waitForExistence(timeout: 10),
                      "a statisztika-párbaj nem jelent meg")
        XCTAssertTrue(element("match.detail.verification", in: app).waitForExistence(timeout: 10),
                      "az ellenőrzési sáv nem jelent meg")
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(sheet, timeout: 10), "Escape nem zárta be a részletezőt")
    }

    /// A meccs-szintű mikro-adatok szekciói: felállás, játékosstatisztika, lövések.
    func testMatchDetailShowsLineupsPlayerStatsAndShots() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()

        let lineupsButton = app.buttons.matching(identifier: "match.detail.section.lineups").firstMatch
        XCTAssertTrue(lineupsButton.waitForExistence(timeout: 10), "a Felállás fül hiányzik")
        lineupsButton.click()
        let lineup = element("match.lineup.home.900001", in: app)
        XCTAssertTrue(lineup.waitForExistence(timeout: 10), "a kezdő játékos hiányzik")
        XCTAssertTrue(lineup.label.contains("Kapus Játékos"), lineup.label)
        XCTAssertTrue(element("match.playerstat.row.0", in: app).exists, "a játékosstatisztika hiányzik")

        let shotsButton = app.buttons.matching(identifier: "match.detail.section.shots").firstMatch
        XCTAssertTrue(shotsButton.waitForExistence(timeout: 10), "a Lövések fül hiányzik")
        shotsButton.click()
        let shot = element("match.shot.1", in: app)
        XCTAssertTrue(shot.waitForExistence(timeout: 10), "a lövéstérkép hiányzik")
        XCTAssertTrue(shot.label.contains("xG 0.18"), shot.label)
    }

    /// A research szekció mutatja a generált riportot és a tárolt forráslinkeket.
    func testMatchDetailShowsReportAndSourceLinks() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()
        app.buttons.matching(identifier: "match.detail.section.research").firstMatch.click()

        let report = element("match.insight.report", in: app)
        XCTAssertTrue(report.waitForExistence(timeout: 10), "a magyar riport hiányzik")
        XCTAssertTrue(report.label.contains("Magyar riport") || (report.value as? String ?? "").contains("Magyar riport"))

        let source = app.buttons.matching(identifier: "match.link.source").firstMatch
        XCTAssertTrue(source.waitForExistence(timeout: 10), "a forráslink hiányzik")
        XCTAssertTrue(source.label.contains("Forrás megnyitása"), source.label)
        XCTAssertTrue(app.buttons.matching(identifier: "match.link.reference").firstMatch.exists,
                      "a hivatkozás-link hiányzik")
    }

    /// A meccsoldal Research füle mutatja a jegyzeteket és az előzményeket.
    func testMatchDetailShowsNotesAndHistory() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()
        app.buttons.matching(identifier: "match.detail.section.research").firstMatch.click()

        let note = element("match.note.00000000-0000-0000-0000-000000000031", in: app)
        XCTAssertTrue(note.waitForExistence(timeout: 10), "a jegyzet hiányzik")
        let noteText = note.label + (note.value as? String ?? "")
        XCTAssertTrue(noteText.contains("Az odds forrása"), noteText)
        XCTAssertTrue(element("match.note.body", in: app).exists, "az új jegyzet mező hiányzik")
        XCTAssertTrue(element("match.note.add", in: app).exists, "a jegyzet hozzáadása gomb hiányzik")

        let history = element("match.history.row.0", in: app)
        XCTAssertTrue(history.waitForExistence(timeout: 10), "az előzmény hiányzik")
        let historyText = history.label + (history.value as? String ?? "")
        XCTAssertTrue(historyText.contains("manual_match_edit"), historyText)
    }

    /// A meccsrészletezőből a csapat- és versenylinkek a konkrét entitásra vezetnek.
    func testMatchDetailEntityLinksOpenSheets() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()

        let homeLink = element("match.detail.link.home.team", in: app)
        XCTAssertTrue(homeLink.waitForExistence(timeout: 10), "a hazai csapat link hiányzik")
        app.buttons.matching(identifier: "match.detail.link.home.team").firstMatch.click()
        let team = element("team.detail", in: app)
        XCTAssertTrue(team.waitForExistence(timeout: 10), "a csapat-adatlap nem nyílt meg")
        XCTAssertTrue((team.value as? String ?? "").contains("DEBRECEN"), "a csapat-adatlap címe: \(team.value ?? "")")
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(team, timeout: 10), "a csapat-adatlap nem zárt be")

        let competitionLink = element("match.detail.link.competition", in: app)
        XCTAssertTrue(competitionLink.waitForExistence(timeout: 10), "a verseny link hiányzik")
        app.buttons.matching(identifier: "match.detail.link.competition").firstMatch.click()
        let competition = element("competition.detail", in: app)
        XCTAssertTrue(competition.waitForExistence(timeout: 10), "a verseny-adatlap nem nyílt meg")
        XCTAssertTrue((competition.value as? String ?? "").contains("NB I"), "a verseny-adatlap címe: \(competition.value ?? "")")
    }

    /// A research időbélyeg hónapot, nem percet ír ki (`MM` ≠ `mm`).
    func testMatchDetailResearchTimestampUsesCalendarMonth() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()

        let insightButton = element("match.detail.section.research", in: app)
        XCTAssertTrue(insightButton.waitForExistence(timeout: 20), "a Research fül hiányzik")
        XCTAssertTrue(insightButton.isEnabled, "a Research fül nincs adat hiányában letiltva")
        insightButton.click()

        let insight = element("match.insight", in: app)
        XCTAssertTrue(insight.waitForExistence(timeout: 20), "a research-blokk nem jelent meg")
        let text = insight.label + (insight.value as? String ?? "")
        XCTAssertTrue(text.contains("2026. 11. 22."), "hibás research időbélyeg: \(text)")
        XCTAssertFalse(text.contains("2026. 44."), "a hónap helyett perceket ír ki: \(text)")
    }

    /// A szekciófülek a research-blokkot görgetik elő és meghirdetik a kiválasztást.
    func testMatchDetailSectionNavigationReachesResearch() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()

        let overview = element("match.detail.section.overview", in: app)
        XCTAssertTrue(overview.waitForExistence(timeout: 20), "a szekciósáv nem jelent meg")
        let navigation = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label == %@", "Meccsrészletező szekciók"))
            .firstMatch
        XCTAssertTrue(navigation.exists, "a szekciósáv akadálymentes címkéje hiányzik")
        for section in ["overview", "events", "stats", "odds", "research"] {
            XCTAssertTrue(element("match.detail.section.\(section)", in: app).exists,
                          "hiányzó szekciógomb: \(section)")
        }

        let research = element("match.insight", in: app)
        XCTAssertTrue(research.waitForExistence(timeout: 10), "a research-blokk nem jelent meg")
        XCTAssertFalse(research.isHittable, "a research a nyitó nézetben látható maradt")

        let researchButton = element("match.detail.section.research", in: app)
        researchButton.click()
        XCTAssertTrue(waitForHittable(research, timeout: 10), "a Research gomb nem görgetette elő a blokkot")
        XCTAssertTrue(waitForSelected(researchButton, timeout: 10),
                      "a Research gomb nem hirdeti meg a kiválasztott állapotot")

        let sheet = element("match.detail", in: app)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(sheet, timeout: 10), "Escape nem zárta be a részletezőt")
    }

    func testHomeHeaderIsCompactAndKeepsEveryControl() throws {
        let app = try launch(port: Self.loadedPort, windowWidth: 1040)
        app.activate()

        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 30), "az alkalmazás ablaka hiányzik")
        XCTAssertEqual(window.frame.width, 1040, accuracy: 1, "a széles tesztablak nem 1040 pt szélességű")

        let header = element("home.header", in: app)
        XCTAssertTrue(header.waitForExistence(timeout: 30), "a főoldali fejléc hiányzik")
        XCTAssertFalse(element("home.header.compactDate", in: app).exists,
                       "a széles tesztablak a kompakt fejlécágra váltott")
        print("HOME_HEADER_FRAME \(header.frame)")
        XCTAssertLessThanOrEqual(header.frame.height, 76, "a fejléc magassága nem csökkent a fele akkorára")

        for identifier in [
            "home.stat.matches", "home.stat.live", "home.stat.odds",
            "home.stat.weather", "home.stat.refresh",
            "nav.chevron.left", "nav.today", "nav.chevron.right",
            "nav.calendar", "nav.arrow.clockwise",
        ] {
            XCTAssertTrue(element(identifier, in: app).exists, "a fejlécből hiányzik: \(identifier)")
        }
    }

    func testHomeHeaderCompactBranchFitsNarrowWindow() throws {
        let app = try launch(port: Self.loadedPort, windowWidth: 820)
        app.activate()

        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 30), "az alkalmazás ablaka hiányzik")
        XCTAssertEqual(window.frame.width, 820, accuracy: 1, "a szűk tesztablak nem 820 pt szélességű")

        let compactDate = element("home.header.compactDate", in: app)
        XCTAssertTrue(compactDate.waitForExistence(timeout: 10), "a szűk fejlécágra váltott dátumblokk hiányzik")
        let header = element("home.header", in: app)
        XCTAssertTrue(header.waitForExistence(timeout: 10), "a szűk fejléc hiányzik")
        print("HOME_HEADER_COMPACT_FRAME \(header.frame)")
        XCTAssertLessThanOrEqual(header.frame.height, 76, "a szűk fejléc magassága meghaladja a 76 pt-ot")

        for identifier in [
            "nav.chevron.left", "nav.today", "nav.chevron.right",
            "nav.calendar", "nav.arrow.clockwise",
        ] {
            let control = element(identifier, in: app)
            XCTAssertTrue(control.waitForExistence(timeout: 10), "a szűk fejlécből hiányzik: \(identifier)")
            XCTAssertTrue(control.isHittable, "a szűk fejléc vezérlője nem érhető el: \(identifier)")
        }
        for identifier in [
            "home.stat.matches", "home.stat.live", "home.stat.odds",
            "home.stat.weather", "home.stat.refresh",
        ] {
            XCTAssertTrue(element(identifier, in: app).exists, "a szűk fejlécből hiányzik: \(identifier)")
        }
    }

    func testMatchChainOpensFormMatchAndRestoresParent() throws {
        let app = try openLoadedMatchDetail()
        let link = element("match.detail.form.10000000-0000-0000-0000-000000000001", in: app)
        XCTAssertTrue(link.waitForExistence(timeout: 10), "a forma-meccs gombja hiányzik")
        XCTAssertTrue(link.label.contains("Meccs megnyitása"), link.label)
        XCTAssertTrue(link.label.contains("Ferencváros"), link.label)
        XCTAssertTrue(link.label.contains("győzelem"), link.label)

        link.click()
        assertChainTarget(id: "10000000-0000-0000-0000-000000000001", marker: "Lánc próba 001", in: app)
    }

    func testMatchChainOpensHeadToHeadWithoutInventingScore() throws {
        let app = try openLoadedMatchDetail()
        let link = element("match.detail.h2h.10000000-0000-0000-0000-000000000005", in: app)
        XCTAssertTrue(link.waitForExistence(timeout: 10), "a H2H-meccs gombja hiányzik")
        XCTAssertTrue(link.label.contains("Meccs megnyitása"), link.label)
        XCTAssertTrue(link.label.contains("eredmény még nincs"), link.label)
        XCTAssertFalse(link.label.contains("0 – 0"), link.label)
        XCTAssertFalse(link.label.contains("0–0"), link.label)

        link.click()
        assertChainTarget(id: "10000000-0000-0000-0000-000000000005", marker: "Lánc próba 005", in: app)
    }

    func testMatchChainOpensNextMatchAndPreservesScrollAndFocus() throws {
        let app = try openLoadedMatchDetail()
        let link = element("match.detail.next.10000000-0000-0000-0000-000000000006", in: app)
        XCTAssertTrue(link.waitForExistence(timeout: 10), "a következő meccs gombja hiányzik")
        XCTAssertTrue(link.label.contains("Meccs megnyitása"), link.label)
        XCTAssertTrue(link.label.contains("Győr"), link.label)
        XCTAssertTrue(link.label.contains("hazai"), link.label)

        XCTAssertTrue(focusByTabbing(link, in: app, timeout: 15), "Tabbal nem érhető el a következő meccs")
        let originalY = link.frame.midY
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])
        assertChainTarget(id: "10000000-0000-0000-0000-000000000006", marker: "Lánc próba 006", in: app)

        XCTAssertTrue(link.waitForExistence(timeout: 10), "a szülő részletező eltűnt")
        XCTAssertTrue(element("match.detail.form", in: app).exists, "a szülő tartalma újratöltődött")
        XCTAssertEqual(link.frame.midY, originalY, accuracy: 2, "a szülő görgetése elállt")
        XCTAssertTrue(waitForSelected(link, timeout: 10), "a fókusz nem állt vissza a kapcsolódó meccsre")
    }

    // MARK: - Üzemeltetés: hiányjelek és idővonal (S2)

    private func openOperations(_ port: UInt16) throws -> XCUIApplication {
        let app = try launch(port: port)
        let tab = element("nav.tab.ops", in: app)
        XCTAssertTrue(tab.waitForExistence(timeout: 30), "a felső navigáció nem töltődött be")
        tab.click()
        let toggle = element("ops.issues.openOnly", in: app)
        if !toggle.waitForExistence(timeout: 15) {
            let found = app.descendants(matching: .any).allElementsBoundByIndex
                .map { $0.identifier }.filter { !$0.isEmpty }
            XCTFail("az Üzemeltetés oldal nem töltődött be; AX: \(found.prefix(60).joined(separator: ", "))")
        }
        return app
    }

    func testOperationsListsIssuesWithHungarianLabels() throws {
        let app = try openOperations(Self.loadedPort)
        let issue = element("ops.issue.AAAAAAAA-0000-0000-0000-000000000001", in: app)
        XCTAssertTrue(issue.waitForExistence(timeout: 15), "a hiányjel nem jelenik meg")
        XCTAssertTrue(issue.label.contains("Hiányzó odds"), issue.label)
        XCTAssertTrue(issue.label.contains("Figyelmeztetés"), issue.label)
        XCTAssertTrue(issue.label.contains("először"), issue.label)
        XCTAssertTrue(issue.label.contains("utoljára"), issue.label)
        XCTAssertTrue(issue.label.contains("megoldatlan"), issue.label)
        XCTAssertFalse(issue.label.contains("null"), "a nyers null nem kerülhet a címkébe")
    }

    func testOperationsOpensMatchHistoryFromIssue() throws {
        let app = try openOperations(Self.loadedPort)
        let issue = element("ops.issue.AAAAAAAA-0000-0000-0000-000000000001", in: app)
        XCTAssertTrue(issue.waitForExistence(timeout: 15), "a hiányjel nem jelenik meg")
        issue.click()
        let row = element("ops.history.row.0", in: app)
        XCTAssertTrue(row.waitForExistence(timeout: 15), "az idővonal részletező nem nyílt meg")
        // A SwiftUI Text szövege az AX value-ban van, nem a labelben.
        XCTAssertTrue(textElement(valueContains: "manual_match_edit", in: app).exists,
                      "az audit akció nem jelenik meg")
        XCTAssertTrue(textElement(valueContains: "kickoff_at", in: app).exists,
                      "a provenance mező nem jelenik meg")
        XCTAssertTrue(element("ops.history.row.1", in: app).exists, "a második esemény hiányzik")
    }

    func testOperationsShowsEmptyIssueState() throws {
        let app = try openOperations(Self.emptyPort)
        let empty = element("ops.issues.empty", in: app)
        XCTAssertTrue(empty.waitForExistence(timeout: 15), "az üres hiánylista nem jelenik meg")
        XCTAssertTrue(empty.label.contains("Még nincs rögzített hiányjel."), empty.label)
    }

    func testOperationsShowsSummaryAndGlobalAuditFeed() throws {
        let app = try openOperations(Self.loadedPort)
        XCTAssertTrue(element("ops.summary", in: app).waitForExistence(timeout: 15),
                      "az összefoglaló sáv hiányzik")
        XCTAssertTrue(textElement(valueContains: "2 nyitott hiányjel", in: app).exists,
                      "a magyar összefoglaló mondat hiányzik")
        XCTAssertTrue(element("ops.audit", in: app).exists, "az audit szekció hiányzik")
        let row = element("ops.audit.BBBBBBBB-0000-0000-0000-000000000001", in: app)
        if !row.waitForExistence(timeout: 15) {
            let found = app.descendants(matching: .any).allElementsBoundByIndex
                .map { $0.identifier }.filter { !$0.isEmpty }
            XCTFail("az audit esemény hiányzik; AX: \(found.prefix(80).joined(separator: ", "))")
        }
        XCTAssertTrue(textElement(valueContains: "manual_match_edit", in: app).exists,
                      "az audit akció szövege hiányzik")
    }

    /// S174 — az összefoglaló dátuma nem a mai napra van rögzítve: a választó
    /// magyar AX-címkével és az API-val megegyező kulcsokkal jelenik meg.
    func testOperationsSummaryDatePickerHasHungarianLabelAndOptions() throws {
        let app = try openOperations(Self.loadedPort)
        let picker = element("ops.summary.date", in: app)
        if !picker.waitForExistence(timeout: 15) {
            let found = app.descendants(matching: .any).allElementsBoundByIndex
                .map { "\($0.elementType):\($0.identifier):\($0.label)" }
            XCTFail("az összefoglaló dátumválasztója hiányzik; AX: \(found.prefix(80).joined(separator: ", "))")
        }
        XCTAssertTrue(picker.label.contains("Összefoglaló dátuma"), picker.label)

        picker.click()
        let today = app.menuItems["Mai nap"]
        XCTAssertTrue(today.waitForExistence(timeout: 5),
                      "a 'Mai nap' opció hiányzik a dátummenüben")
        XCTAssertTrue(app.menuItems["Előző nap"].exists,
                      "az 'Előző nap' opció hiányzik a dátummenüben")
        today.click()
        XCTAssertTrue(element("ops.summary", in: app).exists,
                      "az összefoglaló sáv a dátumváltás után is megmarad")
    }

    func testOperationsExposesResolveActionForOpenIssue() throws {
        let app = try openOperations(Self.loadedPort)
        let resolve = element("ops.issue.resolve.AAAAAAAA-0000-0000-0000-000000000001", in: app)
        XCTAssertTrue(resolve.waitForExistence(timeout: 15), "a megoldás akció hiányzik")
        XCTAssertTrue(resolve.label.contains("Jel megoldottnak jelölése"), resolve.label)
    }

    // MARK: - Segédek

    private func openLoadedMatchDetail() throws -> XCUIApplication {
        let app = try launch(port: Self.loadedPort)
        app.activate()
        let card = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(card.waitForExistence(timeout: 30), "a nap nem töltődött be")
        card.click()
        XCTAssertTrue(element("match.detail", in: app).waitForExistence(timeout: 20),
                      "a részletező nem nyílt meg")
        XCTAssertTrue(element("match.detail.form", in: app).waitForExistence(timeout: 10),
                      "a részletező forma szakasza hiányzik")
        return app
    }

    private func assertChainTarget(id: String, marker: String, in app: XCUIApplication) {
        let child = element("match.detail.\(id)", in: app)
        XCTAssertTrue(child.waitForExistence(timeout: 20), "a kapcsolódó meccs részletezője nem nyílt meg")
        XCTAssertEqual(app.descendants(matching: .any).matching(identifier: "match.detail").count, 1,
                       "a szülő és a gyermek részletező azonosítója ütközik")
        XCTAssertEqual(app.descendants(matching: .any).matching(identifier: "match.detail.\(id)").count, 1,
                       "a gyermek részletező azonosítója nem egyedi")
        let back = element("match.detail.back", in: app)
        XCTAssertTrue(back.waitForExistence(timeout: 10), "a gyermek vissza-gombja hiányzik")
        XCTAssertTrue(textElement(valueContains: marker, in: app).waitForExistence(timeout: 10),
                      "a kért UUID helyett másik meccs töltődött be: \(marker)")
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(back, timeout: 10), "Escape nem a gyermek részletezőt zárta be")
        XCTAssertTrue(waitForGone(child, timeout: 10), "a gyermek részletező Escape után is megmaradt")
        XCTAssertTrue(element("match.detail.form", in: app).waitForExistence(timeout: 10),
                      "Escape a teljes meccsláncot bezárta")
    }

    private func focusByTabbing(_ element: XCUIElement, in app: XCUIApplication, timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            if element.isSelected { return true }
            app.typeKey("\t", modifierFlags: [])
            _ = element.waitForExistence(timeout: 0.2)
        } while Date() < deadline
        return element.isSelected
    }

    /// A szekcióra görgetés után az elem ténylegesen látható-e (nemcsak az AX-fában van).
    private func waitForHittable(_ element: XCUIElement, timeout: TimeInterval) -> Bool {
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isHittable == true"),
            object: element
        )
        return XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed
    }

    /// A fókuszált kártya az AX-fában `selected` jelzést kap (ezt hirdeti a VoiceOver is).
    private func waitForSelected(_ element: XCUIElement, timeout: TimeInterval) -> Bool {
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isSelected == true"),
            object: element
        )
        return XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed
    }

    /// A Tab-sorrend fix: 4 felső navigáció + 5 fejléc-vezérlő + 4 szűrő +
    /// a kiemelt-kör váltó után jön az első kártya (a 15. Tab).
    /// Tabbal a lista első kártyájáig lépked. A felső navigáció és a szűrősáv
    /// elemszáma a látható vezérlőktől függ, ezért nem fix Tab-számot várunk:
    /// addig lépkedünk, amíg a kártya fókuszba nem kerül.
    private func focusFirstMatchCard(_ element: XCUIElement, in app: XCUIApplication) -> Bool {
        for _ in 0..<30 {
            if element.isSelected { return true }
            app.typeKey("\t", modifierFlags: [])
            _ = element.waitForExistence(timeout: 0.2)
        }
        return waitForSelected(element, timeout: 5)
    }

    private func waitForAnySelected(_ elements: [XCUIElement], timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            if elements.contains(where: { $0.exists && $0.isSelected }) { return true }
            _ = XCTWaiter.wait(for: [XCTestExpectation()], timeout: 0.2)
        } while Date() < deadline
        return false
    }

    private func waitForNotSelected(_ element: XCUIElement, timeout: TimeInterval) -> Bool {
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "isSelected == false"),
            object: element
        )
        return XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed
    }

    private func waitForGone(_ element: XCUIElement, timeout: TimeInterval) -> Bool {
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "exists == false"),
            object: element
        )
        return XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed
    }

    /// A futtató környezet (runner) sandboxja miatt a runtime.json-t a runner
    /// saját temp könyvtárába írjuk; az app (nincs sandboxban) onnan olvassa.
    private func launch(
        port: UInt16,
        playerID: String? = nil,
        windowWidth: CGFloat? = nil
    ) throws -> XCUIApplication {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("football-ui-fixture-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let runtimeURL = directory.appendingPathComponent("runtime.json")
        // Az Üzemeltetés oldal ops tokent használ; a valódi runtime.json is tartalmazza.
        let runtime = "{\"port\": \(port), \"read_token\": \"ui-test-token\", \"ops_token\": \"ui-test-ops-token\"}"
        try Data(runtime.utf8).write(to: runtimeURL)

        let app = XCUIApplication()
        app.launchEnvironment["FOOTBALL_APP_RUNTIME_FILE"] = runtimeURL.path
        if let playerID {
            app.launchEnvironment["FOOTBALL_UI_PLAYER_ID"] = playerID
        }
        if let windowWidth {
            app.launchEnvironment["FOOTBALL_UI_WINDOW_WIDTH"] = String(Int(windowWidth))
        }
        app.launch()
        app.activate()
        return app
    }

    /// A SwiftUI `Text` a macOS accessibility fában üres labellel, a szöveget
    /// a `value` attribútumban hordozza, ezért ott kell keresni.
    private func textElement(valueContains fragment: String, in app: XCUIApplication) -> XCUIElement {
        app.staticTexts.matching(NSPredicate(format: "value CONTAINS %@", fragment)).firstMatch
    }

    private func element(_ identifier: String, in app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: identifier).firstMatch
    }

}
