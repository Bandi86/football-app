import Foundation

/// Kézi eredménybevitel kérése — a gólszám és az indoklás kötelező.
public struct ManualResultRequest: Encodable, Sendable {
    public let homeFt: Int
    public let awayFt: Int
    public let note: String

    public init(homeFt: Int, awayFt: Int, note: String) {
        self.homeFt = homeFt
        self.awayFt = awayFt
        self.note = note
    }

    private enum CodingKeys: String, CodingKey {
        case homeFt = "home_ft"
        case awayFt = "away_ft"
        case note
    }
}

/// A szerver válasza: a frissített kapuállapot és a bizonyíték azonosítói.
public struct ManualResultResponse: Decodable, Sendable {
    public let matchId: UUID
    public let observationId: UUID
    public let auditId: UUID
    public let state: String

    private enum CodingKeys: String, CodingKey {
        case matchId = "match_id"
        case observationId = "observation_id"
        case auditId = "audit_id"
        case state
    }
}

public protocol ManualEntryService: Sendable {
    func submitResult(matchId: UUID, request: ManualResultRequest) async throws -> ManualResultResponse
}

/// A kézi írást az ops tokennel végzi — ugyanazzal, amit az Üzemeltetés nézet
/// használ. A read token nem jogosít írásra.
public struct HTTPManualEntryService: ManualEntryService {
    private let baseURL: URL
    private let opsToken: String
    private let session: URLSession

    public init(baseURL: URL, opsToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.opsToken = opsToken
        self.session = session
    }

    public func submitResult(matchId: UUID, request: ManualResultRequest) async throws -> ManualResultResponse {
        var urlRequest = URLRequest(
            url: baseURL.appendingPathComponent("v1/match/\(matchId.uuidString.lowercased())/manual-result")
        )
        urlRequest.httpMethod = "PUT"
        urlRequest.setValue("Bearer \(opsToken)", forHTTPHeaderField: "Authorization")
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        urlRequest.httpBody = try? JSONEncoder().encode(request)

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: urlRequest)
        } catch is CancellationError {
            throw CancellationError()
        } catch let urlError as URLError where urlError.code == .cancelled {
            throw CancellationError()
        } catch {
            throw APIError.network
        }
        guard let http = response as? HTTPURLResponse else { throw APIError.network }
        switch http.statusCode {
        case 200...299:
            do {
                return try APIDecoding.makeDecoder().decode(ManualResultResponse.self, from: data)
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
