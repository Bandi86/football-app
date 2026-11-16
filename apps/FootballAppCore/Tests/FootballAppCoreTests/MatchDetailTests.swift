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
}
