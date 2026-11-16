import XCTest
import FootballAppCore
@testable import FootballApp

@MainActor
final class TeamsModelTests: XCTestCase {
    private struct StubLoader: RuntimeInfoLoading {
        let result: Result<RuntimeInfo, APIError>
        func load() throws -> RuntimeInfo { try result.get() }
    }

    private struct StubService: TeamService {
        let rows: Result<[TeamSummary], APIError>
        let detail: Result<TeamDetail, APIError>
        func teams(date: Date) async throws -> [TeamSummary] { try rows.get() }
        func detail(id: UUID, date: Date, window: Int) async throws -> TeamDetail {
            try detail.get()
        }
    }

    private let info = RuntimeInfo(port: 56782, readToken: "r")

    private func summaries() -> [TeamSummary] {
        let json = """
        [{"team_id":"00000000-0000-0000-0000-0000000000e1","name":"Liverpool",
          "country_code":"GB","country_name":"England","gender":"male","branch":"senior",
          "logo":null,"recent_matches":3},
         {"team_id":"00000000-0000-0000-0000-0000000000e2","name":"Bayer Leverkusen W",
          "country_code":"DE","country_name":"Germany","gender":"female","branch":"women",
          "logo":null,"recent_matches":1}]
        """.data(using: .utf8)!
        return try! APIDecoding.makeDecoder().decode([TeamSummary].self, from: json)
    }

    func testLoadedAndSearch() async {
        let rows = summaries()
        let model = TeamsModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(rows: .success(rows), detail: .failure(.notFound)) }
        )
        await model.load()
        XCTAssertEqual(model.state, .loaded(rows))
        model.search = "liver"
        XCTAssertEqual(model.filtered.map(\.name), ["Liverpool"])
        model.search = "német"
        XCTAssertEqual(model.filtered.map(\.name), ["Bayer Leverkusen W"], "ország szerinti keresés")
    }

    func testDetailModelLoads() async {
        let json = """
        {"detail":{"team_id":"00000000-0000-0000-0000-0000000000e1","name":"Liverpool",
          "country_code":"GB","country_name":"England","gender":"male","branch":"senior",
          "logo":null,"elo":1783.5,"played":2,"wins":1,"draws":1,"losses":0,
          "goals_for":3,"goals_against":2},"matches":[]}
        """.data(using: .utf8)!
        let detail = try! APIDecoding.makeDecoder().decode(TeamDetail.self, from: json)
        let model = TeamDetailModel(
            loader: StubLoader(result: .success(info)),
            serviceFactory: { _ in StubService(rows: .success([]), detail: .success(detail)) }
        )
        await model.load(id: detail.detail.teamId)
        XCTAssertEqual(model.state, .loaded(detail))
    }
}
