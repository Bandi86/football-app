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
        let status: Result<OpsStatus, APIError>
        let issues: Result<IssuesResponseDTO, APIError>
        let history: Result<HistoryResponseDTO, APIError>

        func status() async throws -> OpsStatus { try status.get() }
        func issues(query: IssuesQueryDTO) async throws -> IssuesResponseDTO { try issues.get() }
        func history(matchID: UUID, limit: Int) async throws -> HistoryResponseDTO { try history.get() }
    }

    private let info = RuntimeInfo(port: 56782, readToken: "r", opsToken: "o")

    private static func decode<T: Decodable>(_ json: String) -> T {
        try! APIDecoding.makeDecoder().decode(T.self, from: Data(json.utf8))
    }

    private static func sampleStatus() -> OpsStatus {
        decode("""
        {"pending_jobs":1,"leased_jobs":0,"quarantined":0,"pending_reviews":2,
         "sources":[{"source_slug":"soccerway","mode":"canary","enabled":true,
                     "attempts_today":14,"daily_budget":14,"breaker_state":"closed"}]}
        """)
    }

    private static let issuesJSON = """
    {"issues":[{"id":"00000000-0000-0000-0000-000000000001","subject_kind":"match",
      "subject_id":"00000000-0000-0000-0000-000000000002","code":"missing_odds",
      "severity":"warning","first_seen_at":"2026-11-26T08:00:00Z",
      "last_seen_at":"2026-11-26T09:30:00Z","resolved_at":null,"source":"daily-check"}],
     "count":1}
    """

    private static func sampleIssues() -> IssuesResponseDTO { decode(issuesJSON) }

    private static func sampleHistory() -> HistoryResponseDTO {
        decode("""
        {"entries":[{"at":"2026-11-26T10:00:00Z","origin":"audit","actor":"ui",
          "action":"manual_match_edit","field_key":null,"before":null,"after":null,"source":null}]}
        """)
    }

    private func makeModel(
        status: Result<OpsStatus, APIError>? = nil,
        issues: Result<IssuesResponseDTO, APIError>? = nil,
        history: Result<HistoryResponseDTO, APIError>? = nil
    ) -> OpsModel {
        OpsModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in
                StubService(
                    status: status ?? .success(Self.sampleStatus()),
                    issues: issues ?? .success(Self.sampleIssues()),
                    history: history ?? .success(Self.sampleHistory())
                )
            }
        )
    }

    func testLoadedState() async {
        let status = Self.sampleStatus()
        let model = OpsModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(
                status: .success(status),
                issues: .success(Self.sampleIssues()),
                history: .success(Self.sampleHistory())
            ) }
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
        let model = makeModel(status: .failure(.server))
        await model.load()
        XCTAssertEqual(model.state, .error(.server))
    }

    func testIssuesLoadIntoListState() async {
        let issues = Self.sampleIssues()
        let model = makeModel(issues: .success(issues))
        await model.loadIssues()
        XCTAssertEqual(model.issuesState, .loaded(issues))
    }

    func testEmptyIssueListIsNotAnError() async {
        let model = makeModel(issues: .success(IssuesResponseDTO(issues: [], count: 0)))
        await model.loadIssues()
        guard case .loaded(let response) = model.issuesState else {
            return XCTFail("Az üres lista is betöltött állapot")
        }
        XCTAssertEqual(response.count, 0)
    }

    func testIssueServiceFailureShowsError() async {
        let model = makeModel(issues: .failure(.unauthorized))
        await model.loadIssues()
        XCTAssertEqual(model.issuesState, .error(.unauthorized))
    }

    func testSelectingMatchIssueLoadsHistory() async {
        let history = Self.sampleHistory()
        let model = makeModel(history: .success(history))
        let issue = Self.sampleIssues().issues[0]
        model.selectIssue(issue)
        XCTAssertNotNil(model.selectedIssue)
        for _ in 0..<20 {
            if case .loaded = model.historyState { break }
            await Task.yield()
        }
        XCTAssertEqual(model.historyState, .loaded(history))
        model.closeIssue()
        XCTAssertNil(model.selectedIssue)
    }

    func testChangingFilterReloadsIssues() async {
        let model = makeModel()
        await model.loadIssues()
        model.query = IssuesQueryDTO(code: "identity_review", openOnly: true)
        for _ in 0..<20 {
            if case .loaded = model.issuesState { break }
            await Task.yield()
        }
        XCTAssertEqual(model.query.code, "identity_review")
        XCTAssertTrue(model.query.openOnly)
    }
}
