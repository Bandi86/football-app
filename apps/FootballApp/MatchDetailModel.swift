import Foundation
import Observation
import FootballAppCore

@MainActor
@Observable
final class MatchDetailModel {
    enum State: Equatable {
        case idle
        case loading
        case loaded(MatchDetail)
        case error(APIError)
    }

    var state: State = .idle
    let logos = LogoStore()

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> MatchDetailService
    private var generation = 0

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> MatchDetailService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? Self.makeService
    }

    func load(matchId: UUID) async {
        generation &+= 1
        let currentGeneration = generation
        state = .loading
        do {
            let info = try loader.load()
            if let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
                logos.configure(baseURL: baseURL, token: info.readToken)
            }
            let service = try serviceFactory(info)
            let detail = try await service.detail(for: matchId)
            guard currentGeneration == generation else { return }
            state = .loaded(detail)
        } catch is CancellationError {
            return
        } catch let error as APIError {
            guard currentGeneration == generation else { return }
            state = .error(error)
        } catch {
            guard currentGeneration == generation else { return }
            state = .error(.network)
        }
    }

    private static func makeService(_ info: RuntimeInfo) throws -> MatchDetailService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPMatchDetailService(baseURL: baseURL, readToken: info.readToken)
    }
}
