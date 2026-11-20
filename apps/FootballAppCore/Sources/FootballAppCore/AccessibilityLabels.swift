import Foundation

/// VoiceOver-címkék és stabil accessibility-identifiers.
///
/// A címkék itt, a `FootballAppCore`-ban élnek, hogy a megjelenített szöveg és az
/// akadálymentes szöveg ugyanabból a forrásból jöjjön, és unit-tesztelhető legyen.
public enum AccessibilityLabels {
    private static let statusLabels: [String: String] = [
        "scheduled": "Ütemezett",
        "confirmed": "Megerősített",
        "live": "Élő",
        "halftime": "Félidő",
        "finished": "Lezárt",
        "postponed": "Elhalasztott",
        "canceled": "Törölt",
        "abandoned": "Félbeszakadt",
        "awarded": "Megítélt",
    ]

    private static let kickoffFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    /// Ismeretlen státusz esetén a nyers érték marad (nem találunk ki magyarázatot).
    public static func statusLabel(for status: String) -> String {
        statusLabels[status] ?? status
    }

    /// A publikációs állapot kimondása: a szín önmagában nem hordozza az információt.
    public static func publishLabel(_ state: PublishState) -> String {
        switch state {
        case .verifiedPublish: return "Ellenőrzött publikáció"
        case .provisionalPublish: return "Ideiglenes publikáció"
        case .withheld: return "Visszatartott publikáció"
        case .quarantined: return "Karanténban lévő publikáció"
        case .unknown: return "Ismeretlen publikáció"
        }
    }

    /// Egy meccssor teljes, felolvasható tartalma. A hiányzó eredményt kimondja,
    /// soha nem olvasja fel 0–0-ként.
    public static func matchRowLabel(
        home: String,
        away: String,
        kickoffAt: Date,
        status: String,
        homeScore: Int?,
        awayScore: Int?,
        publishState: PublishState
    ) -> String {
        let score: String
        if let homeScore, let awayScore {
            score = "\(homeScore) – \(awayScore)"
        } else {
            score = "eredmény még nincs"
        }
        return "\(home) – \(away), kezdés \(kickoffFormatter.string(from: kickoffAt)), "
            + "\(statusLabel(for: status)), \(score), \(publishLabel(publishState))"
    }

    /// A napi lista kártyájának címkéje: a megjelenített dátumot és időt használja
    /// (nem a nyers UTC Date-t). Az odds csak akkor kerül a címkébe, ha a kártyán
    /// is látszik.
    public static func matchCardLabel(
        home: String,
        away: String,
        time: String,
        status: String,
        homeScore: Int?,
        awayScore: Int?,
        publishState: PublishState,
        dayLabel: String? = nil,
        odds: String? = nil
    ) -> String {
        let score: String
        if let homeScore, let awayScore {
            score = "\(homeScore) – \(awayScore)"
        } else {
            score = "eredmény még nincs"
        }
        let kickoff = [dayLabel, time]
            .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        var parts = [home + " – " + away, kickoff, statusLabel(for: status), score,
                     publishLabel(publishState)]
        if let odds, !odds.isEmpty {
            parts.append("odds \(odds)")
        }
        return parts.joined(separator: ", ")
    }

    /// Kattintás- és tesztstabil azonosító soronként.
    public static func matchIdentifier(for id: UUID) -> String {
        "match.row.\(id.uuidString)"
    }
}
