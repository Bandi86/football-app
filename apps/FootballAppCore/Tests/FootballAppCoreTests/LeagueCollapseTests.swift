import XCTest
@testable import FootballAppCore

final class LeagueCollapseTests: XCTestCase {
    private func makeDefaults() -> UserDefaults {
        let suite = "LeagueCollapseTests.\(UUID().uuidString)"
        return UserDefaults(suiteName: suite)!
    }

    func testToggleAddsAndRemovesKeys() {
        let store = LeagueCollapse(defaults: makeDefaults())
        XCTAssertTrue(store.load().isEmpty)
        XCTAssertEqual(store.toggle("ES|la liga"), ["ES|la liga"])
        XCTAssertTrue(store.contains("ES|la liga"))
        XCTAssertTrue(store.toggle("ES|la liga").isEmpty)
        XCTAssertFalse(store.contains("ES|la liga"))
    }

    func testSaveStoresSortedKeys() {
        let defaults = makeDefaults()
        let store = LeagueCollapse(defaults: defaults)
        store.save(["PL|i liga", "ES|la liga"])
        XCTAssertEqual(defaults.stringArray(forKey: LeagueCollapse.defaultsKey), ["ES|la liga", "PL|i liga"])
    }
}
