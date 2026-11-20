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

        let finished = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(finished.waitForExistence(timeout: 10), "a lezárt meccs kártyája hiányzik")
        XCTAssertTrue(finished.label.contains("Debrecen – Paks"), finished.label)
        XCTAssertTrue(finished.label.contains("Lezárt"), finished.label)
        XCTAssertTrue(finished.label.contains("2 – 1"), finished.label)
        XCTAssertTrue(finished.label.contains("Ellenőrzött publikáció"), finished.label)
    }

    // MARK: - Részleges / bizonytalan adat

    /// A hiányzó eredményt és a publikációs minőségi állapotot minden esetben
    /// ki kell mondani — a szín önmagában nem hordozhat információt.
    func testPartialDataSpeaksStatusScoreAndPublishState() throws {
        let app = try launch(port: Self.partialPort)

        let live = element("match.card.AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA", in: app)
        XCTAssertTrue(live.waitForExistence(timeout: 30), "a részleges nap nem töltődött be")
        XCTAssertTrue(live.label.contains("Győr – Fehérvár"), live.label)
        XCTAssertTrue(live.label.contains("Élő"), live.label)
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

    // MARK: - Billentyűzet-fókusz a napi listában

    /// Tab belép a listába, a nyilak lépkednek, Return megnyitja a részletezőt,
    /// Escape kilép. A fókusz vizuális jelzése a kártya borostyán kerete.
    func testKeyboardFocusWalksMatchCardsAndOpensDetail() throws {
        let app = try launch(port: Self.loadedPort)
        app.activate()

        let first = element("match.card.11111111-1111-1111-1111-111111111111", in: app)
        let second = element("match.card.22222222-2222-2222-2222-222222222222", in: app)
        XCTAssertTrue(first.waitForExistence(timeout: 30), "a nap nem töltődött be")

        app.typeKey("\t", modifierFlags: [])
        XCTAssertTrue(waitForSelected(first, timeout: 10), "Tab nem az első kártyára vitte a fókuszt")

        app.typeKey(XCUIKeyboardKey.downArrow.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForSelected(second, timeout: 10), "↓ nem lépett a következő kártyára")

        app.typeKey(XCUIKeyboardKey.upArrow.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForSelected(first, timeout: 10), "↑ nem lépett vissza az előző kártyára")

        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])
        let sheet = element("match.detail", in: app)
        XCTAssertTrue(sheet.waitForExistence(timeout: 15), "Return nem nyitotta meg a részletezőt")

        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(waitForGone(sheet, timeout: 10), "Escape nem zárta be a részletezőt")

        app.typeKey("\t", modifierFlags: .shift)
        XCTAssertTrue(waitForSelected(second, timeout: 10), "Shift+Tab nem lépett vissza a listába")
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

    private func waitForGone(_ element: XCUIElement, timeout: TimeInterval) -> Bool {
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "exists == false"),
            object: element
        )
        return XCTWaiter().wait(for: [expectation], timeout: timeout) == .completed
    }

    /// A futtató környezet (runner) sandboxja miatt a runtime.json-t a runner
    /// saját temp könyvtárába írjuk; az app (nincs sandboxban) onnan olvassa.
    private func launch(port: UInt16) throws -> XCUIApplication {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("football-ui-fixture-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let runtimeURL = directory.appendingPathComponent("runtime.json")
        let runtime = "{\"port\": \(port), \"read_token\": \"ui-test-token\"}"
        try Data(runtime.utf8).write(to: runtimeURL)

        let app = XCUIApplication()
        app.launchEnvironment["FOOTBALL_APP_RUNTIME_FILE"] = runtimeURL.path
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

