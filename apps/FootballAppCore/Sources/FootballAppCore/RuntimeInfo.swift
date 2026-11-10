import Foundation

public struct RuntimeInfo: Codable, Equatable, Sendable {
    public let port: Int
    public let readToken: String
    public let opsToken: String?

    enum CodingKeys: String, CodingKey {
        case port
        case readToken = "read_token"
        case opsToken = "ops_token"
    }

    public init(port: Int, readToken: String, opsToken: String? = nil) {
        self.port = port
        self.readToken = readToken
        self.opsToken = opsToken
    }
}
