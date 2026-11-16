import Foundation
import Observation
import FootballAppCore

@MainActor
@Observable
final class TeamsModel {
    enum State: Equatable {
        case idle
        case loading
        case loaded([TeamSummary])
        case error(APIError)
    }

    var state: State = .idle
    var search = ""
    let logos = LogoStore()

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> TeamService
    private var generation = 0

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> TeamService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? Self.makeService
    }

    func load() async {
        generation &+= 1
        let currentGeneration = generation
        state = .loading
        do {
            let info = try loader.load()
            if let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
                logos.configure(baseURL: baseURL, token: info.readToken)
            }
            let service = try serviceFactory(info)
            let rows = try await service.teams(date: Date())
            guard currentGeneration == generation else { return }
            state = .loaded(rows)
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

    var filtered: [TeamSummary] {
        guard case .loaded(let rows) = state else { return [] }
        let needle = search.trimmingCharacters(in: .whitespaces).lowercased()
        guard !needle.isEmpty else { return rows }
        return rows.filter { row in
            let display = CountryDisplay.display(code: row.countryCode, name: row.countryName).name
            return row.name.lowercased().contains(needle)
                || display.lowercased().contains(needle)
        }
    }

    private static func makeService(_ info: RuntimeInfo) throws -> TeamService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPTeamService(baseURL: baseURL, readToken: info.readToken)
    }
}

@MainActor
@Observable
final class TeamDetailModel {
    enum State: Equatable {
        case idle
        case loading
        case loaded(TeamDetail)
        case error(APIError)
    }

    var state: State = .idle
    let logos = LogoStore()

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> TeamService
    private var generation = 0

    init(
        loader: any RuntimeInfoLoading = RuntimeInfoLoader(),
        serviceFactory: ((RuntimeInfo) throws -> TeamService)? = nil
    ) {
        self.loader = loader
        self.serviceFactory = serviceFactory ?? Self.makeService
    }

    func load(id: UUID, date: Date = Date(), window: Int = 30) async {
        generation &+= 1
        let currentGeneration = generation
        state = .loading
        do {
            let info = try loader.load()
            if let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
                logos.configure(baseURL: baseURL, token: info.readToken)
            }
            let service = try serviceFactory(info)
            let detail = try await service.detail(id: id, date: date, window: window)
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

    private static func makeService(_ info: RuntimeInfo) throws -> TeamService {
        guard let baseURL = URL(string: "http://127.0.0.1:\(info.port)") else {
            throw APIError.runtimeMissing
        }
        return HTTPTeamService(baseURL: baseURL, readToken: info.readToken)
    }
}
