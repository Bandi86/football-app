import Foundation

/// Egy globális keresési találat. A `kind` a háttér által megnevezett fajta:
/// `team`, `competition` vagy `match`.
public struct SearchResult: Codable, Equatable, Sendable, Identifiable {
    public let kind: String
    public let id: UUID
    public let title: String
    public let subtitle: String?

    public init(kind: String, id: UUID, title: String, subtitle: String?) {
        self.kind = kind
        self.id = id
        self.title = title
        self.subtitle = subtitle
    }

    public var kindLabel: String {
        switch kind {
        case "team": return "Csapat"
        case "competition": return "Verseny"
        case "match": return "Meccs"
        case "player": return "Játékos"
        default: return "Találat"
        }
    }
}

/// A háttér `/v1/search` válasza — hiány esetén üres tömb, nem hamis találat.
public struct SearchFeed: Codable, Equatable, Sendable {
    public let query: String
    public let results: [SearchResult]

    public init(query: String, results: [SearchResult]) {
        self.query = query
        self.results = results
    }
}
