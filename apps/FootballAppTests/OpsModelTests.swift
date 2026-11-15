import XCTest
import FootballAppCore
@testable import FootballApp

@MainActor
final class OpsModelTests: XCTestCase {
    private struct StubLoader: RuntimeInfoLoading {
        let result: Result<RuntimeInfo, APIError>
        func load() throws -> RuntimeInfo { try result.get() }
    }

    private struct StubService: OpsService {
        let result: Result<OpsStatus, APIError>
        func status() async throws -> OpsStatus { try result.get() }
    }

    private let info = RuntimeInfo(port: 56782, readToken: "r", opsToken: "o")

    private func sampleStatus() -> OpsStatus {
        let json = """
        {"pending_jobs":1,"leased_jobs":0,"quarantined":0,"pending_reviews":2,
         "sources":[{"source_slug":"soccerway","mode":"canary","enabled":true,
                     "attempts_today":14,"daily_budget":14,"breaker_state":"closed"}]}
        """.data(using: .utf8)!
        return try! APIDecoding.makeDecoder().decode(OpsStatus.self, from: json)
    }

    func testLoadedState() async {
        let status = sampleStatus()
        let model = OpsModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(status)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .loaded(status))
    }

    func testMissingOpsTokenIsUnauthorized() async {
        let model = OpsModel(
            loader: StubLoader(result: .success(RuntimeInfo(port: 56782, readToken: "r"))),
            serviceFactory: nil
        )
        await model.load()
        XCTAssertEqual(model.state, .error(.unauthorized))
    }

    func testServiceFailureShowsError() async {
        let model = OpsModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .failure(.server)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .error(.server))
    }
}
