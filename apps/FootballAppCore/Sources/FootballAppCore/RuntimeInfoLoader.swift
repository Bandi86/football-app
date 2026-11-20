import Foundation

public protocol RuntimeInfoLoading: Sendable {
    func load() throws -> RuntimeInfo
}

public struct RuntimeInfoLoader: RuntimeInfoLoading {
    /// Ha be van állítva, ez az útvonal helyettesíti a felhasználói runtime.json-t.
    /// Az UI-tesztek és a párhuzamos helyi környezetek használják; üres érték esetén nem hat.
    public static let environmentKey = "FOOTBALL_APP_RUNTIME_FILE"

    public let fileURL: URL

    public static var defaultURL: URL {
        resolveDefaultURL()
    }

    /// A `FOOTBALL_APP_RUNTIME_FILE` környezeti változót tiszteletben tartó feloldás.
    public static func resolveDefaultURL(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> URL {
        if let override = environment[environmentKey], !override.isEmpty {
            return URL(fileURLWithPath: override)
        }
        return FileManager.default.homeDirectoryForCurrentUser
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
