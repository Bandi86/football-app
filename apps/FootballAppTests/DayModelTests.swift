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
        let result: Result<DayFeed, APIError>
        func day(for date: Date) async throws -> DayFeed { try result.get() }
    }

    private struct DelayService: DayService {
        let feed: DayFeed
        let delay: Duration
        func day(for date: Date) async throws -> DayFeed {
            try await Task.sleep(for: delay)
            return feed
        }
    }

    private struct CancellingService: DayService {
        func day(for date: Date) async throws -> DayFeed { throw CancellationError() }
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

    private func matches(home: String) throws -> DayFeed {
        let json = """
        {"date":"2026-11-09","timezone":"Europe/Budapest","matches":[
          {"id":"00000000-0000-0000-0000-000000000101","kickoff_at":"2026-11-09T18:00:00Z",
           "status":"scheduled","home":"\(home)","away":"Győr","home_score":null,"away_score":null,
           "publish_state":"verified_publish","blockers":[]}
        ]}
        """.data(using: .utf8)!
        let response = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json)
        return DayFeed(refreshedAt: response.refreshedAt, matches: response.matches)
    }

    private let info = RuntimeInfo(port: 56782, readToken: "t")

    func testRefreshKeepsLoadedContentAndSwallowsErrors() async throws {
        let first = try matches(home: "Első")
        let sequence = ServiceSequence([
            StubService(result: .success(first)),
            StubService(result: .failure(.network)),
        ])
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in sequence.next() }
        )
        await model.load()
        guard case .loaded(let first) = model.state else {
            return XCTFail("a betöltés nem hozott listát")
        }
        XCTAssertEqual(first.matches.first?.home, "Első")

        await model.refresh()
        // Háttér-hiba esetén a korábbi tartalom marad, csak a jelző vált.
        guard case .loaded(let stillThere) = model.state else {
            return XCTFail("a háttér-hiba eldobta a listát")
        }
        XCTAssertEqual(stillThere.matches.first?.home, "Első")
        XCTAssertTrue(model.lastRefreshFailed)
    }

    func testRefreshReplacesContent() async throws {
        let first = try matches(home: "Első")
        let second = try matches(home: "Második")
        let sequence = ServiceSequence([
            StubService(result: .success(first)),
            StubService(result: .success(second)),
        ])
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in sequence.next() }
        )
        await model.load()
        await model.refresh()
        guard case .loaded(let second) = model.state else {
            return XCTFail("a frissítés nem hozott listát")
        }
        XCTAssertEqual(second.matches.first?.home, "Második")
        XCTAssertFalse(model.lastRefreshFailed)
    }

    func testEmptyState() async {
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(DayFeed(refreshedAt: nil, matches: []))) }
        )
        await model.load()
        XCTAssertEqual(model.state, .empty)
    }

    func testErrorWhenRuntimeMissing() async {
        let model = DayModel(
            loader: StubLoader(result: .failure(.runtimeMissing)),
            serviceFactory: { _ in StubService(result: .success(DayFeed(refreshedAt: nil, matches: []))) }
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
        let response = try APIDecoding.makeDecoder().decode(DayResponse.self, from: json)
        let feed = DayFeed(refreshedAt: response.refreshedAt, matches: response.matches)
        let model = DayModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(feed)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .loaded(feed))
    }

    /// Az `.task` és az `.onChange` indíthat egymást átfedő betöltést; a korábbi
    /// (lassabb) válasz nem írhatja felül a frissebb eredményt.
    func testStaleLoadDoesNotOverwriteNewerResult() async throws {
        let stale = try matches(home: "Régi")
        let fresh = try matches(home: "Friss")
        let sequence = ServiceSequence([
            DelayService(feed: stale, delay: .milliseconds(300)),
            DelayService(feed: fresh, delay: .zero),
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

    private func makeModel() -> DayModel {
        DayModel(
            loader: StubLoader(result: .failure(.runtimeMissing)),
            serviceFactory: { _ in StubService(result: .success(DayFeed(refreshedAt: nil, matches: []))) }
        )
    }

    func testGoToNextDayAdvancesOneDay() {
        let model = makeModel()
        let start = Calendar.current.date(from: DateComponents(year: 2026, month: 11, day: 9))!
        model.selectedDate = start
        model.goToNextDay()
        let expected = Calendar.current.date(byAdding: .day, value: 1, to: start)!
        XCTAssertTrue(Calendar.current.isDate(model.selectedDate, inSameDayAs: expected))
    }

    func testGoToPreviousDayGoesBackOneDay() {
        let model = makeModel()
        let start = Calendar.current.date(from: DateComponents(year: 2026, month: 11, day: 9))!
        model.selectedDate = start
        model.goToPreviousDay()
        let expected = Calendar.current.date(byAdding: .day, value: -1, to: start)!
        XCTAssertTrue(Calendar.current.isDate(model.selectedDate, inSameDayAs: expected))
    }

    func testGoToTodaySelectsToday() {
        let model = makeModel()
        model.selectedDate = Calendar.current.date(from: DateComponents(year: 2000, month: 1, day: 1))!
        model.goToToday()
        XCTAssertTrue(Calendar.current.isDateInToday(model.selectedDate))
    }
}
