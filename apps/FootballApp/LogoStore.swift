import AppKit
import Foundation
import Observation

/// Logó-betöltő a helyi API-ból (Bearer tokennel) és külső URL-ekről
/// (token nélkül). Memória-cache-sel; a hálózati hiba csendben placeholder.
@MainActor
@Observable
final class LogoStore {
    private let cache = NSCache<NSString, NSData>()
    private var baseURL: URL?
    private var token: String?

    func configure(baseURL: URL, token: String) {
        self.baseURL = baseURL
        self.token = token
    }

    func image(for path: String) async -> NSImage? {
        let key = path as NSString
        if let cached = cache.object(forKey: key) {
            return NSImage(data: cached as Data)
        }
        guard let url = resolve(path) else { return nil }
        var request = URLRequest(url: url)
        // Csak a saját API-hoz megy a token; külső CDN-hez nem.
        if path.hasPrefix("/"), let token {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                return nil
            }
            cache.setObject(data as NSData, forKey: key)
            return NSImage(data: data)
        } catch {
            return nil
        }
    }

    private func resolve(_ path: String) -> URL? {
        if path.hasPrefix("http://") || path.hasPrefix("https://") {
            return URL(string: path)
        }
        guard let baseURL else { return nil }
        return URL(string: path, relativeTo: baseURL)
    }
}
