import Foundation

public enum PublishState: String, Decodable, Sendable {
    case verifiedPublish = "verified_publish"
    case provisionalPublish = "provisional_publish"
    case withheld = "withheld"
    case quarantined = "quarantined"
    case unknown = "unknown"

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = PublishState(rawValue: raw) ?? .unknown
    }
}

public struct Blocker: Decodable, Equatable, Sendable {
    public let ruleKey: String
    public let severity: String
    public let field: String?
    public let message: String

    enum CodingKeys: String, CodingKey {
        case ruleKey = "rule_key"
        case severity
        case field
        case message
    }
}

public struct DayMatch: Decodable, Equatable, Identifiable, Sendable {
    public let id: UUID
    public let kickoffAt: Date
    public let status: String
    public let home: String
    public let away: String
    public let homeScore: Int?
    public let awayScore: Int?
    public let publishState: PublishState
    public let blockers: [Blocker]

    enum CodingKeys: String, CodingKey {
        case id
        case kickoffAt = "kickoff_at"
        case status
        case home
        case away
        case homeScore = "home_score"
        case awayScore = "away_score"
        case publishState = "publish_state"
        case blockers
    }
}

public struct DayResponse: Decodable, Sendable {
    public let date: String
    public let timezone: String
    public let matches: [DayMatch]
}
