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

    enum ListState: Equatable {
        case idle
        case loading
        case loaded(IssuesResponseDTO)
        case error(APIError)
    }

    enum HistoryState: Equatable {
        case idle
        case loading
        case loaded(HistoryResponseDTO)
        case error(APIError)
    }

    var state: State = .idle
    var issuesState: ListState = .idle
    var historyState: HistoryState = .idle

    /// A hiánylista szűrője; a változtatás azonnal új lekérést indít.
    var query = IssuesQueryDTO() {
        didSet { Task { await loadIssues() } }
    }

    /// Kiválasztott jel: meccs-tárgy esetén az idővonal részletezője nyílik meg.
    private(set) var selectedIssue: IssueRowDTO?

    private let loader: any RuntimeInfoLoading
    private let serviceFactory: (RuntimeInfo) throws -> OpsService
    private var generation = 0
    private var issuesGeneration = 0
    private var historyGeneration = 0

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
            let service = try makeServiceFromRuntime()
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

    func loadIssues() async {
        issuesGeneration &+= 1
        let currentGeneration = issuesGeneration
        if case .loaded = issuesState {} else { issuesState = .loading }
        do {
            let service = try makeServiceFromRuntime()
            let response = try await service.issues(query: query)
            guard currentGeneration == issuesGeneration else { return }
            issuesState = .loaded(response)
        } catch is CancellationError {
            return
        } catch let error as APIError {
            guard currentGeneration == issuesGeneration else { return }
            issuesState = .error(error)
        } catch {
            guard currentGeneration == issuesGeneration else { return }
            issuesState = .error(.network)
        }
    }

    /// Egy jel kiválasztása; a régi idővonal-eredmény nem írja felül az újat.
    func selectIssue(_ issue: IssueRowDTO) {
        selectedIssue = issue
        historyGeneration &+= 1
        guard issue.subjectKind == .match else {
            historyState = .loaded(HistoryResponseDTO(entries: []))
            return
        }
        historyState = .loading
        Task { await loadHistory(for: issue) }
    }

    func closeIssue() {
        historyGeneration &+= 1
        selectedIssue = nil
        historyState = .idle
    }

    func loadHistory(for issue: IssueRowDTO) async {
        historyGeneration &+= 1
        let currentGeneration = historyGeneration
        do {
            let service = try makeServiceFromRuntime()
            let response = try await service.history(
                matchID: issue.subjectId, limit: 50
            )
            guard currentGeneration == historyGeneration else { return }
            historyState = .loaded(response)
        } catch is CancellationError {
            return
        } catch let error as APIError {
            guard currentGeneration == historyGeneration else { return }
            historyState = .error(error)
        } catch {
            guard currentGeneration == historyGeneration else { return }
            historyState = .error(.network)
        }
    }

    /// 30 mp-enkénti automatikus frissítés, amíg a nézet látható.
    func refreshLoop() async {
        while !Task.isCancelled {
            await load()
            await loadIssues()
            try? await Task.sleep(for: .seconds(30))
        }
    }

    private func makeServiceFromRuntime() throws -> OpsService {
        try serviceFactory(loader.load())
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
