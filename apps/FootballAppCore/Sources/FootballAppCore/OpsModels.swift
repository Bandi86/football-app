import Foundation

public struct OpsSource: Decodable, Equatable, Sendable, Identifiable {
    public let sourceSlug: String
    public let mode: String
    public let enabled: Bool
    public let attemptsToday: Int
    public let dailyBudget: Int
    public let breakerState: String

    public var id: String { sourceSlug }

    enum CodingKeys: String, CodingKey {
        case sourceSlug = "source_slug"
        case mode
        case enabled
        case attemptsToday = "attempts_today"
        case dailyBudget = "daily_budget"
        case breakerState = "breaker_state"
    }
}

public struct OpsIntegrity: Decodable, Equatable, Sendable {
    public let verdict: String
    public let duplicateKickoffGroups: Int
    public let duplicateFixtureGroups: Int
    public let orphanProvenance: Int
    public let activeRefsToMerged: Int
    public let pendingReviews: Int
    public let withheldGates: Int
    public let failedJobs: Int
    public let openBreakers: Int

    enum CodingKeys: String, CodingKey {
        case verdict
        case duplicateKickoffGroups = "duplicate_kickoff_groups"
        case duplicateFixtureGroups = "duplicate_fixture_groups"
        case orphanProvenance = "orphan_provenance"
        case activeRefsToMerged = "active_refs_to_merged"
        case pendingReviews = "pending_reviews"
        case withheldGates = "withheld_gates"
        case failedJobs = "failed_jobs"
        case openBreakers = "open_breakers"
    }
}

public struct OpsStatus: Decodable, Equatable, Sendable {
    public let pendingJobs: Int
    public let leasedJobs: Int
    public let quarantined: Int
    public let pendingReviews: Int
    public let sources: [OpsSource]
    public let integrity: OpsIntegrity?

    enum CodingKeys: String, CodingKey {
        case pendingJobs = "pending_jobs"
        case leasedJobs = "leased_jobs"
        case quarantined
        case pendingReviews = "pending_reviews"
        case sources
        case integrity
    }
}
