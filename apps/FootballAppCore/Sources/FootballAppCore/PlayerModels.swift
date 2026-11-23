import Foundation

/// A `GET /v1/player/{id}` válasz DTO-i. A hiány `nil`, nem kitalált 0 —
/// például a rating és az esemény-perc is opcionális a forrásban.
public struct PlayerDetail: Codable, Equatable, Sendable, Identifiable {
    public let id: UUID
    public let name: String
    public let countryCode: String?

    enum CodingKeys: String, CodingKey {
        case id, name
        case countryCode = "country_code"
    }
}

public struct PlayerSpell: Codable, Equatable, Sendable {
    public let teamName: String
    public let validFrom: String
    public let validTo: String?
    public let loan: Bool

    enum CodingKeys: String, CodingKey {
        case teamName = "team_name"
        case validFrom = "valid_from"
        case validTo = "valid_to"
        case loan
    }
}

public struct PlayerAppearance: Codable, Equatable, Sendable {
    public let matchId: UUID
    public let kickoffAt: Date
    public let status: String
    public let competitionName: String?
    public let side: String?
    public let role: String
    public let rating: Double?
    public let capturedAt: Date
    public let sourceCode: String

    enum CodingKeys: String, CodingKey {
        case matchId = "match_id"
        case kickoffAt = "kickoff_at"
        case status
        case competitionName = "competition_name"
        case side, role, rating
        case capturedAt = "captured_at"
        case sourceCode = "source_code"
    }
}

public struct PlayerStat: Codable, Equatable, Sendable {
    public let matchId: UUID
    public let statKey: String
    public let value: Double?
    public let capturedAt: Date
    public let sourceCode: String

    enum CodingKeys: String, CodingKey {
        case matchId = "match_id"
        case statKey = "stat_key"
        case value
        case capturedAt = "captured_at"
        case sourceCode = "source_code"
    }
}

public struct PlayerIncident: Codable, Equatable, Sendable {
    public let matchId: UUID
    public let incidentType: String
    public let minute: Int?
    public let playerName: String?
    public let relatedPlayerName: String?
    public let capturedAt: Date

    enum CodingKeys: String, CodingKey {
        case matchId = "match_id"
        case incidentType = "incident_type"
        case minute
        case playerName = "player_name"
        case relatedPlayerName = "related_player_name"
        case capturedAt = "captured_at"
    }
}

public struct PlayerFeed: Codable, Equatable, Sendable {
    public let player: PlayerDetail
    public let spells: [PlayerSpell]
    public let appearances: [PlayerAppearance]
    public let stats: [PlayerStat]
    public let incidents: [PlayerIncident]
}
