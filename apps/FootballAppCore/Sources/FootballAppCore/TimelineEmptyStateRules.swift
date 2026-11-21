import Foundation

/// Az idővonal üres állapotának eldöntése: a meccs státusza alapján várható-e
/// esemény, és ha igen, még nem érkezett-e meg egy sem. A döntés itt, a Core-ban
/// él, hogy a nézet vékony maradjon és a szabály unit-teszttel mérhető legyen.
public enum TimelineEmptyStateRules {
    /// Ezeknél a státuszoknál a meccs már elkezdődött (vagy lezárult), tehát
    /// esemény várható; ha az `incidents` üres, azt a felhasználónak kimondjuk.
    /// A `confirmed` és a `scheduled` szándékosan kimarad: ott még nincs esemény.
    public static let statusesWithExpectedIncidents: Set<String> = [
        "live",
        "halftime",
        "finished",
        "awarded",
        "abandoned",
        "penalties",
    ]

    /// Akkor igaz, ha a státusz szerint esemény várható, de az `incidents`
    /// `nil` vagy üres. Nem esemény-váró státusznál és meglévő eseménynél
    /// hamis, tehát a hívó a valódi idővonalat jelenítheti meg helyette.
    public static func shouldShowTimelineEmptyState(
        status: String,
        incidents: [MatchIncident]?
    ) -> Bool {
        guard incidents?.isEmpty ?? true else { return false }
        return statusesWithExpectedIncidents.contains(status)
    }
}