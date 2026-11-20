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

    // MARK: - Segédek

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

