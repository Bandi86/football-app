import Foundation
import Testing
@testable import FootballAppCore

/// S31c — a kliens DTO-inak kontrakt-tesztje.
///
/// Ugyanazt a `tests/fixtures/contract/day-response.json` fájlt dekódolja, amit a
/// Rust oldal (`rust/crates/api/tests/day_contract.rs`) a `DayResponse`
/// szerializációjához mér. Ha az API mezőt nevez át vagy típust vált, a Rust teszt
/// bukik; ha a kliens `CodingKey`-e csúszik el, ez a teszt bukik.
@Suite struct DayContractTests {
    private static var fixtureURL: URL {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = directory.appendingPathComponent(
                "tests/fixtures/contract/day-response.json"
            )
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate
            }
            directory.deleteLastPathComponent()
        }
        fatalError("a kontrakt-fixture nem található a #filePath-ból indulva")
    }

    private func decodeContract() throws -> DayResponse {
        let data = try Data(contentsOf: Self.fixtureURL)
        return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data)
    }

    @Test func contractPayloadDecodesIntoDayResponse() throws {
        let response = try decodeContract()
        #expect(response.date == "2026-11-20")
        #expect(response.timezone == "Europe/Budapest")
        #expect(response.matches.count == 2)
        #expect(response.refreshedAt != nil)
    }

    @Test func fullMatchExposesEveryContractField() throws {
        let match = try #require(try decodeContract().matches.first)
        #expect(match.home == "Ferencváros")
        #expect(match.away == "Újpest")
        #expect(match.status == "finished")
        #expect(match.homeScore == 2)
        #expect(match.awayScore == 1)
        #expect(match.publishState == .verifiedPublish)
        #expect(match.homeHt == 1)
        #expect(match.awayHt == 0)
        #expect(match.homePosition == 1)
        #expect(match.awayPosition == 4)
        #expect(match.oddsSource == "footballdata")
        #expect(match.marketCount == 5)
        #expect(match.homeLogo == "hu/ftc.png")
        #expect(match.awayElo == 1480.25)
        #expect(match.oddsTrustState == "trusted")
        #expect(match.oddsCapturedAt != nil)

        #expect(match.blockers.count == 1)
        #expect(match.blockers.first?.ruleKey == "odds_freshness")
        #expect(match.blockers.first?.severity == "warning")

        let odds = try #require(match.odds)
        #expect(odds.home == 1.85)
        #expect(odds.draw == 3.4)
        #expect(odds.away == 4.2)
        #expect(odds.bttsYes == 1.72)
        #expect(odds.over25 == 1.9)
        #expect(odds.under25 == 1.95)
    }

    @Test func minimalMatchKeepsMissingDataAsNil() throws {
        let matches = try decodeContract().matches
        #expect(matches.count == 2)
        let minimal = matches[1]
        #expect(minimal.home == "Debrecen")
        #expect(minimal.status == "scheduled")
        #expect(minimal.homeScore == nil)
        #expect(minimal.awayScore == nil)
        #expect(minimal.publishState == .unknown)
        #expect(minimal.blockers.isEmpty)
        #expect(minimal.odds == nil)
        #expect(minimal.marketCount == nil)
        #expect(minimal.homeElo == nil)
    }
}
