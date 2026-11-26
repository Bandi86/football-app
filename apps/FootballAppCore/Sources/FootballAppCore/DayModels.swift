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
    // A piacok hiányozhatnak (pl. footballdata BTTS nélkül) → opcionális.
    public let home: Double?
    public let draw: Double?
    public let away: Double?
    public let bttsYes: Double?
    public let bttsNo: Double?
    public let over25: Double?
    public let under25: Double?

    /// Az odds-szekció csak akkor érhető el, ha van legalább egy megjeleníthető
    /// piac — az üres objektum nem számít adatnak.
    public var hasAnyValue: Bool {
        [home, draw, away, bttsYes, bttsNo, over25, under25].contains { $0 != nil }
    }

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
    /// „exact" vagy „date_only": utóbbinál a kezdés ideje still placeholder,
    /// ezért a felület nem mutat időt. Régi válaszokban hiányozhat.
    public let kickoffPrecision: String?
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
    /// A forduló nyers címkéje (pl. „17", „1/8", „final").
    public let roundLabel: String?
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
    public let oddsTrustState: String?
    public let homeLogo: String?
    public let awayLogo: String?
    public let competitionLogo: String?
    public let homeElo: Double?
    public let awayElo: Double?
    /// Élő meccs játékideje percekben a research pillanatképből (S134).
    /// Régi válaszokban hiányozhat — ilyenkor a kliens a kezdésből becsül.
    public let minute: Int?

    enum CodingKeys: String, CodingKey {
        case id
        case kickoffAt = "kickoff_at"
        case kickoffPrecision = "kickoff_precision"
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
        case roundLabel = "round_label"
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
        case oddsTrustState = "odds_trust_state"
        case homeLogo = "home_logo"
        case awayLogo = "away_logo"
        case competitionLogo = "competition_logo"
        case homeElo = "home_elo"
        case awayElo = "away_elo"
        case minute
    }
}

/// Napi hiány-mutatók (a nap adatminőségének áttekintéséhez).
public struct DayGaps: Decodable, Equatable, Sendable {
    public let missingKickoff: Int
    public let openPast: Int
    public let missingResult: Int
    public let missingTeamLogo: Int
    public let missingCompetitionLogo: Int
    public let missingOdds: Int
    public let withheld: Int

    public init(missingKickoff: Int = 0, openPast: Int = 0, missingResult: Int = 0,
                missingTeamLogo: Int = 0, missingCompetitionLogo: Int = 0,
                missingOdds: Int = 0, withheld: Int = 0) {
        self.missingKickoff = missingKickoff
        self.openPast = openPast
        self.missingResult = missingResult
        self.missingTeamLogo = missingTeamLogo
        self.missingCompetitionLogo = missingCompetitionLogo
        self.missingOdds = missingOdds
        self.withheld = withheld
    }

    public var isEmpty: Bool {
        missingKickoff + openPast + missingResult + missingTeamLogo
            + missingCompetitionLogo + missingOdds + withheld == 0
    }

    enum CodingKeys: String, CodingKey {
        case missingKickoff = "missing_kickoff"
        case openPast = "open_past"
        case missingResult = "missing_result"
        case missingTeamLogo = "missing_team_logo"
        case missingCompetitionLogo = "missing_competition_logo"
        case missingOdds = "missing_odds"
        case withheld
    }
}

public struct DayResponse: Decodable, Sendable {
    public let date: String
    public let timezone: String
    public let refreshedAt: Date?
    /// A következő tényleges adatfrissítés (research napi slot), UTC-ben.
    public let nextRefreshAt: Date?
    public let gaps: DayGaps?
    public let matches: [DayMatch]

    enum CodingKeys: String, CodingKey {
        case date, timezone, matches, gaps
        case refreshedAt = "refreshed_at"
        case nextRefreshAt = "next_refresh_at"
    }
}

/// Egy nap teljes válasza: a batch-frissítés ideje + a meccsek.
public struct DayFeed: Equatable, Sendable {
    public let refreshedAt: Date?
    /// A következő tényleges adatfrissítés (research napi slot), UTC-ben.
    public let nextRefreshAt: Date?
    public let gaps: DayGaps?
    public let matches: [DayMatch]

    public init(refreshedAt: Date?, nextRefreshAt: Date? = nil, gaps: DayGaps? = nil, matches: [DayMatch]) {
        self.refreshedAt = refreshedAt
        self.nextRefreshAt = nextRefreshAt
        self.gaps = gaps
        self.matches = matches
    }
}
