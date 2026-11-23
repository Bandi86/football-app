import Foundation

/// Játékosadatlap a helyi API-ból. A nézet nem ismeri a HTTP részleteket.
public protocol PlayerService: Sendable {
    func player(id: UUID) async throws -> PlayerFeed
}

public enum PlayerServiceError: Error, Equatable, Sendable {
    case unavailable
    case notFound
    case decoding
}

public struct HTTPPlayerService: PlayerService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession
    private let decoder = APIDecoding.makeDecoder()

    public init(baseURL: URL, readToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
    }

    public func player(id: UUID) async throws -> PlayerFeed {
        let url = baseURL.appendingPathComponent("v1/player/\(id.uuidString)")
        var request = URLRequest(url: url)
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
            throw PlayerServiceError.unavailable
        }
        guard let http = response as? HTTPURLResponse else {
            throw PlayerServiceError.unavailable
        }
        switch http.statusCode {
        case 200...299:
            do {
                return try decoder.decode(PlayerFeed.self, from: data)
            } catch {
                throw PlayerServiceError.decoding
            }
        case 404:
            throw PlayerServiceError.notFound
        default:
            throw PlayerServiceError.unavailable
        }
    }
}
