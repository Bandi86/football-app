import Foundation
import Observation
import FootballAppCore

/// A globális keresés állapota. A hálózat csak a Core protokollon át érkezik;
/// a kliens csak a `127.0.0.1` API-t hívja.
@MainActor
@Observable
final class SearchModel {
    enum Status: Equatable {
        case idle
        case loading
        case loaded
        case failed
    }

    private(set) var query = ""
    private(set) var feed: SearchFeed?
    private(set) var status: Status = .idle

    private var service: any SearchService

    init(service: (any SearchService)? = nil) {
        if let service {
            self.service = service
        } else if let info = try? RuntimeInfoLoader().load(),
                  let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
            self.service = HTTPSearchService(baseURL: baseURL, readToken: info.readToken)
        } else {
            self.service = UnavailableSearchService()
        }
    }

    func search(_ text: String) async {
        query = text
        let term = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard term.count >= 2 else {
            feed = nil
            status = .idle
            return
        }
        status = .loading
        do {
            feed = try await service.search(term: term)
            status = .loaded
        } catch is CancellationError {
            status = feed == nil ? .idle : .loaded
        } catch {
            // A korábbi találatok megmaradnak; a hiba jelzése nem hamisít eredményt.
            status = .failed
        }
    }
}

private struct UnavailableSearchService: SearchService {
    func search(term: String) async throws -> SearchFeed {
        throw SearchError.unavailable
    }
}
