import Foundation

/// A shadow predikció read tokennel, a fő meccsadattól függetlenül.
public protocol MatchPredictionService: Sendable {
    func load(matchId: UUID) async throws -> PredictionResponseDTO
}

public struct HTTPMatchPredictionService: MatchPredictionService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession

    public init(baseURL: URL, readToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
    }

    public func load(matchId: UUID) async throws -> PredictionResponseDTO {
        var request = URLRequest(
            url: baseURL.appendingPathComponent("v1/match/\(matchId.uuidString.lowercased())/prediction")
        )
        request.setValue("Bearer \(readToken)", forHTTPHeaderField: "Authorization")
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw APIError.network }
        switch http.statusCode {
        case 200...299:
            do { return try APIDecoding.makeDecoder().decode(PredictionResponseDTO.self, from: data) }
            catch { throw APIError.decoding }
        case 401: throw APIError.unauthorized
        case 404: throw APIError.notFound
        case 400: throw APIError.badRequest
        default: throw APIError.server
        }
    }
}
