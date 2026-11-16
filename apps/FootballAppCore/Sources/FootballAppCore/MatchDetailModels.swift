import Foundation

/// Egy kanonikus meccs teljes, ellenőrzött képe (S10).
public struct MatchDetail: Decodable, Equatable, Sendable {
    public let id: UUID
    public let kickoffAt: Date
    public let kickoffPrecision: String
    public let status: String
    public let roundLabel: String?
    public let competitionName: String
    public let countryCode: String?
    public let countryName: String?
    public let tier: Int
    public let priority: Int
    public let seasonLabel: String
    public let homeName: String
    public let awayName: String
    public let homeLogo: String?
    public let awayLogo: String?
    public let homeElo: Double?
    public let awayElo: Double?
    public let homeFt: Int?
    public let awayFt: Int?
    public let homeHt: Int?
    public let awayHt: Int?
    public let homeEt: Int?
    public let awayEt: Int?
    public let homePens: Int?
    public let awayPens: Int?
    public let decidedBy: String?
    public let referee: String?
    public let statsSource: String?
    public let statsCapturedAt: Date?
    public let homeShots: Int?
    public let awayShots: Int?
    public let homeShotsOnTarget: Int?
    public let awayShotsOnTarget: Int?
    public let homeCorners: Int?
    public let awayCorners: Int?
    public let homeYellow: Int?
    public let awayYellow: Int?
    public let homeRed: Int?
    public let awayRed: Int?
    public let homeFouls: Int?
    public let awayFouls: Int?
    public let homeXg: Double?
    public let awayXg: Double?
    public let odds: OddsSnapshot?
    public let marketCount: Int?
    public let oddsSource: String?
    public let oddsCapturedAt: Date?
    public let oddsTrustState: String?
    public let gateState: PublishState?
    public let gateBlockers: [Blocker]
    public let gateEvaluatedAt: Date?
    public let gateRuleSetVersion: String?
    public let publishDate: String?

    enum CodingKeys: String, CodingKey {
        case id
        case kickoffAt = "kickoff_at"
        case kickoffPrecision = "kickoff_precision"
        case status
        case roundLabel = "round_label"
        case competitionName = "competition_name"
        case countryCode = "country_code"
        case countryName = "country_name"
        case tier, priority
        case seasonLabel = "season_label"
        case homeName = "home_name"
        case awayName = "away_name"
        case homeLogo = "home_logo"
        case awayLogo = "away_logo"
        case homeElo = "home_elo"
        case awayElo = "away_elo"
        case homeFt = "home_ft"
        case awayFt = "away_ft"
        case homeHt = "home_ht"
        case awayHt = "away_ht"
        case homeEt = "home_et"
        case awayEt = "away_et"
        case homePens = "home_pens"
        case awayPens = "away_pens"
        case decidedBy = "decided_by"
        case referee
        case statsSource = "stats_source"
        case statsCapturedAt = "stats_captured_at"
        case homeShots = "home_shots"
        case awayShots = "away_shots"
        case homeShotsOnTarget = "home_shots_on_target"
        case awayShotsOnTarget = "away_shots_on_target"
        case homeCorners = "home_corners"
        case awayCorners = "away_corners"
        case homeYellow = "home_yellow"
        case awayYellow = "away_yellow"
        case homeRed = "home_red"
        case awayRed = "away_red"
        case homeFouls = "home_fouls"
        case awayFouls = "away_fouls"
        case homeXg = "home_xg"
        case awayXg = "away_xg"
        case odds
        case marketCount = "market_count"
        case oddsSource = "odds_source"
        case oddsCapturedAt = "odds_captured_at"
        case oddsTrustState = "odds_trust_state"
        case gateState = "gate_state"
        case gateBlockers = "gate_blockers"
        case gateEvaluatedAt = "gate_evaluated_at"
        case gateRuleSetVersion = "gate_rule_set_version"
        case publishDate = "publish_date"
    }
}
