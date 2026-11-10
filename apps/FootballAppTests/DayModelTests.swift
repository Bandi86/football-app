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
}
