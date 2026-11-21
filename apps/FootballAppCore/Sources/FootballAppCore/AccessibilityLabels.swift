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
        kickoffNote: String? = nil,
        round: String? = nil,
        odds: String? = nil
    ) -> String {
        let score: String
        if let homeScore, let awayScore {
            score = "\(homeScore) – \(awayScore)"
        } else {
            score = "eredmény még nincs"
        }
        let kickoff = [dayLabel, kickoffNote ?? time]
            .compactMap { $0?.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        var parts = [home + " – " + away, kickoff, statusLabel(for: status), score,
                     publishLabel(publishState)]
        if let round, !round.isEmpty {
            parts.append(round)
        }
        if let odds, !odds.isEmpty {
            parts.append("odds \(odds)")
        }
        return parts.joined(separator: ", ")
    }

    /// Kattintás- és tesztstabil azonosító soronként.
    public static func matchIdentifier(for id: UUID) -> String {
        "match.row.\(id.uuidString)"
    }

    /// Az idővonal üres állapotának stabil accessibility-identifierje.
    public static let timelineEmptyIdentifier = "match.timeline.empty"

    /// Az idővonal üres állapotának megjelenített és felolvasott szövege
    /// ugyanabból a forrásból jön, hogy a kettő ne csússzon el egymástól.
    public static let timelineEmptyTitle = "Az események még nem érkeztek meg."
    public static let timelineEmptySubtitle = "A részletes adatok betöltése folyamatban lehet."

    /// Az üres idővonal felolvasható tartalma: a címet és az alcímet mondja ki.
    public static func timelineEmptyLabel() -> String {
        "\(timelineEmptyTitle) \(timelineEmptySubtitle)"
    }

    /// Az idővonal-esemény típusának magyar, felolvasható neve. Az ismeretlen
    /// forrás-stringet sem hallgatjuk el, hanem egyértelműen "egyéb"-ként mondjuk.
    public static func incidentTypeLabel(_ kind: MatchIncidentKind) -> String {
        switch kind {
        case .goal: return "Gól"
        case .card: return "Kártya"
        case .substitution: return "Csere"
        case .half: return "Félidő"
        case .addedTime: return "Ráadásidő"
        case .varReview: return "VAR-ellenőrzés"
        case .missedPenalty: return "Kihagyott büntető"
        case .penaltyShootout: return "Tizenegyespárbaj"
        case .other(let raw):
            return raw.isEmpty ? "Egyéb esemény" : "Egyéb esemény: \(raw)"
        }
    }

    /// Egy idővonal-sor teljes, felolvasható tartalma. A hiányzó percet
    /// kimondja, a hiányzó oldalt nem találja ki; a gól utáni állást csak
    /// akkor mondja, ha az esemény valóban gól és mindkét érték megvan.
    public static func matchIncidentLabel(
        minute: Int?,
        kind: MatchIncidentKind,
        side: String?,
        playerName: String?,
        relatedPlayerName: String?,
        detail: String?,
        scoreAfterHome: Int?,
        scoreAfterAway: Int?
    ) -> String {
        var parts: [String] = []
        if let minute {
            parts.append("\(minute). perc")
        } else {
            parts.append("perc nélkül")
        }
        switch side {
        case "home": parts.append("hazai")
        case "away": parts.append("vendég")
        default: break
        }
        parts.append(incidentTypeLabel(kind))
        if let playerName, !playerName.isEmpty {
            parts.append(playerName)
        }
        if let relatedPlayerName, !relatedPlayerName.isEmpty {
            parts.append("másik játékos: \(relatedPlayerName)")
        }
        if let detail, !detail.isEmpty {
            parts.append(detail)
        }
        if kind == .goal, let scoreAfterHome, let scoreAfterAway {
            parts.append("\(scoreAfterHome) – \(scoreAfterAway)")
        }
        return parts.joined(separator: ", ")
    }
}
