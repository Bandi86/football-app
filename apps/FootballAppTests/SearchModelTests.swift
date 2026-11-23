import XCTest
import FootballAppCore
@testable import FootballApp

private final class StubSearchService: SearchService, @unchecked Sendable {
    private let feed: SearchFeed
    private(set) var callCount = 0

    init(feed: SearchFeed) {
        self.feed = feed
    }

    func search(term: String) async throws -> SearchFeed {
        callCount += 1
        return feed
    }
}

@MainActor
final class SearchModelTests: XCTestCase {
    private var feed: SearchFeed {
        SearchFeed(query: "ujpest", results: [
            SearchResult(kind: "team", id: UUID(), title: "Újpest", subtitle: "Magyarország"),
            SearchResult(kind: "match", id: UUID(), title: "Újpest – Győr", subtitle: "2026-11-22 18:00"),
        ])
    }

    func testShortTermDoesNotCallService() async {
        let service = StubSearchService(feed: feed)
        let model = SearchModel(service: service)
        await model.search("u")
        XCTAssertEqual(service.callCount, 0)
        XCTAssertNil(model.feed)
        XCTAssertEqual(model.status, .idle)
    }

    func testSearchPublishesResults() async {
        let service = StubSearchService(feed: feed)
        let model = SearchModel(service: service)
        await model.search("ujpest")
        XCTAssertEqual(service.callCount, 1)
        XCTAssertEqual(model.status, .loaded)
        XCTAssertEqual(model.feed?.results.count, 2)
        XCTAssertEqual(model.feed?.results.first?.kindLabel, "Csapat")
    }
}
