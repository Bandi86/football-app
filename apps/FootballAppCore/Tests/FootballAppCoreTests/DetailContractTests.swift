import Foundation
import Testing
@testable import FootballAppCore

/// S96 — a meccs-részletező (`GET /v1/match/{id}`) kontrakt-tesztje.
///
/// Ugyanazt a `tests/fixtures/contract/match-detail-response.json` fájlt dekódolja,
/// amit a Rust oldal (`rust/crates/storage/src/repositories/match_detail.rs`,
/// `contract_tests`) a `MatchDetailRow` szerializációs köréhez mér. Ha a backend
/// mezőt nevez át vagy típust vált, a Rust teszt bukik; ha a kliens `CodingKey`-e
/// csúszik el, ez a teszt bukik.
@Suite struct DetailContractTests {
    private static var fixtureURL: URL {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = directory.appendingPathComponent(
                "tests/fixtures/contract/match-detail-response.json"
            )
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate
            }
            directory.deleteLastPathComponent()
        }
        fatalError("a kontrakt-fixture nem található a #filePath-ból indulva")
    }

    private func fixtureData() throws -> Data {
        try Data(contentsOf: Self.fixtureURL)
    }

    private func decodeContract() throws -> MatchDetail {
        try APIDecoding.makeDecoder().decode(MatchDetail.self, from: fixtureData())
    }

    @Test func contractPayloadDecodesIntoMatchDetail() throws {
        let detail = try decodeContract()
        #expect(detail.status == "finished")
        #expect(detail.homeName == "CD Recoleta")
        #expect(detail.competitionName == "Division Profesional: Clausura")
        #expect(detail.homeTeamId?.uuidString == "01A1FA06-82A2-7139-B52C-D420BD8533EE")
        #expect(detail.awayTeamId?.uuidString == "01A1FC05-0D31-71B7-9867-47F5D5A6A311")
        #expect(detail.homeFt != nil)
        #expect(detail.incidents?.count == 20)
        #expect(detail.gateState == .verifiedPublish)
    }

    @Test func venueAndWeatherDecodeFromMappedKeys() throws {
        let detail = try decodeContract()
        let venue = try #require(detail.venue)
        #expect(venue.name == "Estadio Ricardo Gregor")
        #expect(venue.city == "Luque")
        #expect(venue.country == "Paraguay")
        #expect(venue.capacity == 4000)
        #expect(venue.surface == "grass")

        let weather = try #require(detail.weather)
        #expect(weather.temperatureC == 27.0)
        #expect(weather.condition == "Thunder")
        #expect(weather.windSpeed == 2.0)
        #expect(weather.windDirection == "SE")
        #expect(weather.humidityPct == 86)
        #expect(weather.precipitation == 0.0)
        #expect(weather.cloudCoverPct == 54)
        #expect(weather.observedAt == "2026-11-21T00:30:00Z")
    }

    /// A venue/weather mezők új szerződés-elemek: egy régebbi backend-válaszból
    /// hiányozhatnak, ezért a dekódolásnak ilyenkor sem szabad elbuknia.
    @Test func missingVenueAndWeatherDecodeAsNil() throws {
        var payload = try #require(
            try JSONSerialization.jsonObject(with: fixtureData()) as? [String: Any]
        )
        payload.removeValue(forKey: "venue")
        payload.removeValue(forKey: "weather")
        let data = try JSONSerialization.data(withJSONObject: payload)

        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)
        #expect(detail.venue == nil)
        #expect(detail.weather == nil)
        #expect(detail.homeName == "CD Recoleta")
    }

    /// A `CodingKeys` halmaza pontosan a fixture felső szintű kulcsait fedi le:
    /// új backend-mező a kliensben is megjelenik, eltűnt mező pedig itt bukik.
    @Test func codingKeysMatchTheFixtureKeySet() throws {
        let payload = try #require(
            try JSONSerialization.jsonObject(with: fixtureData()) as? [String: Any]
        )
        let fixtureKeys = Set(payload.keys)
        let clientKeys = Set(MatchDetail.CodingKeys.allCases.map(\.stringValue))
        #expect(fixtureKeys == clientKeys)
    }

    /// S113: a research insight blokk értelmezése, a generált jelzők megőrzése.
    @Test func decodesResearchInsightWithTrustFlags() throws {
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: fixtureData())
        let insight = try #require(detail.insight)
        #expect(insight.source == "research")
        #expect(insight.previewHu?.generated == true)
        #expect(insight.previewHu?.verified == false)
        #expect(insight.prediction?.model == "poisson_v1")
        #expect(insight.prediction?.winProbabilityPct?.away == 48.0)
        #expect(insight.injuries?.items.first?.name == "Antoine Baroan")
        #expect(insight.oddsMarkets?.marketsSource == "tipplap")
        #expect(insight.venueDetails?.capacity == 8400)
        let report = try #require(insight.matchReportHu)
        #expect(report.generated == true)
        #expect(report.verified == false)
        #expect(report.text == "A hazai csapat kezdett jobban.")
    }

    /// S221: a régi kontrakt-fixture továbbra is dekódolható, az új mezők nil.
    @Test func s221FieldsAreOptionalForOldPayloads() throws {
        let detail = try decodeContract()
        #expect(detail.h2hSummary == nil)
        #expect(detail.homeNext?.first?.homeLogo == nil)
        #expect(detail.homeNext?.first?.awayLogo == nil)
    }

    /// S221: az új mezők értékei akkor jelennek meg, ha a backend küldi őket.
    @Test func decodesS221SummaryLogosAndSeasonMetrics() throws {
        var json = try JSONSerialization.jsonObject(with: fixtureData()) as! [String: Any]
        json["h2h_summary"] = [
            "matches": 3,
            "with_stats": 2,
            "total_goals": 7,
            "btts_pct": 33.3,
            "over25_pct": 66.7,
            "corners_avg": 10.0,
            "cards_avg": 3.0,
        ]
        var h2h = json["head_to_head"] as! [[String: Any]]
        if h2h.isEmpty {
            h2h.append([
                "match_id": "01A1E98A-B407-77F6-9AE8-1691E8E88CF5",
                "kickoff_at": "2025-02-05T19:45:00Z",
                "competition_name": "Championship",
                "home_name": "Coventry",
                "away_name": "Leeds United",
                "home_goals": 0,
                "away_goals": 2,
            ])
        }
        h2h[0]["home_corners"] = 4
        h2h[0]["away_corners"] = 6
        h2h[0]["home_yellow"] = 2
        h2h[0]["away_yellow"] = 1
        h2h[0]["home_xg"] = 1.25
        json["head_to_head"] = h2h
        var next = json["home_next"] as! [[String: Any]]
        next[0]["home_logo"] = "/v1/asset/home.webp"
        next[0]["away_logo"] = "/v1/asset/away.webp"
        json["home_next"] = next
        var insight = json["insight"] as! [String: Any]
        insight["season_stats"] = [
            "home": [
                "matches": 11, "goals_for": 14, "goals_against": 9,
                "rank_label": "8/20", "clean_sheets": 4, "assists": 8,
                "possession_pct": 46.8, "average_rating": 7.44,
                "points": 19, "goal_difference": 5,
            ],
            "away": ["matches": 11, "rank_label": "17/20"],
        ]
        json["insight"] = insight

        #expect((h2h[0]["home_corners"] as? Int) == 4)
        #expect((next[0]["home_logo"] as? String) == "/v1/asset/home.webp")
        let data = try JSONSerialization.data(withJSONObject: json)
        let detail = try APIDecoding.makeDecoder().decode(MatchDetail.self, from: data)
        let summary = try #require(detail.h2hSummary)
        #expect(summary.matches == 3)
        #expect(summary.withStats == 2)
        #expect(summary.totalGoals == 7)
        #expect(summary.bttsPct == 33.3)
        #expect(summary.cornersAvg == 10.0)
        #expect(summary.cardsAvg == 3.0)
        #expect(detail.headToHead?.first?.homeCorners == 4)
        #expect(detail.headToHead?.first?.awayCorners == 6)
        #expect(detail.headToHead?.first?.homeYellow == 2)
        #expect(detail.headToHead?.first?.homeXg == 1.25)
        #expect(detail.homeNext?.first?.homeLogo == "/v1/asset/home.webp")
        #expect(detail.homeNext?.first?.awayLogo == "/v1/asset/away.webp")
        let home = try #require(detail.insight?.seasonStats?.home)
        #expect(home.rankLabel == "8/20")
        #expect(home.cleanSheets == 4)
        #expect(home.assists == 8)
        #expect(home.possessionPct == 46.8)
        #expect(home.averageRating == 7.44)
        #expect(home.points == 19)
        #expect(home.goalDifference == 5)
        let away = try #require(detail.insight?.seasonStats?.away)
        #expect(away.cleanSheets == nil)
    }

    @Test func sourceLinksDecodeFromMappedKeys() throws {
        let detail = try decodeContract()
        let links = try #require(detail.links)
        #expect(links.count == 2)
        #expect(links.first?.kind == "source")
        #expect(links.first?.url == "https://www.fotmob.com/match/6015575")
        #expect(links.last?.kind == "reference")
    }
}