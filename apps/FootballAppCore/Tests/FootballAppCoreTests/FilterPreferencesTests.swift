import Foundation
import Testing
@testable import FootballAppCore

@Suite struct FilterPreferencesTests {
    @Test func defaultsAreSafeAndReadOnly() {
        let suite = "filterprefs.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        let preferences = FilterPreferences(defaults: defaults)
        #expect(preferences.playerStatsSide == "Mind")
        #expect(preferences.aiAssistEnabled == false)
    }

    @Test func selectionIsPersistedAndInvalidValueFallsBack() {
        let suite = "filterprefs.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        var preferences = FilterPreferences(defaults: defaults)
        preferences.playerStatsSide = "Vendég"
        preferences.save(defaults: defaults)

        let loaded = FilterPreferences(defaults: defaults)
        #expect(loaded.playerStatsSide == "Vendég")

        defaults.set(["playerStatsSide": "Ismeretlen", "aiAssistEnabled": false],
                     forKey: FilterPreferences.defaultsKey)
        #expect(FilterPreferences(defaults: defaults).playerStatsSide == "Mind")
    }
}
