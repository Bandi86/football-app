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

    @Test func decodesTeamDetailWithFormAndNext() throws {
        let json = """
        {"detail":{"team_id":"00000000-0000-0000-0000-0000000000e1","name":"Liverpool",
          "country_code":"GB","country_name":"England","gender":"male","branch":"senior",
          "logo":null,"elo":1783.5,"played":2,"wins":1,"draws":1,"losses":0,
          "goals_for":3,"goals_against":2},
         "matches":[],
         "form":[{"match_id":"00000000-0000-0000-0000-0000000000a1","kickoff_at":"2026-11-10T20:00:00Z",
                  "competition_name":"Premier League","opponent_name":"Everton","is_home":true,
                  "team_goals":2,"opponent_goals":0,"outcome":"W"}],
         "next":[{"match_id":"00000000-0000-0000-0000-0000000000a2","kickoff_at":"2026-11-24T15:00:00Z",
                  "competition_name":"Premier League","opponent_name":"Arsenal","is_home":false}]}
        """.data(using: .utf8)!
        let detail = try APIDecoding.makeDecoder().decode(TeamDetail.self, from: json)
        #expect(detail.form?.count == 1)
        #expect(detail.form?.first?.outcome == "W")
        #expect(detail.form?.first?.opponentName == "Everton")
        #expect(detail.next?.count == 1)
        #expect(detail.next?.first?.isHome == false)
    }
}
