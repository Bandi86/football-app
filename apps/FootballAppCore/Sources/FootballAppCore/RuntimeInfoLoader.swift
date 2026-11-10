import Foundation

public protocol RuntimeInfoLoading: Sendable {
    func load() throws -> RuntimeInfo
}

public struct RuntimeInfoLoader: RuntimeInfoLoading {
    public let fileURL: URL

    public static var defaultURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/FootballAppV2/runtime.json")
    }

    public init(fileURL: URL = RuntimeInfoLoader.defaultURL) {
        self.fileURL = fileURL
    }

    public func load() throws -> RuntimeInfo {
        guard let data = try? Data(contentsOf: fileURL) else {
            throw APIError.runtimeMissing
        }
        do {
            return try JSONDecoder().decode(RuntimeInfo.self, from: data)
        } catch {
            throw APIError.decoding
        }
    }
}
