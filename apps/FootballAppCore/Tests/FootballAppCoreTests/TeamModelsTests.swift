import Foundation
import Testing
@testable import FootballAppCore

struct TeamModelsTests {
    @Test func decodesTeamSummary() throws {
        let json = """
        [{"team_id":"00000000-0000-0000-0000-0000000000e1","name":"Liverpool",
          "country_code":"GB","country_name":"England","gender":"male","branch":"senior",
          "logo":null,"recent_matches":3}]
        """.data(using: .utf8)!
        let rows = try APIDecoding.makeDecoder().decode([TeamSummary].self, from: json)
        #expect(rows.count == 1)
        #expect(rows[0].name == "Liverpool")
        #expect(rows[0].recentMatches == 3)
    }

    @Test func decodesTeamDetail() throws {
        let json = """
        {"detail":{"team_id":"00000000-0000-0000-0000-0000000000e1","name":"Liverpool",
          "country_code":"GB","country_name":"England","gender":"male","branch":"senior",
          "logo":null,"elo":1783.5,"played":2,"wins":1,"draws":1,"losses":0,
          "goals_for":3,"goals_against":2},"matches":[]}
        """.data(using: .utf8)!
        let detail = try APIDecoding.makeDecoder().decode(TeamDetail.self, from: json)
        #expect(detail.detail.elo == 1783.5)
        #expect(detail.detail.wins == 1)
    }
}
