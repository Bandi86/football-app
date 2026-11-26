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
        "result_pending": "Eredmény várható",
    ]

    private static let kickoffFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    private static let relatedMatchFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .medium
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

    public static func formMatchLabel(teamName: String, _ entry: FormEntry) -> String {
        let venue = entry.isHome ? "hazai" : "vendég"
        let result = entry.outcome == "W" ? "győzelem"
            : (entry.outcome == "L" ? "vereség" : "döntetlen")
        return "Meccs megnyitása: \(teamName), \(venue) \(entry.opponentName) ellen, "
            + "\(result) \(entry.teamGoals)–\(entry.opponentGoals), \(entry.competitionName), "
            + relatedMatchFormatter.string(from: entry.kickoffAt)
    }

    public static func formMatchIdentifier(
        for id: UUID,
        detailPrefix: String = "match.detail"
    ) -> String {
        "\(detailPrefix).form.\(id.uuidString)"
    }

    public static func headToHeadMatchLabel(_ entry: HeadToHeadEntry) -> String {
        let score: String
        if let home = entry.homeGoals, let away = entry.awayGoals {
            score = "\(home)–\(away)"
        } else {
            score = "eredmény még nincs"
        }
        return "Meccs megnyitása: \(entry.homeName) – \(entry.awayName), \(score), "
            + "\(entry.competitionName), \(relatedMatchFormatter.string(from: entry.kickoffAt))"
    }

    public static func headToHeadMatchIdentifier(
        for id: UUID,
        detailPrefix: String = "match.detail"
    ) -> String {
        "\(detailPrefix).h2h.\(id.uuidString)"
    }

    public static func nextMatchLabel(teamName: String, _ entry: NextMatchEntry) -> String {
        let venue = entry.isHome ? "hazai" : "vendég"
        return "Meccs megnyitása: \(teamName), \(venue) \(entry.opponentName) ellen, "
            + "\(entry.competitionName), \(relatedMatchFormatter.string(from: entry.kickoffAt))"
    }

    public static func nextMatchIdentifier(
        for id: UUID,
        detailPrefix: String = "match.detail"
    ) -> String {
        "\(detailPrefix).next.\(id.uuidString)"
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

    // MARK: - Üzemeltetés: hiányjelek és idővonal

    public static let issuesEmptyIdentifier = "ops.issues.empty"
    public static let issuesEmptyTitle = "Még nincs rögzített hiányjel."
    public static let issuesEmptySubtitle = "A napi ellenőrzés a jeleket ide rögzíti."
    public static func issuesEmptyLabel() -> String {
        "\(issuesEmptyTitle) \(issuesEmptySubtitle)"
    }

    public static let issuesFilterIdentifier = "ops.issues.filter"
    public static let issuesOpenOnlyIdentifier = "ops.issues.openOnly"

    /// A jel teljes, felolvasható tartalma: mi a hiány, milyen súlyos,
    /// mióta látszik, mikor látták utoljára, és megoldva-e.
    public static func issueLabel(
        code: String,
        severity: String,
        subject: String,
        firstSeen: String,
        lastSeen: String,
        source: String,
        resolved: Bool
    ) -> String {
        let state = resolved ? "megoldva" : "megoldatlan"
        return "\(code), \(severity), \(subject), először \(firstSeen), utoljára \(lastSeen), "
            + "forrás \(source), \(state)"
    }

    public static func issueIdentifier(for id: UUID) -> String {
        "ops.issue.\(id.uuidString)"
    }

    /// Egy idővonal-bejegyzés: ki, mikor, mit írt/módosított, forrással.
    public static func historyEntryLabel(
        time: String,
        actor: String,
        action: String,
        field: String?,
        source: String?
    ) -> String {
        var parts = [time, actor, action]
        if let field, !field.isEmpty { parts.append("mező: \(field)") }
        if let source, !source.isEmpty { parts.append("forrás: \(source)") }
        return parts.joined(separator: ", ")
    }

    public static func historyEntryIdentifier(for index: Int) -> String {
        "ops.history.row.\(index)"
    }

    public static let historyEmptyIdentifier = "ops.history.empty"
    public static let historyEmptyTitle = "Nincs rögzített változás ehhez a jelhez."
    public static func historyEmptyLabel() -> String { historyEmptyTitle }

    public static func issueDetailTitle(_ code: String) -> String {
        "Változástörténet: \(code)"
    }

    // MARK: - Összefoglaló, globális audit és akciók

    public static let opsSummaryIdentifier = "ops.summary"
    public static let opsSummaryDateIdentifier = "ops.summary.date"
    public static let opsSummaryDateLabel = "Összefoglaló dátuma"
    public static let opsAuditIdentifier = "ops.audit"
    public static let auditEmptyIdentifier = "ops.audit.empty"
    public static let auditEmptyTitle = "Még nincs audit-esemény."

    public static func auditEntryLabel(
        time: String,
        actor: String,
        action: String,
        subject: String
    ) -> String {
        "\(time), \(actor), \(action), \(subject)"
    }

    public static func auditEntryIdentifier(for id: UUID) -> String {
        "ops.audit.\(id.uuidString)"
    }

    public static let issueResolveLabel = "Jel megoldottnak jelölése"

    public static func issueResolveIdentifier(for id: UUID) -> String {
        "ops.issue.resolve.\(id.uuidString)"
    }
}
