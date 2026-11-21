import Foundation

/// Kedvenc versenyek tárolója. A kulcs a verseny kanonikus azonosítója
/// (`LeagueGroup.id`, pl. `"ES|la liga"`), így a kedvenc a liga átnevezése
/// vagy két entitás összevonása után is megmarad. Az adat a felhasználó
/// UserDefaults-ában él, mert ez kliensoldali személyes beállítás.
public struct LeagueFavorites {
    /// A UserDefaults kulcsa; a tesztek izolált suite-dal használják.
    public static let defaultsKey = "day.favoriteLeagues"

    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    /// A mentett liga-kulcsok rendezett halmaza.
    public func load() -> Set<String> {
        Set(defaults.stringArray(forKey: Self.defaultsKey) ?? [])
    }

    public func contains(_ key: String) -> Bool {
        load().contains(key)
    }

    /// Be- vagy kikapcsolja a kulcsot, és visszaadja az új halmazt.
    @discardableResult
    public func toggle(_ key: String) -> Set<String> {
        var keys = load()
        if keys.contains(key) {
            keys.remove(key)
        } else {
            keys.insert(key)
        }
        save(keys)
        return keys
    }

    public func save(_ keys: Set<String>) {
        defaults.set(keys.sorted(), forKey: Self.defaultsKey)
    }
}
