import Foundation

/// A napi lista háttér-frissítési üteme.
///
/// A múltbeli napoknak és az élő meccs nélküli mai napnak is frissülnie kell:
/// a daemon a research-slotokban írja be az új eredményeket/oddsokat, a kliens
/// pedig különben a nap kiválasztásakor betöltött pillanatképet mutatná örökre.
public enum AutoRefreshPolicy {
    public static let liveIntervalSeconds = 60
    public static let idleIntervalSeconds = 300

    public static func intervalSeconds(liveMatchCount: Int) -> Int {
        liveMatchCount > 0 ? liveIntervalSeconds : idleIntervalSeconds
    }
}

/// Az alkalmazás életciklusa a frissítés szempontjából. A SwiftUI `ScenePhase`
/// helyett Core-ban tartjuk, hogy a döntés unit-tesztelhető maradjon, és a kliens
/// ne kelljen hozzá SwiftUI-t importálni.
public enum AppLifecyclePhase: Equatable {
    case active, inactive, background
}

extension AutoRefreshPolicy {
    /// A háttérben töltött idő alatt a daemon új eredményeket/oddsokat írhat,
    /// miközben az időzített kör szuszpendálva van. Ezért a felhasználó elé
    /// kerüléskor (inactive/background → active) azonnal újra kérjük az adatot —
    /// nem kell megvárni a következő 5 perces kört, és nem kell kézzel Frissíteni.
    public static func shouldRefreshOnActivation(
        from previous: AppLifecyclePhase,
        to current: AppLifecyclePhase
    ) -> Bool {
        current == .active && previous != .active
    }
}
