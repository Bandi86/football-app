import XCTest
import FootballAppCore
@testable import FootballApp

@MainActor
final class CompetitionsModelTests: XCTestCase {
    private struct StubLoader: RuntimeInfoLoading {
        let result: Result<RuntimeInfo, APIError>
        func load() throws -> RuntimeInfo { try result.get() }
    }

    private struct StubService: CompetitionService {
        let rows: Result<[CompetitionSummary], APIError>
        let detail: Result<CompetitionDetail, APIError>
        func competitions(date: Date) async throws -> [CompetitionSummary] { try rows.get() }
        func detail(id: UUID, date: Date, window: Int) async throws -> CompetitionDetail {
            try detail.get()
        }
        func seasons(id: UUID) async throws -> [CompetitionSeason] { [] }
    }

    private let info = RuntimeInfo(port: 56782, readToken: "r")

    private func summaries() -> [CompetitionSummary] {
        let json = """
        [{"competition_id":"00000000-0000-0000-0000-0000000000c1","name":"NB I",
          "country_code":"HU","country_name":"Hungary","tier":1,"priority":8,
          "logo":null,"matches":3,"finished":1,"live":1,"scheduled":1},
         {"competition_id":"00000000-0000-0000-0000-0000000000c2","name":"Premier League",
          "country_code":"GB","country_name":"England","tier":1,"priority":6,
          "logo":null,"matches":5,"finished":5,"live":0,"scheduled":0}]
        """.data(using: .utf8)!
        return try! APIDecoding.makeDecoder().decode([CompetitionSummary].self, from: json)
    }

    func testLoadedAndSearch() async {
        let rows = summaries()
        let model = CompetitionsModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in
                StubService(rows: .success(rows), detail: .failure(.notFound))
            }
        )
        await model.load()
        XCTAssertEqual(model.state, .loaded(rows))
        model.search = "premier"
        XCTAssertEqual(model.filtered.map(\.name), ["Premier League"])
        model.search = "magyar"
        XCTAssertEqual(model.filtered.map(\.name), ["NB I"], "ország szerinti keresés")
    }

    func testDetailModelLoads() async {
        let json = """
        {"detail":{"competition_id":"00000000-0000-0000-0000-0000000000c1","name":"NB I",
          "country_code":"HU","country_name":"Hungary","tier":1,"priority":8,"logo":null,
          "season_label":"2026/27","matches":2,"goals":5,"btts_matches":2,
          "home_wins":1,"draws":1,"away_wins":0},"matches":[],"teams":[]}
        """.data(using: .utf8)!
        let detail = try! APIDecoding.makeDecoder().decode(CompetitionDetail.self, from: json)
        let model = CompetitionDetailModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in
                StubService(rows: .success([]), detail: .success(detail))
            }
        )
        await model.load(id: detail.detail.competitionId)
        XCTAssertEqual(model.state, .loaded(detail))
    }
}
