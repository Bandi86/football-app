import Foundation

/// Összecsukott verseny-szekciók tárolója. A kulcs a verseny kanonikus
/// azonosítója (`LeagueGroup.id`), így a beállítás liga-átnevezés után is
/// megmarad. Kliensoldali személyes beállítás, ezért UserDefaults-ban él.
public struct LeagueCollapse {
    /// A UserDefaults kulcsa; a tesztek izolált suite-dal használják.
    public static let defaultsKey = "day.collapsedLeagues"

    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    /// A mentett, összecsukott liga-kulcsok halmaza.
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
