import Foundation

public struct IssueCodeCountDTO: Decodable, Equatable, Sendable {
    public let code: String
    public let labelHu: String
    public let count: Int
    public let hard: Int

    enum CodingKeys: String, CodingKey {
        case code
        case labelHu = "label_hu"
        case count, hard
    }
}

public struct OpsActionDTO: Decodable, Equatable, Sendable, Identifiable {
    public let kind: String
    public let labelHu: String
    public let issueId: UUID
    public let subjectKind: String
    public let subjectId: UUID

    public var id: UUID { issueId }

    enum CodingKeys: String, CodingKey {
        case kind
        case labelHu = "label_hu"
        case issueId = "issue_id"
        case subjectKind = "subject_kind"
        case subjectId = "subject_id"
    }
}

public struct OpsSummaryDTO: Decodable, Equatable, Sendable {
    public let date: String
    public let openTotal: Int
    public let previousDayTotal: Int
    public let byCode: [IssueCodeCountDTO]
    public let topActions: [OpsActionDTO]
    public let headlineHu: String

    enum CodingKeys: String, CodingKey {
        case date
        case openTotal = "open_total"
        case previousDayTotal = "previous_day_total"
        case byCode = "by_code"
        case topActions = "top_actions"
        case headlineHu = "headline_hu"
    }
}

public struct AuditEntryRowDTO: Decodable, Equatable, Sendable, Identifiable {
    public let id: UUID
    public let actor: String
    public let action: String
    public let subjectKind: String
    public let subjectId: UUID?
    public let createdAt: Date

    enum CodingKeys: String, CodingKey {
        case id, actor, action
        case subjectKind = "subject_kind"
        case subjectId = "subject_id"
        case createdAt = "created_at"
    }
}

public struct AuditResponseDTO: Decodable, Equatable, Sendable {
    public let entries: [AuditEntryRowDTO]
    public let count: Int
}

public struct ResolveResponseDTO: Decodable, Equatable, Sendable {
    public let issueId: UUID
    public let resolved: Bool

    enum CodingKeys: String, CodingKey {
        case issueId = "issue_id"
        case resolved
    }
}
