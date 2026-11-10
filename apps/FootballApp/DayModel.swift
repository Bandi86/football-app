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

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> DayService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? DayModel.makeService
    }

    func load() async {
        state = .loading
        do {
            let info = try loader.load()
            let service = try serviceFactory(info)
            let matches = try await service.day(for: selectedDate)
            state = matches.isEmpty ? .empty : .loaded(matches)
        } catch let error as APIError {
            state = .error(error)
        } catch {
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
