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

    @Test func sourceLinksDecodeFromMappedKeys() throws {
        let detail = try decodeContract()
        let links = try #require(detail.links)
        #expect(links.count == 2)
        #expect(links.first?.kind == "source")
        #expect(links.first?.url == "https://www.fotmob.com/match/6015575")
        #expect(links.last?.kind == "reference")
    }
}