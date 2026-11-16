import Foundation

public protocol TeamService: Sendable {
    func teams(date: Date) async throws -> [TeamSummary]
    func detail(id: UUID, date: Date, window: Int) async throws -> TeamDetail
}

public struct HTTPTeamService: TeamService {
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

    public func teams(date: Date) async throws -> [TeamSummary] {
        let dateString = HTTPDayService.formatDate(date, calendar: calendar)
        let url = baseURL.appendingPathComponent("v1/teams")
            .appending(queryItems: [URLQueryItem(name: "date", value: dateString)])
        let data = try await get(url)
        do {
            return try APIDecoding.makeDecoder().decode([TeamSummary].self, from: data)
        } catch {
            throw APIError.decoding
        }
    }

    public func detail(id: UUID, date: Date, window: Int) async throws -> TeamDetail {
        let dateString = HTTPDayService.formatDate(date, calendar: calendar)
        let url = baseURL.appendingPathComponent("v1/team/\(id.uuidString.lowercased())")
            .appending(queryItems: [
                URLQueryItem(name: "date", value: dateString),
                URLQueryItem(name: "window", value: String(window)),
            ])
        let data = try await get(url)
        do {
            return try APIDecoding.makeDecoder().decode(TeamDetail.self, from: data)
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
