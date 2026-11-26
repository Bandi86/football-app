import Foundation

/// A három meccs-szintű mikro-végpont read tokennel, párhuzamosan.
public protocol MatchMicroService: Sendable {
    func load(matchId: UUID) async throws -> MatchMicro
}

public struct HTTPMatchMicroService: MatchMicroService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession

    public init(baseURL: URL, readToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
    }

    public func load(matchId: UUID) async throws -> MatchMicro {
        async let lineups: LineupsPayload = fetch("lineups", matchId: matchId)
        async let stats: PlayerStatsPayload = fetch("player-stats", matchId: matchId)
        async let shots: ShotsPayload = fetch("shots", matchId: matchId)
        return MatchMicro(
            lineups: try await lineups.lineups,
            playerStats: try await stats.playerStats,
            shots: try await shots.shots
        )
    }

    private func fetch<T: Decodable>(_ suffix: String, matchId: UUID) async throws -> T {
        var request = URLRequest(
            url: baseURL.appendingPathComponent("v1/match/\(matchId.uuidString.lowercased())/\(suffix)")
        )
        request.setValue("Bearer \(readToken)", forHTTPHeaderField: "Authorization")
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw APIError.network }
        switch http.statusCode {
        case 200...299:
            do { return try APIDecoding.makeDecoder().decode(T.self, from: data) }
            catch { throw APIError.decoding }
        case 401: throw APIError.unauthorized
        case 404: throw APIError.notFound
        case 400: throw APIError.badRequest
        default: throw APIError.server
        }
    }
}
