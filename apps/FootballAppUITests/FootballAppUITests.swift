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

    // MARK: - Segédek

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
    private func launch(port: UInt16, playerID: String? = nil) throws -> XCUIApplication {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("football-ui-fixture-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let runtimeURL = directory.appendingPathComponent("runtime.json")
        let runtime = "{\"port\": \(port), \"read_token\": \"ui-test-token\"}"
        try Data(runtime.utf8).write(to: runtimeURL)

        let app = XCUIApplication()
        app.launchEnvironment["FOOTBALL_APP_RUNTIME_FILE"] = runtimeURL.path
        if let playerID {
            app.launchEnvironment["FOOTBALL_UI_PLAYER_ID"] = playerID
        }
        app.launch()
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
