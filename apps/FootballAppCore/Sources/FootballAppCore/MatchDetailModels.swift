import Foundation

/// Egy kanonikus meccs teljes, ellenőrzött képe (S10).
public struct MatchDetail: Decodable, Equatable, Sendable {
    public let id: UUID
    public let kickoffAt: Date
    public let kickoffPrecision: String
    /// A verseny időzónája (IANA név), amiben a kezdés helyi ideje értendő.
    /// Régi válaszokból hiányozhat, ezért opcionális.
    public let competitionTimezone: String?
    public let status: String
    public let roundLabel: String?
    /// A verseny kanonikus azonosítója (a részletező verseny-hivatkozásához).
    /// A backend mindig küldi; a régi fixture-ökből és a korábbi válaszokból
    /// hiányozhat, ezért opcionális (a szerződés-drift elleni kulcshalmaz-teszt
    /// ettől még a mezőt elvárja a válaszban).
    public let competitionId: UUID?
    /// A két csapat kanonikus azonosítója (entitáslinkekhez). Régi válaszokból
    /// hiányozhat, ezért opcionális; a kontrakt-kulcshalmaz teszt mégis elvárja.
    public let homeTeamId: UUID?
    public let awayTeamId: UUID?
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
    /// A meccs helyszíne (stadion) — új backend-mező, régi válaszokból
    /// hiányozhat, ezért opcionális.
    public let venue: VenueInfo?
    /// Az időjárás a kezdés helyszínén — új backend-mező, régi válaszokból
    /// hiányozhat, ezért opcionális.
    public let weather: WeatherInfo?
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
    /// Az utolsó öt lejátszott meccs (a meccs kezdete előtt) — opcionális, hogy
    /// a régi fixture-ök és a minta is dekódolható maradjon.
    public let homeForm: [FormEntry]?
    public let awayForm: [FormEntry]?
    public let headToHead: [HeadToHeadEntry]?
    /// S221: a H2H-listából számolt összesítő (coverage mezővel).
    public let h2hSummary: H2HSummary?
    /// A két csapat következő meccsei (legfeljebb 3-3).
    public let homeNext: [NextMatchEntry]?
    public let awayNext: [NextMatchEntry]?
    /// A csapatok helyezése a verseny aktuális tabelláján (ha van).
    public let homePosition: Int?
    public let awayPosition: Int?
    /// A tabella kivágása a két csapat helyezése körül.
    public let tableSlice: [CompetitionStanding]?
    /// A meccs eseményeinek idővonala (perc szerint rendezve). Régi válaszokból
    /// hiányozhat, ezért opcionális; a nézet a hiányt üres listaként kezeli.
    public let incidents: [MatchIncident]?
    /// S113: research gazdag szekciói. Régi válaszokban a kulcs hiányozhat.
    public let insight: MatchInsight?
    /// S181: a forrásból eltárolt kattintható linkek; régi válaszokból hiányozhat.
    public let links: [MatchLink]?

    enum CodingKeys: String, CodingKey, CaseIterable {
        case id
        case kickoffAt = "kickoff_at"
        case kickoffPrecision = "kickoff_precision"
        case competitionTimezone = "competition_timezone"
        case status
        case roundLabel = "round_label"
        case competitionId = "competition_id"
        case homeTeamId = "home_team_id"
        case awayTeamId = "away_team_id"
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
        case venue
        case weather
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
        case homeForm = "home_form"
        case awayForm = "away_form"
        case headToHead = "head_to_head"
        case h2hSummary = "h2h_summary"
        case homeNext = "home_next"
        case awayNext = "away_next"
        case homePosition = "home_position"
        case awayPosition = "away_position"
        case tableSlice = "table_slice"
        case incidents
        case insight
        case links
    }
}

/// Egy meccsesemény az idővonalon. A szerződés szerint minden mező a
/// `minute`/`incident_type` kivételével hiányozhat; az ismeretlen
/// `incident_type` értéket nem elutasítjuk, hanem `.other` kategóriaként
/// jelenítjük meg, hogy egy új forrás-esemény se törje el a dekódolást.
public struct MatchIncident: Decodable, Equatable, Sendable {
    public let minute: Int?
    public let incidentType: String
    public let side: String?
    public let playerName: String?
    public let relatedPlayerName: String?
    public let detail: String?
    public let scoreAfterHome: Int?
    public let scoreAfterAway: Int?

    enum CodingKeys: String, CodingKey {
        case minute
        case incidentType = "incident_type"
        case side
        case playerName = "player_name"
        case relatedPlayerName = "related_player_name"
        case detail
        case scoreAfterHome = "score_after_home"
        case scoreAfterAway = "score_after_away"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        minute = try container.decodeIfPresent(Int.self, forKey: .minute)
        incidentType = try container.decodeIfPresent(String.self, forKey: .incidentType) ?? ""
        side = try container.decodeIfPresent(String.self, forKey: .side)
        playerName = try container.decodeIfPresent(String.self, forKey: .playerName)
        relatedPlayerName = try container.decodeIfPresent(String.self, forKey: .relatedPlayerName)
        detail = try container.decodeIfPresent(String.self, forKey: .detail)
        scoreAfterHome = try container.decodeIfPresent(Int.self, forKey: .scoreAfterHome)
        scoreAfterAway = try container.decodeIfPresent(Int.self, forKey: .scoreAfterAway)
    }

    /// A nyers `side` érték: a szerződés a `home`/`away` értékeket használja,
    /// de ismeretlen érték esetén sem dobunk, csak semlegesnek kezeljük.
    public var isHome: Bool { side == "home" }
    public var isAway: Bool { side == "away" }

    /// Az esemény ismert kategóriája; az ismeretlen forrás-stringet megőrzi.
    public var kind: MatchIncidentKind { MatchIncidentKind(rawType: incidentType) }
}

/// Az idővonalon ismert eseménytípusok. Az ismeretlen értéket az `.other`
/// hordozza, így a megjelenítés és a felolvasás is tudja kezelni.
public enum MatchIncidentKind: Equatable, Sendable {
    case goal
    case card
    case substitution
    case half
    case addedTime
    case varReview
    case missedPenalty
    case penaltyShootout
    case other(String)

    public init(rawType: String) {
        switch rawType.lowercased() {
        case "goal": self = .goal
        case "card": self = .card
        case "substitution": self = .substitution
        case "half": self = .half
        case "addedtime": self = .addedTime
        case "var": self = .varReview
        case "missedpenalty": self = .missedPenalty
        case "penaltyshootout": self = .penaltyShootout
        default: self = .other(rawType)
        }
    }
}

/// Egy soron következő meccs a csapat szemszögéből.
public struct NextMatchEntry: Decodable, Equatable, Sendable, Identifiable {
    public let matchId: UUID
    public let kickoffAt: Date
    public let competitionName: String
    public let opponentName: String
    public let isHome: Bool
    /// S221: a következő meccs két csapatának címere (hiány `nil`).
    public let homeLogo: String?
    public let awayLogo: String?

    public var id: UUID { matchId }

    enum CodingKeys: String, CodingKey {
        case matchId = "match_id"
        case kickoffAt = "kickoff_at"
        case competitionName = "competition_name"
        case opponentName = "opponent_name"
        case isHome = "is_home"
        case homeLogo = "home_logo"
        case awayLogo = "away_logo"
    }
}

/// Egy korábbi meccs a forma-mutatóhoz (a csapat szemszögéből).
public struct FormEntry: Decodable, Equatable, Sendable, Identifiable {
    public let matchId: UUID
    public let kickoffAt: Date
    public let competitionName: String
    public let opponentName: String
    public let isHome: Bool
    public let teamGoals: Int
    public let opponentGoals: Int
    /// "W" | "D" | "L"
    public let outcome: String

    public var id: UUID { matchId }

    enum CodingKeys: String, CodingKey {
        case matchId = "match_id"
        case kickoffAt = "kickoff_at"
        case competitionName = "competition_name"
        case opponentName = "opponent_name"
        case isHome = "is_home"
        case teamGoals = "team_goals"
        case opponentGoals = "opponent_goals"
        case outcome
    }
}

/// Egy korábbi egymás elleni meccs.
public struct HeadToHeadEntry: Decodable, Equatable, Sendable, Identifiable {
    public let matchId: UUID
    public let kickoffAt: Date
    public let competitionName: String
    public let homeName: String
    public let awayName: String
    public let homeGoals: Int?
    public let awayGoals: Int?
    /// S221: opcionális meccs-szintű mutatók; hiány `nil`, nem 0.
    public let homeCorners: Int?
    public let awayCorners: Int?
    public let homeYellow: Int?
    public let awayYellow: Int?
    public let homeRed: Int?
    public let awayRed: Int?
    public let homeXg: Double?
    public let awayXg: Double?

    public var id: UUID { matchId }

    enum CodingKeys: String, CodingKey {
        case matchId = "match_id"
        case kickoffAt = "kickoff_at"
        case competitionName = "competition_name"
        case homeName = "home_name"
        case awayName = "away_name"
        case homeGoals = "home_goals"
        case awayGoals = "away_goals"
        case homeCorners = "home_corners"
        case awayCorners = "away_corners"
        case homeYellow = "home_yellow"
        case awayYellow = "away_yellow"
        case homeRed = "home_red"
        case awayRed = "away_red"
        case homeXg = "home_xg"
        case awayXg = "away_xg"
    }
}

/// S221: az egymás elleni meccsekből számolt összesítő. A számok csak akkor
/// jelennek meg, ha a backendnek volt miből számolnia (`with_stats`).
public struct H2HSummary: Decodable, Equatable, Sendable {
    public let matches: Int
    public let withStats: Int
    public let totalGoals: Int?
    public let bttsPct: Double?
    public let over25Pct: Double?
    public let cornersAvg: Double?
    public let cardsAvg: Double?

    enum CodingKeys: String, CodingKey {
        case matches
        case withStats = "with_stats"
        case totalGoals = "total_goals"
        case bttsPct = "btts_pct"
        case over25Pct = "over25_pct"
        case cornersAvg = "corners_avg"
        case cardsAvg = "cards_avg"
    }
}

/// Egy kattintható, forrásból származó URL a meccshez. A felhasználó nem
/// adhat meg URL-t; a `Link` csak http(s) sémát engedélyez a nézetben.
public struct MatchLink: Codable, Equatable, Sendable, Identifiable {
    public let kind: String
    public let label: String
    public let url: String
    public let capturedAt: Date

    public var id: String { "\(kind)|\(url)" }

    enum CodingKeys: String, CodingKey {
        case kind, label, url
        case capturedAt = "captured_at"
    }
}
