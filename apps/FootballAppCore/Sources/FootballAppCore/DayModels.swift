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

public struct OddsSnapshot: Decodable, Equatable, Sendable {
    public let home: Double
    public let draw: Double
    public let away: Double
    public let bttsYes: Double
    public let bttsNo: Double
    public let over25: Double
    public let under25: Double

    enum CodingKeys: String, CodingKey {
        case home, draw, away
        case bttsYes = "btts_yes"
        case bttsNo = "btts_no"
        case over25, under25
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
    public let competitionId: UUID?
    public let competitionName: String?
    public let countryCode: String?
    public let countryName: String?
    public let tier: Int?
    public let priority: Int?
    public let homeHt: Int?
    public let awayHt: Int?
    public let homeEt: Int?
    public let awayEt: Int?
    public let homePens: Int?
    public let awayPens: Int?
    public let homePosition: Int?
    public let awayPosition: Int?
    public let odds: OddsSnapshot?
    public let marketCount: Int?
    public let oddsSource: String?
    public let oddsCapturedAt: Date?

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
        case competitionId = "competition_id"
        case competitionName = "competition_name"
        case countryCode = "country_code"
        case countryName = "country_name"
        case tier, priority
        case homeHt = "home_ht"
        case awayHt = "away_ht"
        case homeEt = "home_et"
        case awayEt = "away_et"
        case homePens = "home_pens"
        case awayPens = "away_pens"
        case homePosition = "home_position"
        case awayPosition = "away_position"
        case odds
        case marketCount = "market_count"
        case oddsSource = "odds_source"
        case oddsCapturedAt = "odds_captured_at"
    }
}

public struct DayResponse: Decodable, Sendable {
    public let date: String
    public let timezone: String
    public let matches: [DayMatch]
}
