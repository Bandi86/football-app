import Foundation
import Testing
@testable import FootballAppCore

/// A közös fixture méri, hogy a Swift DTO és a backend válasza nem csúszik el.
@Suite struct PlayerContractTests {
    private var fixture: Data {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/contract/player-response.json")
        return try! Data(contentsOf: url)
    }

    @Test func decodesPlayerDetailWithAllSections() throws {
        let feed = try APIDecoding.makeDecoder().decode(PlayerFeed.self, from: fixture)
        #expect(feed.player.name == "Jobson")
        #expect(feed.player.countryCode == "BR")
        #expect(feed.spells.count == 1)
        #expect(feed.spells.first?.teamName.isEmpty == false)
        #expect(feed.appearances.count == 1)
        #expect(feed.stats.count == 2)
        #expect(feed.incidents.count == 2)
        #expect(feed.incidents.first?.incidentType == "Substitution")
        #expect(feed.appearances.first?.sourceCode == "research")
    }
}
