import Foundation
import Testing
@testable import FootballAppCore

/// A három meccs-mikro végpont közös kontrakt-fixture-jeit dekódolja,
/// ugyanazokat, amiket a Rust oldal (`match_micro_contract.rs`) szerializál.
@Suite struct MatchMicroTests {
    private static func fixture(_ name: String) throws -> Data {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = directory.appendingPathComponent(
                "tests/fixtures/contract/\(name)"
            )
            if FileManager.default.fileExists(atPath: candidate.path) {
                return try Data(contentsOf: candidate)
            }
            directory.deleteLastPathComponent()
        }
        fatalError("a kontrakt-fixture nem található: \(name)")
    }

    @Test func lineupsDecodeWithUnresolvedPlayerAsText() throws {
        let payload = try APIDecoding.makeDecoder().decode(
            LineupsPayload.self,
            from: Self.fixture("match-lineups-response.json")
        )
        #expect(payload.lineups.count == 2)
        #expect(payload.lineups[0].playerName == "Kapus Játékos")
        #expect(payload.lineups[1].playerEntityId == nil)
    }

    @Test func playerStatsDecodeNumericValues() throws {
        let payload = try APIDecoding.makeDecoder().decode(
            PlayerStatsPayload.self,
            from: Self.fixture("match-player-stats-response.json")
        )
        #expect(payload.playerStats.count == 2)
        #expect(payload.playerStats[0].value == 90)
        #expect(payload.playerStats[1].statKey == "Goals")
    }

    @Test func shotsDecodeCoordinatesAndXg() throws {
        let payload = try APIDecoding.makeDecoder().decode(
            ShotsPayload.self,
            from: Self.fixture("match-shots-response.json")
        )
        #expect(payload.shots.count == 2)
        #expect(payload.shots[0].xg == 0.18)
        #expect(payload.shots[1].playerName == nil)
    }
}
