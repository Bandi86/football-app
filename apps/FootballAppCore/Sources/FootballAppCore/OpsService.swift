import Foundation

public protocol OpsService: Sendable {
    func status() async throws -> OpsStatus
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
        var request = URLRequest(url: baseURL.appendingPathComponent("v1/ops/status"))
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
                return try APIDecoding.makeDecoder().decode(OpsStatus.self, from: data)
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
