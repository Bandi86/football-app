import XCTest
import FootballAppCore
@testable import FootballApp

@MainActor
final class DayModelTests: XCTestCase {
    private struct StubLoader: RuntimeInfoLoading {
        let result: Result<RuntimeInfo, APIError>
        func load() throws -> RuntimeInfo { try result.get() }
    }

    private struct StubService: DayService {
        let result: Result<[DayMatch], APIError>
        func day(for date: Date) async throws -> [DayMatch] { try result.get() }
    }

    private struct DelayService: DayService {
        let matches: [DayMatch]
        let delay: Duration
        func day(for date: Date) async throws -> [DayMatch] {
            try await Task.sleep(for: delay)
            return matches
        }
    }

    private struct CancellingService: DayService {
        func day(for date: Date) async throws -> [DayMatch] { throw CancellationError() }
    }

    /// Hívás szerinti szolgáltatásváltást tesz lehetővé anélkül, hogy a
    /// `serviceFactory` bezárásában módosítható változót kellene megosztani.
    private final class ServiceSequence: @unchecked Sendable {
        private let services: [DayService]
        private var index = 0
        init(_ services: [DayService]) { self.services = services }
        func next() -> DayService {
            defer { index += 1 }
            return services[min(index, services.count - 1)]
        }
    }

    private func matches(home: String) throws -> [DayMatch] {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"\(home)","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        return try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches
    }

    private let info = RuntimeInfo(port: 56782, readToken: "t")

    func testEmptyState() async {
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success([])) }
        )
        await model.load()
        XCTAssertEqual(model.state, .empty)
    }

    func testErrorWhenRuntimeMissing() async {
        let model = DayModel(
            loader: StubLoader(result: .failure(.runtimeMissing)),
            serviceFactory: { _ in StubService(result: .success([])) }
        )
        await model.load()
        XCTAssertEqual(model.state, .error(.runtimeMissing))
    }

    func testErrorOnServiceFailure() async {
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .failure(.unauthorized)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .error(.unauthorized))
    }

    func testLoadedState() async throws {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"Újpest","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let matches = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json).matches
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(matches)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .loaded(matches))
    }

    /// Az `.task` és az `.onChange` indíthat egymást átfedő betöltést; a korábbi
    /// (lassabb) válasz nem írhatja felül a frissebb eredményt.
    func testStaleLoadDoesNotOverwriteNewerResult() async throws {
        let stale = try matches(home: "Régi")
        let fresh = try matches(home: "Friss")
        let sequence = ServiceSequence([
            DelayService(matches: stale, delay: .milliseconds(300)),
            DelayService(matches: fresh, delay: .zero),
        ])
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in sequence.next() }
        )

        let firstLoad = Task { await model.load() }
        try await Task.sleep(for: .milliseconds(40))
        await model.load()
        XCTAssertEqual(model.state, .loaded(fresh))

        await firstLoad.value
        XCTAssertEqual(model.state, .loaded(fresh), "Az elavult válasz nem írhatja felül a frisset")
    }

    func testCancellationErrorDoesNotSetErrorState() async {
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in CancellingService() }
        )
        await model.load()
        if case .error = model.state {
            XCTFail("A megszakítást nem szabad hibaként megjeleníteni")
        }
    }
}
