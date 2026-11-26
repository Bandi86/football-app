import XCTest
import FootballAppCore
@testable import FootballApp

/// A research-blokk üres odds-piacokkal és hibás dátumformátummal szembeni regresszió-tesztjei.
/// A `FootballAppCore` DTO-k közös dekódolóját használja, mint az alkalmazás.
@MainActor
final class MatchInsightViewTests: XCTestCase {
    func testEmptyOddsMarketsAreHidden() throws {
        let insight = try makeInsight(marketsJSON: "{}")
        let view = MatchInsightView(insight: insight)

        XCTAssertFalse(view.showsOddsMarkets)
    }

    func testNonEmptyOddsMarketsAreShown() throws {
        let insight = try makeInsight(marketsJSON: #"{"btts":{"yes":1.72}}"#)
        let view = MatchInsightView(insight: insight)

        XCTAssertTrue(view.showsOddsMarkets)
    }

    func testMissingOddsMarketsAreHidden() throws {
        let insight = try makeInsight(marketsJSON: "null")
        let view = MatchInsightView(insight: insight)

        XCTAssertFalse(view.showsOddsMarkets)
    }

    private func makeInsight(marketsJSON: String) throws -> MatchInsight {
        let json = """
        {"captured_at":"2026-11-22T11:44:31Z","source":"research",
         "odds_markets":{"markets_source":"tipplap","markets_raw":\(marketsJSON)}}
        """
        return try APIDecoding.makeDecoder().decode(MatchInsight.self, from: Data(json.utf8))
    }
}
