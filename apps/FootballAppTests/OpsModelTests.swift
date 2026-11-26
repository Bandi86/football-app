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

        let summary: Result<OpsSummaryDTO, APIError>
        let audit: Result<AuditResponseDTO, APIError>
        let resolve: Result<ResolveResponseDTO, APIError>
        let summaryDateRecorder: ((String?) -> Void)?

        init(
            status: Result<OpsStatus, APIError>,
            issues: Result<IssuesResponseDTO, APIError>,
            history: Result<HistoryResponseDTO, APIError>,
            summary: Result<OpsSummaryDTO, APIError>,
            audit: Result<AuditResponseDTO, APIError>,
            resolve: Result<ResolveResponseDTO, APIError>,
            summaryDateRecorder: ((String?) -> Void)? = nil
        ) {
            self.status = status
            self.issues = issues
            self.history = history
            self.summary = summary
            self.audit = audit
            self.resolve = resolve
            self.summaryDateRecorder = summaryDateRecorder
        }

        func status() async throws -> OpsStatus { try status.get() }
        func issues(query: IssuesQueryDTO) async throws -> IssuesResponseDTO { try issues.get() }
        func history(matchID: UUID, limit: Int) async throws -> HistoryResponseDTO { try history.get() }
        func summary(date: String?) async throws -> OpsSummaryDTO {
            summaryDateRecorder?(date)
            return try summary.get()
        }
        func audit(limit: Int, subjectKind: String?, subjectID: UUID?) async throws -> AuditResponseDTO { try audit.get() }
        func resolveIssue(id: UUID) async throws -> ResolveResponseDTO { try resolve.get() }
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

    private static func sampleSummary() -> OpsSummaryDTO {
        decode("""
        {"date":"2026-11-25","open_total":1,"previous_day_total":0,
         "by_code":[{"code":"missing_odds","label_hu":"Hiányzó odds","count":1,"hard":0}],
         "top_actions":[],"headline_hu":"2026. 11. 25.: 1 nyitott hiányjel."}
        """)
    }

    private static func sampleAudit() -> AuditResponseDTO {
        decode("""
        {"entries":[{"id":"00000000-0000-0000-0000-000000000009","actor":"detect",
          "action":"issues_detected","subject_kind":"day","subject_id":null,
          "created_at":"2026-11-26T12:00:00Z"}],"count":1}
        """)
    }

    private static func sampleResolve() -> ResolveResponseDTO {
        decode("""
        {"issue_id":"00000000-0000-0000-0000-000000000001","resolved":true}
        """)
    }

    private func makeModel(
        status: Result<OpsStatus, APIError>? = nil,
        issues: Result<IssuesResponseDTO, APIError>? = nil,
        history: Result<HistoryResponseDTO, APIError>? = nil,
        summary: Result<OpsSummaryDTO, APIError>? = nil,
        audit: Result<AuditResponseDTO, APIError>? = nil,
        summaryDateRecorder: ((String?) -> Void)? = nil
    ) -> OpsModel {
        OpsModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in
                StubService(
                    status: status ?? .success(Self.sampleStatus()),
                    issues: issues ?? .success(Self.sampleIssues()),
                    history: history ?? .success(Self.sampleHistory()),
                    summary: summary ?? .success(Self.sampleSummary()),
                    audit: audit ?? .success(Self.sampleAudit()),
                    resolve: .success(Self.sampleResolve()),
                    summaryDateRecorder: summaryDateRecorder
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
                history: .success(Self.sampleHistory()),
                summary: .success(Self.sampleSummary()),
                audit: .success(Self.sampleAudit()),
                resolve: .success(Self.sampleResolve())
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

    func testSummaryAndAuditLoadIntoStates() async {
        let model = makeModel()
        let summary = Self.sampleSummary()
        let audit = Self.sampleAudit()
        await model.loadSummary()
        await model.loadAudit()
        XCTAssertEqual(model.summaryState, .loaded(summary))
        XCTAssertEqual(model.auditState, .loaded(audit))
    }

    func testSummaryDateChangePassesSelectedDateToAPI() async {
        var requested: [String] = []
        let model = makeModel(summaryDateRecorder: { date in
            if let date { requested.append(date) }
        })
        await model.loadSummary()
        XCTAssertEqual(requested.first ?? "", model.summaryDate)

        model.summaryDate = "2026-11-20"
        for _ in 0..<20 {
            if requested.count >= 2 { break }
            await Task.yield()
        }
        XCTAssertEqual(requested.last ?? "", "2026-11-20")
    }

    func testAuditFilterChangeReloads() async {
        let model = makeModel()
        await model.loadAudit()
        model.auditSubjectKind = "match"
        for _ in 0..<20 {
            if case .loaded = model.auditState { break }
            await Task.yield()
        }
        XCTAssertEqual(model.auditSubjectKind, "match")
    }

    func testResolveIssueRefreshesLists() async {
        let model = makeModel()
        await model.loadIssues()
        await model.resolveIssue(Self.sampleIssues().issues[0].id)
        XCTAssertEqual(model.issuesState, .loaded(Self.sampleIssues()))
    }
}
