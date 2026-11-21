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

/// Teljes meccsszerkesztés kérése a PUT /v1/match/{id}/manual-result végpontra.
///
/// Minden mező opcionális, kivéve a `note` indoklást. Az eredmény-blokk
/// szerveroldali cseréje miatt, ha bármelyik eredmény-mezőt küldjük, a
/// payloadban mind a nyolc eredmény-mezőnek szerepelnie kell — a hiányzókat
/// explicit `null`-ként —, különben a szerver nullázza azokat.
public struct ManualEditRequest: Encodable, Equatable, Sendable {
    public let kickoffAt: Date?
    public let kickoffPrecision: String?
    public let status: String?
    public let homeFt: Int?
    public let awayFt: Int?
    public let homeHt: Int?
    public let awayHt: Int?
    public let homeEt: Int?
    public let awayEt: Int?
    public let homePens: Int?
    public let awayPens: Int?
    public let decidedBy: String?
    public let note: String

    public init(
        note: String,
        kickoffAt: Date? = nil,
        kickoffPrecision: String? = nil,
        status: String? = nil,
        homeFt: Int? = nil,
        awayFt: Int? = nil,
        homeHt: Int? = nil,
        awayHt: Int? = nil,
        homeEt: Int? = nil,
        awayEt: Int? = nil,
        homePens: Int? = nil,
        awayPens: Int? = nil,
        decidedBy: String? = nil
    ) {
        self.note = note
        self.kickoffAt = kickoffAt
        self.kickoffPrecision = kickoffPrecision
        self.status = status
        self.homeFt = homeFt
        self.awayFt = awayFt
        self.homeHt = homeHt
        self.awayHt = awayHt
        self.homeEt = homeEt
        self.awayEt = awayEt
        self.homePens = homePens
        self.awayPens = awayPens
        self.decidedBy = decidedBy
    }

    /// Igaz, ha a kérés bármelyik eredmény-mezőt megadja. Ilyenkor a teljes
    /// eredmény-blokkot el kell küldeni (a nullákat is explicit null-ként).
    public var includesResult: Bool {
        homeFt != nil || awayFt != nil
            || homeHt != nil || awayHt != nil
            || homeEt != nil || awayEt != nil
            || homePens != nil || awayPens != nil
            || decidedBy != nil
    }

    private enum CodingKeys: String, CodingKey {
        case kickoffAt = "kickoff_at"
        case kickoffPrecision = "kickoff_precision"
        case status
        case homeFt = "home_ft"
        case awayFt = "away_ft"
        case homeHt = "home_ht"
        case awayHt = "away_ht"
        case homeEt = "home_et"
        case awayEt = "away_et"
        case homePens = "home_pens"
        case awayPens = "away_pens"
        case decidedBy = "decided_by"
        case note
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encodeIfPresent(kickoffAt?.formatted(.iso8601), forKey: .kickoffAt)
        try container.encodeIfPresent(kickoffPrecision, forKey: .kickoffPrecision)
        try container.encodeIfPresent(status, forKey: .status)
        try container.encode(note, forKey: .note)
        guard includesResult else { return }

        try encodeResult(homeFt, forKey: .homeFt, into: &container)
        try encodeResult(awayFt, forKey: .awayFt, into: &container)
        try encodeResult(homeHt, forKey: .homeHt, into: &container)
        try encodeResult(awayHt, forKey: .awayHt, into: &container)
        try encodeResult(homeEt, forKey: .homeEt, into: &container)
        try encodeResult(awayEt, forKey: .awayEt, into: &container)
        try encodeResult(homePens, forKey: .homePens, into: &container)
        try encodeResult(awayPens, forKey: .awayPens, into: &container)
        try container.encodeIfPresent(decidedBy, forKey: .decidedBy)
    }

    private func encodeResult(
        _ value: Int?,
        forKey key: CodingKeys,
        into container: inout KeyedEncodingContainer<CodingKeys>
    ) throws {
        if let value {
            try container.encode(value, forKey: key)
        } else {
            try container.encodeNil(forKey: key)
        }
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
    func submitEdit(matchId: UUID, request: ManualEditRequest) async throws -> ManualResultResponse
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
        try await put(matchId: matchId, body: request)
    }

    public func submitEdit(matchId: UUID, request: ManualEditRequest) async throws -> ManualResultResponse {
        try await put(matchId: matchId, body: request)
    }

    private func put<Body: Encodable>(matchId: UUID, body: Body) async throws -> ManualResultResponse {
        var urlRequest = URLRequest(
            url: baseURL.appendingPathComponent("v1/match/\(matchId.uuidString.lowercased())/manual-result")
        )
        urlRequest.httpMethod = "PUT"
        urlRequest.setValue("Bearer \(opsToken)", forHTTPHeaderField: "Authorization")
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")
        do {
            urlRequest.httpBody = try JSONEncoder().encode(body)
        } catch {
            throw APIError.decoding
        }

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
            if let message = APIDecoding.serverErrorMessage(from: data) {
                throw APIError.serverMessage(message)
            }
            throw APIError.badRequest
        case 409:
            throw APIError.conflict(APIDecoding.serverErrorMessage(from: data) ?? "")
        default:
            throw APIError.server
        }
    }
}