import Foundation
import Testing
@testable import FootballAppCore

struct CompetitionModelsTests {
    @Test func decodesCompetitionSummary() throws {
        let json = """
        [{"competition_id":"00000000-0000-0000-0000-0000000000c1","name":"NB I",
          "country_code":"HU","country_name":"Hungary","tier":1,"priority":8,
          "logo":null,"matches":3,"finished":1,"live":1,"scheduled":1}]
        """.data(using: .utf8)!
        let rows = try APIDecoding.makeDecoder().decode([CompetitionSummary].self, from: json)
        #expect(rows.count == 1)
        #expect(rows[0].name == "NB I")
        #expect(rows[0].matches == 3)
        #expect(rows[0].live == 1)
    }

    @Test func decodesCompetitionDetailWithTeams() throws {
        let json = """
        {"detail":{"competition_id":"00000000-0000-0000-0000-0000000000c1","name":"NB I",
          "country_code":"HU","country_name":"Hungary","tier":1,"priority":8,"logo":null,
          "season_label":"2026/27","matches":2,"goals":5,"btts_matches":2,
          "home_wins":1,"draws":1,"away_wins":0},
         "matches":[],
         "teams":[{"team_id":"00000000-0000-0000-0000-0000000000d1","name":"Újpest",
                   "logo":null,"played":2,"wins":1,"draws":1,"losses":0,
                   "goals_for":3,"goals_against":2}]}
        """.data(using: .utf8)!
        let detail = try APIDecoding.makeDecoder().decode(CompetitionDetail.self, from: json)
        #expect(detail.detail.goals == 5)
        #expect(detail.detail.bttsMatches == 2)
        #expect(detail.teams.count == 1)
        #expect(detail.teams[0].wins == 1)
    }

    @Test func decodesStandingsTable() throws {
        let json = """
        {"detail":{"competition_id":"00000000-0000-0000-0000-0000000000c1","name":"NB I",
          "country_code":"HU","country_name":"Hungary","tier":1,"priority":8,"logo":null,
          "season_label":"2026/27","matches":2,"goals":5,"btts_matches":2,
          "home_wins":1,"draws":1,"away_wins":0},
         "matches":[],
         "teams":[],
         "standings":[
           {"position":1,"team_id":"00000000-0000-0000-0000-0000000000d1","name":"Újpest",
            "logo":null,"played":2,"wins":1,"draws":1,"losses":0,
            "goals_for":3,"goals_against":2,"goal_diff":1,"points":4},
           {"position":2,"team_id":"00000000-0000-0000-0000-0000000000d2","name":"FTC",
            "logo":null,"played":2,"wins":0,"draws":1,"losses":1,
            "goals_for":2,"goals_against":3,"goal_diff":-1,"points":1}]}
        """.data(using: .utf8)!
        let detail = try APIDecoding.makeDecoder().decode(CompetitionDetail.self, from: json)
        #expect(detail.standings?.count == 2)
        #expect(detail.standings?.first?.position == 1)
        #expect(detail.standings?.first?.points == 4)
        #expect(detail.standings?.last?.goalDiff == -1)
    }
}
