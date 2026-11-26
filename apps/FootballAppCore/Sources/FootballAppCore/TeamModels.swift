import Foundation

public struct TeamSummary: Decodable, Equatable, Sendable, Identifiable {
    public let teamId: UUID
    public let name: String
    public let countryCode: String?
    public let countryName: String?
    public let gender: String
    public let branch: String
    public let logo: String?
    public let recentMatches: Int

    public var id: UUID { teamId }

    enum CodingKeys: String, CodingKey {
        case teamId = "team_id"
        case name
        case countryCode = "country_code"
        case countryName = "country_name"
        case gender, branch, logo
        case recentMatches = "recent_matches"
    }
}

public struct TeamInfo: Decodable, Equatable, Sendable {
    public let teamId: UUID
    public let name: String
    public let countryCode: String?
    public let countryName: String?
    public let gender: String
    public let branch: String
    public let logo: String?
    public let elo: Double?
    public let played: Int
    public let wins: Int
    public let draws: Int
    public let losses: Int
    public let goalsFor: Int
    public let goalsAgainst: Int

    enum CodingKeys: String, CodingKey {
        case teamId = "team_id"
        case name
        case countryCode = "country_code"
        case countryName = "country_name"
        case gender, branch, logo, elo, played, wins, draws, losses
        case goalsFor = "goals_for"
        case goalsAgainst = "goals_against"
    }
}

public struct TeamPlayer: Decodable, Equatable, Sendable, Identifiable {
    public let sourcePlayerId: String
    public let playerEntityId: UUID?
    public let playerName: String?
    public let position: String?
    public let shirtNumber: Int?
    public let capturedAt: Date

    public var id: String { sourcePlayerId }

    enum CodingKeys: String, CodingKey {
        case sourcePlayerId = "source_player_id"
        case playerEntityId = "player_entity_id"
        case playerName = "player_name"
        case position
        case shirtNumber = "shirt_number"
        case capturedAt = "captured_at"
    }
}

public struct TeamDetail: Decodable, Equatable, Sendable {
    public let detail: TeamInfo
    public let matches: [DayMatch]
    /// Az utolsó öt lejátszott meccs a csapat szemszögéből (mai nap előtt).
    public let form: [FormEntry]?
    /// A következő legfeljebb három kijelölt meccs.
    public let next: [NextMatchEntry]?
    /// A legfrissebb felállásokból származó as-of keret (hiány esetén []).
    public let players: [TeamPlayer]?
}
