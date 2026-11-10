import Foundation
import Observation
import FootballAppCore

@MainActor
@Observable
final class DayModel {
    enum State: Equatable {
        case idle
        case loading
        case loaded([DayMatch])
        case empty
        case error(APIError)
    }

    var selectedDate: Date = Date()
    var state: State = .idle

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> DayService

    /// Minden `load()` saját generációt kap; csak a legfrissebb generáció
    /// eredménye írhatja felül a `state`-et.
    private var generation = 0

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> DayService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? DayModel.makeService
    }

    func load() async {
        generation &+= 1
        let currentGeneration = generation
        state = .loading
        do {
            let info = try loader.load()
            let service = try serviceFactory(info)
            let matches = try await service.day(for: selectedDate)
            guard currentGeneration == generation else { return }
            state = matches.isEmpty ? .empty : .loaded(matches)
        } catch is CancellationError {
            // Megszakított betöltés: nem hiba, a frissebb betöltés veszi át.
            return
        } catch let error as APIError {
            guard currentGeneration == generation else { return }
            state = .error(error)
        } catch {
            guard currentGeneration == generation else { return }
            state = .error(.network)
        }
    }

    private static func makeService(_ info: RuntimeInfo) throws -> DayService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPDayService(baseURL: baseURL, readToken: info.readToken)
    }
}
