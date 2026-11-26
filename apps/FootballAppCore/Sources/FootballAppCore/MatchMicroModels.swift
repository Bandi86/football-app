import Foundation

/// Meccs-szintű mikro-adatok (S182/S3): felállás, játékosstatisztika, lövések.
/// Hiány esetén üres lista — a DTO nem talál ki játékost vagy értéket.
/// A három végpont burkolója (`{"lineups": [...]}` stb.).
public struct LineupsPayload: Decodable, Sendable {
    public let lineups: [LineupEntry]
}

public struct PlayerStatsPayload: Decodable, Sendable {
    public let playerStats: [PlayerStatEntry]

    enum CodingKeys: String, CodingKey {
        case playerStats = "player_stats"
    }
}

public struct ShotsPayload: Decodable, Sendable {
    public let shots: [ShotEntry]
}

public struct MatchMicro: Equatable, Sendable {
    public let lineups: [LineupEntry]
    public let playerStats: [PlayerStatEntry]
    public let shots: [ShotEntry]

    public init(lineups: [LineupEntry], playerStats: [PlayerStatEntry], shots: [ShotEntry]) {
        self.lineups = lineups
        self.playerStats = playerStats
        self.shots = shots
    }
}

public struct LineupEntry: Decodable, Equatable, Sendable, Identifiable {
    public let side: String
    public let role: String
    public let sourcePlayerId: String
    public let playerEntityId: UUID?
    public let playerName: String?
    public let position: String?
    public let shirtNumber: Int?
    public let rating: Double?
    public let capturedAt: Date

    public var id: String { "\(side)|\(sourcePlayerId)" }

    enum CodingKeys: String, CodingKey {
        case side, role, position, rating
        case sourcePlayerId = "source_player_id"
        case playerEntityId = "player_entity_id"
        case playerName = "player_name"
        case shirtNumber = "shirt_number"
        case capturedAt = "captured_at"
    }
}

public struct PlayerStatEntry: Decodable, Equatable, Sendable, Identifiable {
    public let side: String
    public let sourcePlayerId: String
    public let playerEntityId: UUID?
    public let playerName: String?
    public let statKey: String
    public let value: Double?
    public let capturedAt: Date

    public var id: String { "\(sourcePlayerId)|\(statKey)" }

    enum CodingKeys: String, CodingKey {
        case side, value
        case sourcePlayerId = "source_player_id"
        case playerEntityId = "player_entity_id"
        case playerName = "player_name"
        case statKey = "stat_key"
        case capturedAt = "captured_at"
    }
}

public struct ShotEntry: Decodable, Equatable, Sendable, Identifiable {
    public let shotNo: Int
    public let side: String
    public let sourcePlayerId: String?
    public let playerEntityId: UUID?
    public let playerName: String?
    public let minute: Int?
    public let x: Double?
    public let y: Double?
    public let xg: Double?
    public let xgot: Double?
    public let shotType: String?
    public let capturedAt: Date

    public var id: Int { shotNo }

    enum CodingKeys: String, CodingKey {
        case side, minute, x, y, xg, xgot
        case shotNo = "shot_no"
        case sourcePlayerId = "source_player_id"
        case playerEntityId = "player_entity_id"
        case playerName = "player_name"
        case shotType = "shot_type"
        case capturedAt = "captured_at"
    }
}
