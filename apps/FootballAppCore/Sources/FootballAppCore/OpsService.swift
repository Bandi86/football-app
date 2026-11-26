import Foundation

public protocol OpsService: Sendable {
    func status() async throws -> OpsStatus
    func issues(query: IssuesQueryDTO) async throws -> IssuesResponseDTO
    func history(matchID: UUID, limit: Int) async throws -> HistoryResponseDTO
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
        try await get("v1/ops/status", query: [], as: OpsStatus.self)
    }

    public func issues(query: IssuesQueryDTO) async throws -> IssuesResponseDTO {
        var items = [
            URLQueryItem(name: "scope", value: query.scope),
        ]
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
        return try await get("v1/issues", query: items, as: IssuesResponseDTO.self)
    }

    public func history(matchID: UUID, limit: Int) async throws -> HistoryResponseDTO {
        try await get(
            "v1/match/\(matchID.uuidString)/history",
            query: [URLQueryItem(name: "limit", value: String(max(1, min(limit, 200))))],
            as: HistoryResponseDTO.self
        )
    }

    private func get<T: Decodable>(
        _ path: String,
        query: [URLQueryItem],
        as type: T.Type
    ) async throws -> T {
        var url = baseURL.appendingPathComponent(path)
        if !query.isEmpty {
            url.append(queryItems: query)
        }
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(opsToken)", forHTTPHeaderField: "Authorization")

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
        default:
            throw APIError.server
        }
    }
}
