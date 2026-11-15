import Foundation
import Observation
import FootballAppCore

@MainActor
@Observable
final class OpsModel {
    enum State: Equatable {
        case idle
        case loading
        case loaded(OpsStatus)
        case error(APIError)
    }

    var state: State = .idle

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> OpsService
    private var generation = 0

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> OpsService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? Self.makeService
    }

    func load() async {
        generation &+= 1
        let currentGeneration = generation
        if case .loaded = state {} else { state = .loading }
        do {
            let info = try loader.load()
            let service = try serviceFactory(info)
            let status = try await service.status()
            guard currentGeneration == generation else { return }
            state = .loaded(status)
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

    /// 30 mp-enkénti automatikus frissítés, amíg a nézet látható.
    func refreshLoop() async {
        while !Task.isCancelled {
            await load()
            try? await Task.sleep(for: .seconds(30))
        }
    }

    private static func makeService(_ info: RuntimeInfo) throws -> OpsService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        guard let opsToken = info.opsToken, !opsToken.isEmpty else {
            throw APIError.unauthorized
        }
        return HTTPOpsService(baseURL: baseURL, opsToken: opsToken)
    }
}
