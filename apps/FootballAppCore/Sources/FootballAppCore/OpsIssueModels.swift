import Foundation

// MARK: - Enumok: ismeretlen érték nem dob, mint a PublishState esetén.

public enum IssueSubjectKind: String, Decodable, Equatable, Sendable {
    case match, team, competition, unknown

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = IssueSubjectKind(rawValue: raw) ?? .unknown
    }
}

public enum IssueSeverity: String, Decodable, Equatable, Sendable {
    case info, warning, hard, unknown

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = IssueSeverity(rawValue: raw) ?? .unknown
    }
}

public enum IssueCode: Decodable, Equatable, Sendable {
    case missingLogo, missingOdds, openPast, duplicateSuspect
    case identityReview, staleStatus, sourceConflict
    case other(String)

    public var key: String {
        switch self {
        case .missingLogo: return "missing_logo"
        case .missingOdds: return "missing_odds"
        case .openPast: return "open_past"
        case .duplicateSuspect: return "duplicate_suspect"
        case .identityReview: return "identity_review"
        case .staleStatus: return "stale_status"
        case .sourceConflict: return "source_conflict"
        case .other(let raw): return raw
        }
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = switch raw {
        case "missing_logo": .missingLogo
        case "missing_odds": .missingOdds
        case "open_past": .openPast
        case "duplicate_suspect": .duplicateSuspect
        case "identity_review": .identityReview
        case "stale_status": .staleStatus
        case "source_conflict": .sourceConflict
        default: .other(raw)
        }
    }
}

public enum HistoryOrigin: Decodable, Equatable, Sendable {
    case audit, provenance, other(String)

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = switch raw {
        case "audit": .audit
        case "provenance": .provenance
        default: .other(raw)
        }
    }
}

/// Tetszőleges JSON mező felolvasható szövege (Audit `before`/`after`).
public indirect enum JSONValue: Decodable, Equatable, Sendable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case null
    case array([JSONValue])
    case object([String: JSONValue])

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(Double.self) {
            self = .number(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([JSONValue].self) {
            self = .array(value)
        } else {
            self = .object(try container.decode([String: JSONValue].self))
        }
    }

    /// Magyar felületen használható, rövid megjelenítés.
    public var displayText: String {
        switch self {
        case .string(let value): return value
        case .number(let value):
            return value.formatted(.number.precision(.fractionLength(0...2)))
        case .bool(let value): return value ? "igen" : "nem"
        case .null: return "nincs adat"
        case .array(let values):
            return values.map(\.displayText).joined(separator: ", ")
        case .object(let fields):
            return fields.keys.sorted().map { key in
                "\(key): \(fields[key]?.displayText ?? "nincs adat")"
            }.joined(separator: ", ")
        }
    }
}

// MARK: - Válasz-DTO-k (snake_case ↔ camelCase, mint a DayModels-ben).

public struct IssueRowDTO: Decodable, Equatable, Sendable, Identifiable {
    public let id: UUID
    public let subjectKind: IssueSubjectKind
    public let subjectId: UUID
    public let code: IssueCode
    public let severity: IssueSeverity
    public let firstSeenAt: Date
    public let lastSeenAt: Date
    public let resolvedAt: Date?
    public let source: String

    enum CodingKeys: String, CodingKey {
        case id
        case subjectKind = "subject_kind"
        case subjectId = "subject_id"
        case code, severity
        case firstSeenAt = "first_seen_at"
        case lastSeenAt = "last_seen_at"
        case resolvedAt = "resolved_at"
        case source
    }
}

public struct IssuesResponseDTO: Decodable, Equatable, Sendable {
    public let issues: [IssueRowDTO]
    public let count: Int

    public init(issues: [IssueRowDTO], count: Int) {
        self.issues = issues
        self.count = count
    }
}

/// A hiánylista szűrője; az alapértelmezések megegyeznek a backendével.
public struct IssuesQueryDTO: Equatable, Sendable {
    public var scope: String
    public var date: String?
    public var code: String?
    public var severity: String?
    public var openOnly: Bool

    public init(
        scope: String = "all",
        date: String? = nil,
        code: String? = nil,
        severity: String? = nil,
        openOnly: Bool = false
    ) {
        self.scope = scope
        self.date = date
        self.code = code
        self.severity = severity
        self.openOnly = openOnly
    }
}

public struct MatchNoteDTO: Decodable, Equatable, Sendable, Identifiable {
    public let id: UUID
    public let matchId: UUID
    public let author: String
    public let body: String
    public let kind: String
    public let createdAt: Date
    public let updatedAt: Date
    public let resolvedAt: Date?

    enum CodingKeys: String, CodingKey {
        case id
        case matchId = "match_id"
        case author, body, kind
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case resolvedAt = "resolved_at"
    }
}

public struct HistoryEntryDTO: Decodable, Equatable, Sendable {
    public let at: Date
    public let origin: HistoryOrigin
    public let actor: String
    public let action: String
    public let fieldKey: String?
    public let before: JSONValue?
    public let after: JSONValue?
    public let source: String?

    enum CodingKeys: String, CodingKey {
        case at, origin, actor, action
        case fieldKey = "field_key"
        case before, after, source
    }
}

public struct NotesResponseDTO: Decodable, Equatable, Sendable {
    public let notes: [MatchNoteDTO]
    public let count: Int

    public init(notes: [MatchNoteDTO], count: Int) {
        self.notes = notes
        self.count = count
    }
}

public struct HistoryResponseDTO: Decodable, Equatable, Sendable {
    public let entries: [HistoryEntryDTO]

    public init(entries: [HistoryEntryDTO]) {
        self.entries = entries
    }
}
