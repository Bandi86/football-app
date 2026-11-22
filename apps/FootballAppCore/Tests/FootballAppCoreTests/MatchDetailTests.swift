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

    // MARK: - Esemény-idővonal (incidents)

    /// A régi tesztek teljes JSON-ját nem kell ismételni: a nem kötelező
    /// mezők hiányozhatnak, ezért egy minimális törzs elég az incidents vizsgálatához.
    private func detailJSON(incidents: String?) -> Data {
        let incidentsField = incidents.map { ",\n \"incidents\": \($0)" } ?? ""
        return """
        {"id":"00000000-0000-0000-0000-0000000000aa","kickoff_at":"2026-11-16T15:00:00Z",
         "kickoff_precision":"exact","status":"finished","round_label":null,
         "competition_name":"NB I","tier":1,"priority":8,"season_label":"2026/27",
         "home_name":"Újpest","away_name":"Győr","gate_blockers":[]\(incidentsField)}
        """.data(using: .utf8)!
    }

    @Test func decodesIncidents() throws {
        let data = detailJSON(incidents: """
        [{"minute":34,"incident_type":"Goal","side":"home","player_name":"Varga Barnabás",
          "related_player_name":"Szoboszlai Dominik","detail":"Header",
          "score_after_home":1,"score_after_away":0},
         {"minute":45,"incident_type":"Half","side":null,"player_name":null,
          "related_player_name":null,"detail":null,
          "score_after_home":1,"score_after_away":0},
         {"minute":null,"incident_type":"VAR","side":"away","player_name":null,
          "related_player_name":null,"detail":"Goal check",
          "score_after_home":null,"score_after_away":null}]
        """)
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)

        #expect(detail.incidents?.count == 3)
        let goal = detail.incidents?.first
        #expect(goal?.minute == 34)
        #expect(goal?.incidentType == "Goal")
        #expect(goal?.kind == .goal)
        #expect(goal?.isHome == true)
        #expect(goal?.isAway == false)
        #expect(goal?.playerName == "Varga Barnabás")
        #expect(goal?.relatedPlayerName == "Szoboszlai Dominik")
        #expect(goal?.detail == "Header")
        #expect(goal?.scoreAfterHome == 1)
        #expect(goal?.scoreAfterAway == 0)

        #expect(detail.incidents?[1].kind == .half)
        #expect(detail.incidents?[1].minute == 45)

        let varReview = detail.incidents?[2]
        #expect(varReview?.minute == nil)
        #expect(varReview?.kind == .varReview)
        #expect(varReview?.isAway == true)
        #expect(varReview?.scoreAfterHome == nil)
    }

    @Test func decodesEmptyIncidentsAsEmptyList() throws {
        let detail = try APIDecoding.makeDecoder()
            .decode(MatchDetail.self, from: detailJSON(incidents: "[]"))
        #expect(detail.incidents?.isEmpty == true)
    }

    @Test func toleratesUnknownIncidentTypeAndUnknownSide() throws {
        let data = detailJSON(incidents: """
        [{"minute":null,"incident_type":"FanChant","side":"neutral",
          "player_name":null,"related_player_name":null,"detail":"Tribune",
          "score_after_home":null,"score_after_away":null}]
        """)
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)

        let incident = detail.incidents?.first
        #expect(incident?.incidentType == "FanChant")
        #expect(incident?.kind == .other("FanChant"))
        #expect(incident?.minute == nil)
        #expect(incident?.isHome == false)
        #expect(incident?.isAway == false)
    }

    @Test func decodesLegacyPayloadWithoutIncidents() throws {
        let detail = try APIDecoding.makeDecoder()
            .decode(MatchDetail.self, from: detailJSON(incidents: nil))
        #expect(detail.incidents == nil)
        #expect(detail.homeName == "Újpest")
        #expect(detail.status == "finished")
        #expect(detail.venue == nil)
        #expect(detail.weather == nil)
    }

    // MARK: - Stadion és időjárás

    private func detailJSON(venue: String?, weather: String?) -> Data {
        let venueField = venue.map { ",\"venue\": \($0)" } ?? ""
        let weatherField = weather.map { ",\"weather\": \($0)" } ?? ""
        return """
        {"id":"00000000-0000-0000-0000-0000000000aa","kickoff_at":"2026-11-16T15:00:00Z",
         "kickoff_precision":"exact","status":"finished","round_label":"13",
         "competition_name":"La Liga","tier":1,"priority":8,"season_label":"2026/27",
         "home_name":"Alfa","away_name":"Beta","gate_blockers":[]\(venueField)\(weatherField)}
        """.data(using: .utf8)!
    }

    @Test func decodesVenueAndWeather() throws {
        let data = detailJSON(
            venue: """
            {"name":"Camp Nou","city":"Barcelona","country":"Spain",
             "capacity":99354,"surface":"grass"}
            """,
            weather: """
            {"temperature_c":12.0,"description":"light rain","wind_speed":3.4,
             "wind_direction":"NW","humidity_pct":70,"precipitation":0.4,
             "cloud_cover_pct":80,"observed_at":"2026-11-16T14:00:00Z"}
            """)
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)

        #expect(detail.venue?.name == "Camp Nou")
        #expect(detail.venue?.city == "Barcelona")
        #expect(detail.venue?.capacity == 99_354)
        #expect(detail.venue?.displayText == "Camp Nou — Barcelona · 99 354 néző")

        #expect(detail.weather?.temperatureC == 12.0)
        #expect(detail.weather?.condition == "light rain")
        #expect(detail.weather?.windDirection == "NW")
        #expect(detail.weather?.humidityPct == 70)
        #expect(
            detail.weather?.displayText
                == "12 °C · enyhe eső · szél 3,4 m/s (ÉNY) · páratartalom 70%")
    }

    @Test func toleratesNullAndMissingVenueWeather() throws {
        let nulled = try APIDecoding.makeDecoder().decode(
            MatchDetail.self, from: detailJSON(venue: "null", weather: "null"))
        #expect(nulled.venue == nil)
        #expect(nulled.weather == nil)
        #expect(nulled.venue?.displayText == nil)
        #expect(nulled.weather?.displayText == nil)

        let missing = try APIDecoding.makeDecoder().decode(
            MatchDetail.self, from: detailJSON(venue: nil, weather: nil))
        #expect(missing.venue == nil)
        #expect(missing.weather == nil)
        #expect(missing.competitionName == "La Liga")
    }

    @Test func venueDisplaySkipsMissingParts() {
        let bare = VenueInfo(name: "  ")
        #expect(bare.displayText == nil)
        let capacityOnly = VenueInfo(name: "Stadion", capacity: 12_345)
        #expect(capacityOnly.displayText == "Stadion · 12 345 néző")
        let cityOnly = VenueInfo(name: "Stadion", city: "Bécs", capacity: 0)
        #expect(cityOnly.displayText == "Stadion — Bécs")
    }

    @Test func weatherDisplayOnlyIncludesPresentParts() {
        let empty = WeatherInfo()
        #expect(empty.displayText == nil)
        let temperatureOnly = WeatherInfo(temperatureC: -2.5)
        #expect(temperatureOnly.displayText == "-2,5 °C")
        // Ismeretlen leírás és ismeretlen égtáj: a nyers leírás marad, az
        // irány kimarad.
        let raw = WeatherInfo(condition: "Szitáló eső", windSpeed: 2.0, windDirection: "SE")
        #expect(raw.displayText == "Szitáló eső · szél 2 m/s (DK)")
    }
}
