import Testing
@testable import FootballAppCore

@Suite struct AutoRefreshPolicyTests {
    @Test func pastDayWithoutLiveMatchesStillRefreshes() {
        #expect(AutoRefreshPolicy.intervalSeconds(liveMatchCount: 0) == 300)
    }

    @Test func liveMatchesUseTheFasterInterval() {
        #expect(AutoRefreshPolicy.intervalSeconds(liveMatchCount: 1) == 60)
        #expect(AutoRefreshPolicy.intervalSeconds(liveMatchCount: 12) == 60)
    }

    @Test func invalidCountFallsBackToTheIdleInterval() {
        #expect(AutoRefreshPolicy.intervalSeconds(liveMatchCount: -1) == 300)
    }
}
