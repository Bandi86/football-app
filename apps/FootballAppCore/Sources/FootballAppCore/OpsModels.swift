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

public struct OpsStatus: Decodable, Equatable, Sendable {
    public let pendingJobs: Int
    public let leasedJobs: Int
    public let quarantined: Int
    public let pendingReviews: Int
    public let sources: [OpsSource]

    enum CodingKeys: String, CodingKey {
        case pendingJobs = "pending_jobs"
        case leasedJobs = "leased_jobs"
        case quarantined
        case pendingReviews = "pending_reviews"
        case sources
    }
}
