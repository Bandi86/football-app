import Foundation
import Testing
@testable import FootballAppCore

struct MatchDetailTests {
    private func json() -> Data {
        """
        {"id":"00000000-0000-0000-0000-0000000000aa","kickoff_at":"2026-11-16T15:00:00Z",
         "kickoff_precision":"exact","status":"finished","round_label":null,
         "competition_id":"00000000-0000-0000-0000-0000000000b1","competition_name":"NB I",
         "country_code":"HU","country_name":"Hungary","tier":1,"priority":8,
         "season_label":"2026/27","home_name":"Újpest","away_name":"Győr",
         "home_logo":null,"away_logo":null,"home_elo":1700.5,"away_elo":null,
         "home_ft":2,"away_ft":1,"home_ht":1,"away_ht":0,
         "home_et":null,"away_et":null,"home_pens":null,"away_pens":null,
         "decided_by":"ft","referee":"M. Oliver","stats_source":"footballdata",
         "stats_captured_at":"2026-11-16T17:00:00Z",
         "home_shots":12,"away_shots":9,"home_shots_on_target":5,"away_shots_on_target":3,
         "home_corners":6,"away_corners":4,"home_yellow":1,"away_yellow":2,
         "home_red":0,"away_red":0,"home_fouls":10,"away_fouls":11,
         "home_xg":1.4,"away_xg":0.8,
         "odds":{"home":1.85,"draw":3.4,"away":4.2,"btts_yes":1.72,"btts_no":2.05,
                 "over25":2.1,"under25":1.75},
         "market_count":12,"odds_source":"Football-Data","odds_captured_at":"2026-11-16T14:00:00Z",
         "odds_trust_state":"verified","gate_state":"verified_publish","gate_blockers":[],
         "gate_evaluated_at":"2026-11-16T17:05:00Z","gate_rule_set_version":"verification-1.1",
         "publish_date":"2026-11-16"}
        """.data(using: .utf8)!
    }

    @Test func decodesFullDetail() throws {
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: json())
        #expect(detail.homeName == "Újpest")
        #expect(detail.gateState == .verifiedPublish)
        #expect(detail.decidedBy == "ft")
        #expect(detail.homeElo == 1700.5)
        #expect(detail.odds?.bttsYes == 1.72)
        #expect(detail.statsSource == "footballdata")
        #expect(detail.referee == "M. Oliver")
        #expect(detail.gateBlockers.isEmpty)
    }

    @Test func decodesFormAndHeadToHead() throws {
        let data = """
        {"id":"00000000-0000-0000-0000-0000000000aa","kickoff_at":"2026-11-21T18:00:00Z",
         "kickoff_precision":"exact","status":"scheduled","round_label":"13. forduló",
         "competition_id":"00000000-0000-0000-0000-0000000000b1","competition_name":"La Liga",
         "country_code":"ES","country_name":"Spanyolország","tier":1,"priority":4,
         "season_label":"2026/27","home_name":"Alfa","away_name":"Beta",
         "home_logo":null,"away_logo":null,"home_elo":null,"away_elo":null,
         "home_ft":null,"away_ft":null,"home_ht":null,"away_ht":null,
         "home_et":null,"away_et":null,"home_pens":null,"away_pens":null,
         "decided_by":null,"referee":null,"stats_source":null,"stats_captured_at":null,
         "home_shots":null,"away_shots":null,"home_shots_on_target":null,
         "away_shots_on_target":null,"home_corners":null,"away_corners":null,
         "home_yellow":null,"away_yellow":null,"home_red":null,"away_red":null,
         "home_fouls":null,"away_fouls":null,"home_xg":null,"away_xg":null,
         "odds":null,"market_count":null,"odds_source":null,"odds_captured_at":null,
         "odds_trust_state":null,"gate_state":null,"gate_blockers":[],
         "gate_evaluated_at":null,"gate_rule_set_version":null,"publish_date":null,
         "home_form":[
           {"match_id":"10000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-14T18:00:00Z",
            "competition_name":"La Liga","opponent_name":"Gamma","is_home":true,
            "team_goals":2,"opponent_goals":1,"outcome":"W"}
         ],
         "away_form":[],
         "head_to_head":[
           {"match_id":"10000000-0000-0000-0000-000000000002","kickoff_at":"2026-11-10T20:00:00Z",
            "competition_name":"Copa del Rey","home_name":"Beta","away_name":"Alfa",
            "home_goals":0,"away_goals":1}
         ]}
        """.data(using: .utf8)!
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)
        #expect(detail.homeForm?.count == 1)
        #expect(detail.homeForm?.first?.outcome == "W")
        #expect(detail.homeForm?.first?.opponentName == "Gamma")
        #expect(detail.awayForm?.isEmpty == true)
        #expect(detail.headToHead?.first?.awayGoals == 1)
    }

    @Test func decodesNextMatches() throws {
        let data = """
        {"id":"00000000-0000-0000-0000-0000000000aa","kickoff_at":"2026-11-21T18:00:00Z",
         "kickoff_precision":"exact","status":"scheduled","round_label":null,
         "competition_id":"00000000-0000-0000-0000-0000000000b1","competition_name":"La Liga",
         "country_code":"ES","country_name":"Spanyolország","tier":1,"priority":4,
         "season_label":"2026/27","home_name":"Alfa","away_name":"Beta",
         "home_logo":null,"away_logo":null,"home_elo":null,"away_elo":null,
         "home_ft":null,"away_ft":null,"home_ht":null,"away_ht":null,
         "home_et":null,"away_et":null,"home_pens":null,"away_pens":null,
         "decided_by":null,"referee":null,"stats_source":null,"stats_captured_at":null,
         "home_shots":null,"away_shots":null,"home_shots_on_target":null,
         "away_shots_on_target":null,"home_corners":null,"away_corners":null,
         "home_yellow":null,"away_yellow":null,"home_red":null,"away_red":null,
         "home_fouls":null,"away_fouls":null,"home_xg":null,"away_xg":null,
         "odds":null,"market_count":null,"odds_source":null,"odds_captured_at":null,
         "odds_trust_state":null,"gate_state":null,"gate_blockers":[],
         "gate_evaluated_at":null,"gate_rule_set_version":null,"publish_date":null,
         "home_form":[],"away_form":[],"head_to_head":[],
         "home_next":[
           {"match_id":"20000000-0000-0000-0000-000000000001","kickoff_at":"2026-11-28T18:00:00Z",
            "competition_name":"La Liga","opponent_name":"Gamma","is_home":false}
         ],
         "away_next":[],
         "home_position":2,"away_position":13}
        """.data(using: .utf8)!
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)
        #expect(detail.homePosition == 2)
        #expect(detail.awayPosition == 13)
        #expect(detail.homeNext?.count == 1)
        #expect(detail.homeNext?.first?.opponentName == "Gamma")
        #expect(detail.homeNext?.first?.isHome == false)
        #expect(detail.awayNext?.isEmpty == true)
    }

    @Test func decodesTableSlice() throws {
        let data = """
        {"id":"00000000-0000-0000-0000-0000000000aa","kickoff_at":"2026-11-21T18:00:00Z",
         "kickoff_precision":"exact","status":"scheduled","round_label":null,
         "competition_id":"00000000-0000-0000-0000-0000000000b1","competition_name":"La Liga",
         "country_code":"ES","country_name":"Spanyolország","tier":1,"priority":4,
         "season_label":"2026/27","home_name":"Alfa","away_name":"Beta",
         "home_logo":null,"away_logo":null,"home_elo":null,"away_elo":null,
         "home_ft":null,"away_ft":null,"home_ht":null,"away_ht":null,
         "home_et":null,"away_et":null,"home_pens":null,"away_pens":null,
         "decided_by":null,"referee":null,"stats_source":null,"stats_captured_at":null,
         "home_shots":null,"away_shots":null,"home_shots_on_target":null,
         "away_shots_on_target":null,"home_corners":null,"away_corners":null,
         "home_yellow":null,"away_yellow":null,"home_red":null,"away_red":null,
         "home_fouls":null,"away_fouls":null,"home_xg":null,"away_xg":null,
         "odds":null,"market_count":null,"odds_source":null,"odds_captured_at":null,
         "odds_trust_state":null,"gate_state":null,"gate_blockers":[],
         "gate_evaluated_at":null,"gate_rule_set_version":null,"publish_date":null,
         "home_position":1,"away_position":3,
         "table_slice":[
           {"position":1,"team_id":"10000000-0000-0000-0000-000000000001","name":"Alfa","logo":null,
            "played":10,"wins":7,"draws":2,"losses":1,"goals_for":20,"goals_against":8,
            "goal_diff":12,"points":23},
           {"position":2,"team_id":"10000000-0000-0000-0000-000000000002","name":"Gamma","logo":null,
            "played":10,"wins":5,"draws":3,"losses":2,"goals_for":15,"goals_against":10,
            "goal_diff":5,"points":18},
           {"position":3,"team_id":"10000000-0000-0000-0000-000000000003","name":"Beta","logo":null,
            "played":10,"wins":4,"draws":3,"losses":3,"goals_for":13,"goals_against":12,
            "goal_diff":1,"points":15}
         ]}
        """.data(using: .utf8)!
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)
        #expect(detail.tableSlice?.count == 3)
        #expect(detail.tableSlice?.first?.position == 1)
        #expect(detail.tableSlice?.last?.name == "Beta")
        #expect(detail.tableSlice?.last?.points == 15)
    }
}
