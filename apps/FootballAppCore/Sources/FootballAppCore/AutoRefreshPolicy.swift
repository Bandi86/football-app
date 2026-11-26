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
