import Foundation

public struct CompetitionSummary: Decodable, Equatable, Sendable, Identifiable {
    public let competitionId: UUID
    public let name: String
    public let countryCode: String?
    public let countryName: String?
    public let tier: Int
    public let priority: Int
    public let logo: String?
    public let matches: Int
    public let finished: Int
    public let live: Int
    public let scheduled: Int

    public var id: UUID { competitionId }

    enum CodingKeys: String, CodingKey {
        case competitionId = "competition_id"
        case name
        case countryCode = "country_code"
        case countryName = "country_name"
        case tier, priority, logo, matches, finished, live, scheduled
    }
}

public struct CompetitionInfo: Decodable, Equatable, Sendable {
    public let competitionId: UUID
    public let name: String
    public let countryCode: String?
    public let countryName: String?
    public let tier: Int
    public let priority: Int
    public let logo: String?
    public let seasonLabel: String?
    public let matches: Int
    public let goals: Int
    public let bttsMatches: Int
    public let homeWins: Int
    public let draws: Int
    public let awayWins: Int

    enum CodingKeys: String, CodingKey {
        case competitionId = "competition_id"
        case name
        case countryCode = "country_code"
        case countryName = "country_name"
        case tier, priority, logo
        case seasonLabel = "season_label"
        case matches, goals
        case bttsMatches = "btts_matches"
        case homeWins = "home_wins"
        case draws
        case awayWins = "away_wins"
    }
}

public struct CompetitionTeam: Decodable, Equatable, Sendable, Identifiable {
    public let teamId: UUID
    public let name: String
    public let logo: String?
    public let played: Int
    public let wins: Int
    public let draws: Int
    public let losses: Int
    public let goalsFor: Int
    public let goalsAgainst: Int

    public var id: UUID { teamId }

    enum CodingKeys: String, CodingKey {
        case teamId = "team_id"
        case name, logo, played, wins, draws, losses
        case goalsFor = "goals_for"
        case goalsAgainst = "goals_against"
    }
}

public struct CompetitionDetail: Decodable, Equatable, Sendable {
    public let detail: CompetitionInfo
    public let matches: [DayMatch]
    public let teams: [CompetitionTeam]
}

/// Egy verseny szezonja a kliens szezon-nézethez (S23f).
public struct CompetitionSeason: Decodable, Equatable, Sendable, Identifiable {
    public let label: String
    public let matches: Int
    public let withResult: Int
    public let withOdds: Int

    public var id: String { label }

    enum CodingKeys: String, CodingKey {
        case label, matches
        case withResult = "with_result"
        case withOdds = "with_odds"
    }
}
