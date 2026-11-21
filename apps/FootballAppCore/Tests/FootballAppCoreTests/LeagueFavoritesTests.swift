import Foundation
import Testing
@testable import FootballAppCore

struct LeagueFavoritesTests {
    private func isolatedDefaults() -> UserDefaults {
        let suite = "test.leagueFavorites.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defaults.removePersistentDomain(forName: suite)
        return defaults
    }

    @Test func togglesAndPersistsKeys() {
        let defaults = isolatedDefaults()
        let store = LeagueFavorites(defaults: defaults)
        #expect(store.load().isEmpty)
        let afterAdd = store.toggle("ES|la liga")
        #expect(afterAdd == ["ES|la liga"])
        #expect(store.contains("ES|la liga"))
        let afterRemove = store.toggle("ES|la liga")
        #expect(afterRemove.isEmpty)
        #expect(!store.contains("ES|la liga"))
    }

    @Test func keepsMultipleKeysSorted() {
        let defaults = isolatedDefaults()
        let store = LeagueFavorites(defaults: defaults)
        store.save(["HU|nb i", "GB-ENG|premier league", "ES|la liga"])
        #expect(defaults.stringArray(forKey: LeagueFavorites.defaultsKey)
            == ["ES|la liga", "GB-ENG|premier league", "HU|nb i"])
    }
}
