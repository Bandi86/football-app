import Foundation

/// Globális keresés a helyi API-ból. A nézet nem ismeri a HTTP részleteket.
public protocol SearchService: Sendable {
    func search(term: String) async throws -> SearchFeed
}

public enum SearchError: Error, Equatable, Sendable {
    case unavailable
    case badRequest
    case unauthorized
    case decoding
}

public struct HTTPSearchService: SearchService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession
    private let decoder = APIDecoding.makeDecoder()

    public init(baseURL: URL, readToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
    }

    public func search(term: String) async throws -> SearchFeed {
        let clean = term.trimmingCharacters(in: .whitespacesAndNewlines)
        guard clean.count >= 2 else { return SearchFeed(query: clean, results: []) }
        guard var components = URLComponents(
            url: baseURL.appendingPathComponent("v1/search"),
            resolvingAgainstBaseURL: false
        ) else { throw SearchError.unavailable }
        components.queryItems = [URLQueryItem(name: "q", value: String(clean.prefix(80)))]
        guard let url = components.url else { throw SearchError.unavailable }

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
            throw SearchError.unavailable
        }
        guard let http = response as? HTTPURLResponse else { throw SearchError.unavailable }
        switch http.statusCode {
        case 200...299:
            do {
                return try decoder.decode(SearchFeed.self, from: data)
            } catch {
                throw SearchError.decoding
            }
        case 400:
            throw SearchError.badRequest
        case 401:
            throw SearchError.unauthorized
        default:
            throw SearchError.unavailable
        }
    }
}
