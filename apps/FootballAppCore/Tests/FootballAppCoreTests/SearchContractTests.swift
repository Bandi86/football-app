import Foundation
import Testing
@testable import FootballAppCore

/// A közös fixture azt méri, hogy a Swift DTO és a backend válasza nem csúszik el.
@Suite struct SearchContractTests {
    private var fixture: Data {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // FootballAppCoreTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // FootballAppCore
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo
            .appendingPathComponent("tests/fixtures/contract/search-response.json")
        return try! Data(contentsOf: url)
    }

    @Test func decodesAllResultKinds() throws {
        let feed = try APIDecoding.makeDecoder().decode(SearchFeed.self, from: fixture)
        #expect(feed.query == "ujpest")
        #expect(feed.results.count == 3)
        #expect(feed.results.map(\.kind) == ["team", "competition", "match"])
        #expect(feed.results[0].title == "Újpest")
        #expect(feed.results[0].subtitle == "Magyarország")
        #expect(feed.results[2].kindLabel == "Meccs")
    }

    @Test func emptyResultsAreNotAnError() throws {
        let json = "{\"query\":\"nincs\",\"results\":[]}".data(using: .utf8)!
        let feed = try APIDecoding.makeDecoder().decode(SearchFeed.self, from: json)
        #expect(feed.results.isEmpty)
    }
}
