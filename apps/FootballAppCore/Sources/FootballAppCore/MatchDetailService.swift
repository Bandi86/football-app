import Foundation

public protocol MatchDetailService: Sendable {
    func detail(for matchId: UUID) async throws -> MatchDetail
}

public struct HTTPMatchDetailService: MatchDetailService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession

    public init(baseURL: URL, readToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
    }

    public func detail(for matchId: UUID) async throws -> MatchDetail {
        var request = URLRequest(
            url: baseURL.appendingPathComponent("v1/match/\(matchId.uuidString.lowercased())")
        )
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
            do {
                return try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)
            } catch {
                throw APIError.decoding
            }
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
