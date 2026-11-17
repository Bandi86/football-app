import Foundation

public protocol CompetitionService: Sendable {
    func competitions(date: Date) async throws -> [CompetitionSummary]
    func detail(id: UUID, date: Date, window: Int) async throws -> CompetitionDetail
    func seasons(id: UUID) async throws -> [CompetitionSeason]
}

public struct HTTPCompetitionService: CompetitionService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession
    private let calendar: Calendar

    public init(
        baseURL: URL,
        readToken: String,
        session: URLSession = .shared,
        calendar: Calendar = Calendar(identifier: .gregorian)
    ) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
        self.calendar = calendar
    }

    public func competitions(date: Date) async throws -> [CompetitionSummary] {
        let dateString = HTTPDayService.formatDate(date, calendar: calendar)
        let url = baseURL.appendingPathComponent("v1/competitions")
            .appending(queryItems: [URLQueryItem(name: "date", value: dateString)])
        let data = try await get(url)
        do {
            return try APIDecoding.makeDecoder().decode([CompetitionSummary].self, from: data)
        } catch {
            throw APIError.decoding
        }
    }

    public func detail(id: UUID, date: Date, window: Int) async throws -> CompetitionDetail {
        let dateString = HTTPDayService.formatDate(date, calendar: calendar)
        let url = baseURL.appendingPathComponent("v1/competition/\(id.uuidString.lowercased())")
            .appending(queryItems: [
                URLQueryItem(name: "date", value: dateString),
                URLQueryItem(name: "window", value: String(window)),
            ])
        let data = try await get(url)
        do {
            return try APIDecoding.makeDecoder().decode(CompetitionDetail.self, from: data)
        } catch {
            throw APIError.decoding
        }
    }

    public func seasons(id: UUID) async throws -> [CompetitionSeason] {
        let url = baseURL.appendingPathComponent(
            "v1/competition/\(id.uuidString.lowercased())/seasons"
        )
        let data = try await get(url)
        do {
            return try APIDecoding.makeDecoder().decode([CompetitionSeason].self, from: data)
        } catch {
            throw APIError.decoding
        }
    }

    private func get(_ url: URL) async throws -> Data {
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(readToken)", forHTTPHeaderField: "Authorization")
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
            return data
        case 401:
            throw APIError.unauthorized
        case 404:
            throw APIError.notFound
        case 400:
            throw APIError.badRequest
        default:
            throw APIError.server
        }
    }
}
