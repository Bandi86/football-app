import Foundation
import Testing
@testable import FootballAppCore

@Suite struct MatchMarketModelsTests {
    private let json = """
    {"quotes":[
      {"market":"btts","outcome":"yes","line":0,"price":1.53,"bookmaker":"Tippmix",
       "phase":"opening","source_code":"tipplap","captured_at":"2026-11-27T08:00:00Z"},
      {"market":"btts","outcome":"no","line":0,"price":2.19,"bookmaker":"Tippmix",
       "phase":"opening","source_code":"tipplap","captured_at":"2026-11-27T08:00:00Z"},
      {"market":"ou","outcome":"over","line":2.5,"price":1.44,"bookmaker":"Tippmix",
       "phase":"opening","source_code":"tipplap","captured_at":"2026-11-27T08:00:00Z"},
      {"market":"handicap_-1:101080935","outcome":"home","line":1,"price":6.00,
       "bookmaker":"Tippmix","phase":"opening","source_code":"tipplap",
       "captured_at":"2026-11-27T08:00:00Z"}
    ],"count":4}
    """

    @Test func decodesQuotesWithSourceAndCapture() throws {
        let response = try APIDecoding.makeDecoder().decode(
            MarketsResponseDTO.self, from: Data(json.utf8)
        )
        #expect(response.count == 4)
        #expect(response.quotes.count == 4)
        #expect(response.quotes[0].sourceCode == "tipplap")
        #expect(response.quotes[2].line == 2.5)
        #expect(Calendar.current.isDate(response.quotes[0].capturedAt, equalTo:
            Date(timeIntervalSince1970: 1_795_766_400), toGranularity: .second))
    }

    @Test func groupsPreserveOrderAndHungarianLabels() throws {
        let response = try APIDecoding.makeDecoder().decode(
            MarketsResponseDTO.self, from: Data(json.utf8)
        )
        let groups = MarketDisplay.groups(response.quotes)
        #expect(groups.count == 3)
        #expect(groups.map(\.key) == ["btts", "ou", "handicap_-1:101080935"])
        #expect(groups[0].title == "Mindkét csapat gólt szerez")
        #expect(groups[0].quotes.map { $0.outcome } == ["yes", "no"])
        #expect(groups[1].title == "Gólszám")
        #expect(groups[2].title == "Hendikep -1")
    }

    @Test func outcomeAndPriceLabelsAreHungarian() {
        #expect(MarketDisplay.outcomeLabel("home") == "Hazai")
        #expect(MarketDisplay.outcomeLabel("over") == "Több")
        #expect(MarketDisplay.outcomeLabel("1x") == "1X")
        #expect(MarketDisplay.priceText(1.5) == "1,50")
    }

    @Test func unknownMarketKeepsRawNameInsteadOfInventingOne() {
        #expect(MarketDisplay.title(for: "other:12345") == "Egyéb piac")
        #expect(MarketDisplay.title(for: "1x2") == "1X2")
    }
}
