import Foundation
import Observation
import FootballAppCore

/// A játékosadatlap állapota. Hálózat csak a Core protokollon át; hiba esetén
/// nem mutatunk hamis adatot, hanem kifejezett üzenetet jelenítünk meg.
@MainActor
@Observable
final class PlayerModel {
    enum Status: Equatable {
        case idle
        case loading
        case loaded
        case failed
        case notFound
    }

    private(set) var feed: PlayerFeed?
    private(set) var status: Status = .idle

    private let service: any PlayerService
    private let playerID: UUID

    init(id: UUID, service: (any PlayerService)? = nil) {
        self.playerID = id
        if let service {
            self.service = service
        } else if let info = try? RuntimeInfoLoader().load(),
                  let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
            self.service = HTTPPlayerService(baseURL: baseURL, readToken: info.readToken)
        } else {
            self.service = UnavailablePlayerService()
        }
    }

    func load() async {
        status = .loading
        do {
            feed = try await service.player(id: playerID)
            status = .loaded
        } catch is CancellationError {
            status = feed == nil ? .idle : .loaded
        } catch PlayerServiceError.notFound {
            status = .notFound
        } catch {
            status = .failed
        }
    }
}

private struct UnavailablePlayerService: PlayerService {
    func player(id: UUID) async throws -> PlayerFeed {
        throw PlayerServiceError.unavailable
    }
}
