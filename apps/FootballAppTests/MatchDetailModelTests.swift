import XCTest
import FootballAppCore
@testable import FootballApp

@MainActor
final class MatchDetailModelTests: XCTestCase {
    private struct StubLoader: RuntimeInfoLoading {
        let result: Result<RuntimeInfo, APIError>
        func load() throws -> RuntimeInfo { try result.get() }
    }

    private struct StubService: MatchDetailService {
        let result: Result<MatchDetail, APIError>
        func detail(for matchId: UUID) async throws -> MatchDetail { try result.get() }
    }

    private let info = RuntimeInfo(port: 56782, readToken: "r")

    private func sample() -> MatchDetail {
        let json = """
        {"id":"00000000-0000-0000-0000-0000000000aa","kickoff_at":"2026-11-16T15:00:00Z",
         "kickoff_precision":"exact","status":"finished","round_label":null,
         "competition_id":"00000000-0000-0000-0000-0000000000b1","competition_name":"NB I",
         "country_code":"HU","country_name":"Hungary","tier":1,"priority":8,
         "season_label":"2026/27","home_name":"Újpest","away_name":"Győr",
         "home_logo":null,"away_logo":null,"home_elo":null,"away_elo":null,
         "home_ft":2,"away_ft":1,"home_ht":1,"away_ht":0,
         "home_et":null,"away_et":null,"home_pens":null,"away_pens":null,
         "decided_by":"ft","referee":null,"stats_source":null,"stats_captured_at":null,
         "home_shots":null,"away_shots":null,"home_shots_on_target":null,
         "away_shots_on_target":null,"home_corners":null,"away_corners":null,
         "home_yellow":null,"away_yellow":null,"home_red":null,"away_red":null,
         "home_fouls":null,"away_fouls":null,"home_xg":null,"away_xg":null,
         "odds":null,"market_count":null,"odds_source":null,"odds_captured_at":null,
         "odds_trust_state":null,"gate_state":"verified_publish","gate_blockers":[],
         "gate_evaluated_at":null,"gate_rule_set_version":null,"publish_date":"2026-11-16"}
        """.data(using: .utf8)!
        return try! APIDecoding.makeDecoder().decode(MatchDetail.self, from: json)
    }

    func testLoadedState() async {
        let detail = sample()
        let model = MatchDetailModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(detail)) }
        )
        await model.load(matchId: detail.id)
        XCTAssertEqual(model.state, .loaded(detail))
    }

    func testNotFoundShowsError() async {
        let model = MatchDetailModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .failure(.notFound)) }
        )
        await model.load(matchId: UUID())
        XCTAssertEqual(model.state, .error(.notFound))
    }

    // MARK: - Teljes meccsszerkesztés

    private actor RecordingManualService: ManualEntryService {
        private(set) var lastMatchId: UUID?
        private(set) var lastEdit: ManualEditRequest?
        private let result: Result<ManualResultResponse, APIError>

        init(result: Result<ManualResultResponse, APIError>) {
            self.result = result
        }

        func submitResult(matchId: UUID, request: ManualResultRequest) async throws -> ManualResultResponse {
            try result.get()
        }

        func submitEdit(matchId: UUID, request: ManualEditRequest) async throws -> ManualResultResponse {
            lastMatchId = matchId
            lastEdit = request
            return try result.get()
        }
    }

    private func manualResponse() -> ManualResultResponse {
        let json = """
        {"match_id":"00000000-0000-0000-0000-0000000000aa",
         "observation_id":"00000000-0000-0000-0000-0000000000cc",
         "audit_id":"00000000-0000-0000-0000-0000000000dd",
         "state":"provisional_publish"}
        """.data(using: .utf8)!
        return try! APIDecoding.makeDecoder().decode(ManualResultResponse.self, from: json)
    }

    private func editRequest() -> ManualEditRequest {
        ManualEditRequest(
            note: "kézi javítás",
            kickoffAt: Date(timeIntervalSince1970: 1_800_000_000),
            kickoffPrecision: "exact",
            status: "finished",
            homeFt: 3,
            awayFt: 1,
            homeHt: 2,
            awayHt: 0,
            decidedBy: "ft"
        )
    }

    func testSubmitEditForwardsRequestAndReloads() async {
        let detail = sample()
        let recorder = RecordingManualService(result: .success(manualResponse()))
        let model = MatchDetailModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(detail)) },
            manualServiceFactory: { _ in recorder }
        )
        await model.load(matchId: detail.id)
        let request = editRequest()
        await model.submitEdit(matchId: detail.id, request: request)

        let recorded = await recorder.lastEdit
        let recordedMatchId = await recorder.lastMatchId
        XCTAssertEqual(recorded, request)
        XCTAssertEqual(recordedMatchId, detail.id)
        XCTAssertNil(model.submitError)
        XCTAssertFalse(model.isSubmitting)
        XCTAssertEqual(model.state, .loaded(detail))
    }

    func testSubmitEditSurfacesServerMessage() async {
        let detail = sample()
        let recorder = RecordingManualService(
            result: .failure(.serverMessage("A kezdés nem lehet a jövőben."))
        )
        let model = MatchDetailModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(detail)) },
            manualServiceFactory: { _ in recorder }
        )
        await model.load(matchId: detail.id)
        await model.submitEdit(matchId: detail.id, request: editRequest())

        XCTAssertEqual(model.submitError, .serverMessage("A kezdés nem lehet a jövőben."))
        XCTAssertFalse(model.isSubmitting)
        // Hiba esetén nincs újratöltés: a betöltött részletek megmaradnak.
        XCTAssertEqual(model.state, .loaded(detail))
    }

    func testSubmitEditRejectsBlankNoteWithoutCallingService() async {
        let detail = sample()
        let recorder = RecordingManualService(result: .success(manualResponse()))
        let model = MatchDetailModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(result: .success(detail)) },
            manualServiceFactory: { _ in recorder }
        )
        await model.load(matchId: detail.id)
        await model.submitEdit(matchId: detail.id, request: ManualEditRequest(note: "   "))

        XCTAssertEqual(model.submitError, .badRequest)
        let recorded = await recorder.lastEdit
        XCTAssertNil(recorded)
    }
}
