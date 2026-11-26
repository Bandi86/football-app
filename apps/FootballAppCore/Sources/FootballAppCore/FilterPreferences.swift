import Foundation

/// A megjelenítési szűrők kliensoldali, `UserDefaults`-ban megőrzött beállításai.
/// A választások nem kerülnek a nyilvános API-ba és nem tartalmaznak titkot.
public struct FilterPreferences: Equatable, Sendable {
    public static let defaultsKey = "ui.filterPreferences"

    public var playerStatsSide: String
    /// Az AI-gomb alapból ki; a bekapcsolás külön döntés (S9).
    public var aiAssistEnabled: Bool

    public init(playerStatsSide: String = "Mind", aiAssistEnabled: Bool = false) {
        self.playerStatsSide = playerStatsSide
        self.aiAssistEnabled = aiAssistEnabled
    }

    public init(defaults: UserDefaults = .standard) {
        let values = defaults.dictionary(forKey: Self.defaultsKey)
        let side = values?["playerStatsSide"] as? String ?? "Mind"
        let ai = values?["aiAssistEnabled"] as? Bool ?? false
        self.init(
            playerStatsSide: ["Mind", "Hazai", "Vendég"].contains(side) ? side : "Mind",
            aiAssistEnabled: ai
        )
    }

    public func save(defaults: UserDefaults = .standard) {
        defaults.set([
            "playerStatsSide": playerStatsSide,
            "aiAssistEnabled": aiAssistEnabled,
        ], forKey: Self.defaultsKey)
    }
}
