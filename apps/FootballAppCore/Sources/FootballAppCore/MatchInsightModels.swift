import Foundation

/// A `GET /v1/match/{id}` research-insight blokkja. A hiány `nil`, nem kitalált 0.
/// A `generated`/`verified` jelzőket a DTO megőrzi — a felület kötelezően jelöli
/// a számított, nem ellenőrzött tartalmat.
public struct MatchInsight: Codable, Equatable, Sendable {
    public let capturedAt: Date
    public let source: String
    public let previewHu: MatchPreview?
    public let prediction: MatchPrediction?
    public let h2hStreaks: H2HStreaks?
    public let standings: StandingPair?
    public let seasonStats: SeasonPair?
    public let injuries: InsightInjuries?
    public let fanPoll: FanPoll?
    public let oddsMarkets: InsightOdds?
    public let venueDetails: InsightVenue?
    /// S181: magyar nyelvű, generált meccsriport — tényként nem jeleníthető meg.
    public let matchReportHu: MatchReport?

    enum CodingKeys: String, CodingKey {
        case capturedAt = "captured_at"
        case source
        case previewHu = "preview_hu"
        case prediction
        case h2hStreaks = "h2h_streaks"
        case standings
        case seasonStats = "season_stats"
        case injuries
        case fanPoll = "fan_poll"
        case oddsMarkets = "odds_markets"
        case venueDetails = "venue_details"
        case matchReportHu = "match_report_hu"
    }
}

public struct MatchReport: Codable, Equatable, Sendable {
    public let title: String?
    public let text: String?
    public let language: String?
    public let generated: Bool?
    public let verified: Bool?
    public let source: String?
    public let generatedAt: Date?

    enum CodingKeys: String, CodingKey {
        case title, text, language, generated, verified, source
        case generatedAt = "generated_at"
    }
}

public struct MatchPreview: Codable, Equatable, Sendable {
    public let title: String?
    public let keyPoints: [String]?
    public let sections: [PreviewSection]?
    public let generated: Bool?
    public let verified: Bool?
    public let source: String?

    enum CodingKeys: String, CodingKey {
        case title
        case keyPoints = "key_points"
        case sections
        case generated, verified, source
    }
}

public struct PreviewSection: Codable, Equatable, Sendable {
    public let heading: String?
    public let text: String?
}

public struct MatchPrediction: Codable, Equatable, Sendable {
    public let model: String?
    public let expectedGoals: PredictionGoals?
    public let winProbabilityPct: PredictionProbabilities?
    public let mostLikelyScore: PredictionScore?
    public let note: String?

    enum CodingKeys: String, CodingKey {
        case model
        case expectedGoals = "expected_goals"
        case winProbabilityPct = "win_probability_pct"
        case mostLikelyScore = "most_likely_score"
        case note
    }
}

public struct PredictionGoals: Codable, Equatable, Sendable {
    public let home: Double?
    public let away: Double?
    public let total: Double?
}

public struct PredictionProbabilities: Codable, Equatable, Sendable {
    public let home: Double?
    public let draw: Double?
    public let away: Double?
}

public struct PredictionScore: Codable, Equatable, Sendable {
    public let home: Int?
    public let away: Int?
    public let probabilityPct: Double?

    enum CodingKeys: String, CodingKey {
        case home, away
        case probabilityPct = "probability_pct"
    }
}

public struct H2HStreaks: Codable, Equatable, Sendable {
    public let home: H2HStreak?
    public let away: H2HStreak?
}

public struct H2HStreak: Codable, Equatable, Sendable {
    public let noWin: Int?
    public let noCleanSheet: Int?

    enum CodingKeys: String, CodingKey {
        case noWin = "no_win"
        case noCleanSheet = "no_clean_sheet"
    }
}

/// A research `injuries` blokk két alakban érkezhet: közvetlen lista, vagy
/// `{home: [...], away: [...]}` térkép. Mindkettőt egy listára simítjuk.
public struct InsightInjuries: Codable, Equatable, Sendable {
    public let items: [MatchInjury]

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let list = try? container.decode([MatchInjury].self) {
            items = list
        } else if let sides = try? container.decode([String: [MatchInjury]].self) {
            items = (sides["home"] ?? []) + (sides["away"] ?? [])
        } else {
            items = []
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(items)
    }
}

public struct MatchInjury: Codable, Equatable, Sendable {
    public let playerId: Int?
    public let name: String?
    public let type: String?
    public let expectedReturn: String?

    enum CodingKeys: String, CodingKey {
        case playerId = "player_id"
        case name, type
        case expectedReturn = "expected_return"
    }
}

public struct StandingPair: Codable, Equatable, Sendable {
    public let home: InsightStanding?
    public let away: InsightStanding?
    public let snapshotAt: Date?
    public let source: String?

    enum CodingKeys: String, CodingKey {
        case home, away
        case snapshotAt = "snapshot_at"
        case source
    }
}

public struct InsightStanding: Codable, Equatable, Sendable {
    public let position: Int?
    public let points: Int?
    public let played: Int?
    public let goalDifference: Int?
    public let record: String?
    public let team: String?

    enum CodingKeys: String, CodingKey {
        case position, points, played, record, team
        case goalDifference = "goal_difference"
    }
}

public struct SeasonPair: Codable, Equatable, Sendable {
    public let home: InsightSeasonStat?
    public let away: InsightSeasonStat?
}

public struct InsightSeasonStat: Codable, Equatable, Sendable {
    public let matches: Int?
    public let wins: Int?
    public let draws: Int?
    public let losses: Int?
    public let goalsFor: Int?
    public let goalsAgainst: Int?
    public let goalsPerGame: Double?
    public let goalsConcededPerGame: Double?
    public let position: Int?
    public let rankLabel: String?
    public let group: String?
    public let source: String?
    /// S221: a research JSON-ban már jelen lévő, de korábban dekódolatlan mezők.
    /// Mind opcionális — a legtöbb forrás ezeket nem adja meg.
    public let goalDifference: Int?
    public let points: Int?
    public let cleanSheets: Int?
    public let assists: Int?
    public let possessionPct: Double?
    public let averageRating: Double?

    enum CodingKeys: String, CodingKey {
        case matches, wins, draws, losses, position, group, source
        case goalDifference = "goal_difference"
        case points
        case cleanSheets = "clean_sheets"
        case assists
        case possessionPct = "possession_pct"
        case averageRating = "average_rating"
        case goalsFor = "goals_for"
        case goalsAgainst = "goals_against"
        case goalsPerGame = "goals_per_game"
        case goalsConcededPerGame = "goals_conceded_per_game"
        case rankLabel = "rank_label"
    }
}

public struct FanPoll: Codable, Equatable, Sendable {
    public let available: Bool?
    public let votes: Int?
    public let questions: [FanPollQuestion]?
}

public struct FanPollQuestion: Codable, Equatable, Sendable {
    public let market: String?
    public let question: String?
    public let fact: String?
}

public struct InsightOdds: Codable, Equatable, Sendable {
    public let marketsSource: String?
    public let observedAt: Date?
    public let byBookmaker: [String: InsightBookmaker]?
    public let marketsRaw: [String: [String: Double]]?

    enum CodingKeys: String, CodingKey {
        case marketsSource = "markets_source"
        case observedAt = "observed_at"
        case byBookmaker = "by_bookmaker"
        case marketsRaw = "markets_raw"
    }
}

public struct InsightBookmaker: Codable, Equatable, Sendable {
    public let markets: [String: [String: Double]]?
    /// A forrás könyvenként eltérő időformátumot használ
    /// (pl. `2026-11-22 06:33:15` időzóna nélkül) — nyers szövegként őrizzük.
    public let observedAt: String?
    public let sources: [String]?

    enum CodingKeys: String, CodingKey {
        case markets
        case observedAt = "observed_at"
        case sources
    }
}

public struct InsightVenue: Codable, Equatable, Sendable {
    public let name: String?
    public let city: String?
    public let country: String?
    public let capacity: Int?
    public let surface: String?
    public let coordinates: InsightCoordinates?
}

public struct InsightCoordinates: Codable, Equatable, Sendable {
    public let lat: Double?
    public let long: Double?
}
