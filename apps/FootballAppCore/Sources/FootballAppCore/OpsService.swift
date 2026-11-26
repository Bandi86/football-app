import Foundation

public protocol OpsService: Sendable {
    func status() async throws -> OpsStatus
    func issues(query: IssuesQueryDTO) async throws -> IssuesResponseDTO
    func history(matchID: UUID, limit: Int) async throws -> HistoryResponseDTO
    func notes(matchID: UUID) async throws -> NotesResponseDTO
    func addNote(matchID: UUID, author: String, body: String, kind: String) async throws -> MatchNoteDTO
    func editNote(id: UUID, body: String, kind: String) async throws -> MatchNoteDTO
    func summary(date: String?) async throws -> OpsSummaryDTO
    func audit(
        limit: Int,
        subjectKind: String?,
        subjectID: UUID?
    ) async throws -> AuditResponseDTO
    func resolveIssue(id: UUID) async throws -> ResolveResponseDTO
}

public struct HTTPOpsService: OpsService {
    private let baseURL: URL
    private let opsToken: String
    private let session: URLSession

    public init(baseURL: URL, opsToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.opsToken = opsToken
        self.session = session
    }

    public func status() async throws -> OpsStatus {
        try await send("GET", "v1/ops/status", query: [], body: nil, as: OpsStatus.self)
    }

    public func issues(query: IssuesQueryDTO) async throws -> IssuesResponseDTO {
        var items = [URLQueryItem(name: "scope", value: query.scope)]
        if let date = query.date, !date.isEmpty {
            items.append(URLQueryItem(name: "date", value: date))
        }
        if let code = query.code, !code.isEmpty {
            items.append(URLQueryItem(name: "code", value: code))
        }
        if let severity = query.severity, !severity.isEmpty {
            items.append(URLQueryItem(name: "severity", value: severity))
        }
        if query.openOnly {
            items.append(URLQueryItem(name: "open_only", value: "true"))
        }
        return try await send("GET", "v1/issues", query: items, body: nil, as: IssuesResponseDTO.self)
    }

    public func history(matchID: UUID, limit: Int) async throws -> HistoryResponseDTO {
        try await send(
            "GET",
            "v1/match/\(matchID.uuidString)/history",
            query: [URLQueryItem(name: "limit", value: String(max(1, min(limit, 200))))],
            body: nil,
            as: HistoryResponseDTO.self
        )
    }

    public func notes(matchID: UUID) async throws -> NotesResponseDTO {
        try await send(
            "GET",
            "v1/match/\(matchID.uuidString)/notes",
            query: [],
            body: nil,
            as: NotesResponseDTO.self
        )
    }

    public func addNote(
        matchID: UUID,
        author: String,
        body: String,
        kind: String
    ) async throws -> MatchNoteDTO {
        let payload = try JSONSerialization.data(withJSONObject: [
            "author": author, "body": body, "kind": kind,
        ])
        return try await send(
            "POST",
            "v1/match/\(matchID.uuidString)/notes",
            query: [],
            body: payload,
            as: MatchNoteDTO.self
        )
    }

    public func editNote(id: UUID, body: String, kind: String) async throws -> MatchNoteDTO {
        let payload = try JSONSerialization.data(withJSONObject: ["body": body, "kind": kind])
        return try await send(
            "PATCH",
            "v1/note/\(id.uuidString)",
            query: [],
            body: payload,
            as: MatchNoteDTO.self
        )
    }

    public func summary(date: String?) async throws -> OpsSummaryDTO {
        var query: [URLQueryItem] = []
        if let date, !date.isEmpty {
            query.append(URLQueryItem(name: "date", value: date))
        }
        return try await send("GET", "v1/ops/summary", query: query, body: nil, as: OpsSummaryDTO.self)
    }

    public func audit(
        limit: Int,
        subjectKind: String?,
        subjectID: UUID?
    ) async throws -> AuditResponseDTO {
        var query = [URLQueryItem(name: "limit", value: String(max(1, min(limit, 200))))]
        if let subjectKind, !subjectKind.isEmpty {
            query.append(URLQueryItem(name: "subject_kind", value: subjectKind))
        }
        if let subjectID {
            query.append(URLQueryItem(name: "subject_id", value: subjectID.uuidString))
        }
        return try await send("GET", "v1/ops/audit", query: query, body: nil, as: AuditResponseDTO.self)
    }

    public func resolveIssue(id: UUID) async throws -> ResolveResponseDTO {
        try await send(
            "POST",
            "v1/issue/\(id.uuidString)/resolve",
            query: [],
            body: nil,
            as: ResolveResponseDTO.self
        )
    }

    private func send<T: Decodable>(
        _ method: String,
        _ path: String,
        query: [URLQueryItem],
        body: Data?,
        as type: T.Type
    ) async throws -> T {
        var url = baseURL.appendingPathComponent(path)
        if !query.isEmpty {
            url.append(queryItems: query)
        }
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("Bearer \(opsToken)", forHTTPHeaderField: "Authorization")
        if body != nil {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = body
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch is CancellationError {
            throw CancellationError()
        } catch let urlError as URLError where urlError.code == .cancelled {
            throw CancellationError()
        } catch {
            throw APIError.network
        }
        guard let http = response as? HTTPURLResponse else {
            throw APIError.network
        }
        switch http.statusCode {
        case 200...299:
            do {
                return try APIDecoding.makeDecoder().decode(type, from: data)
            } catch {
                throw APIError.decoding
            }
        case 401:
            throw APIError.unauthorized
        case 400:
            throw APIError.badRequest
        case 404:
            throw APIError.notFound
        default:
            throw APIError.server
        }
    }
}
