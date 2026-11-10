import XCTest
@testable import FootballApp

final class MatchStatusLabelTests: XCTestCase {
    func testKnownStatusesAreTranslatedToHungarian() {
        let expected: [String: String] = [
            "scheduled": "Ütemezett",
            "confirmed": "Megerősített",
            "live": "Élő",
            "halftime": "Félidő",
            "finished": "Lezárt",
            "postponed": "Elhalasztott",
            "canceled": "Törölt",
            "abandoned": "Félbeszakadt",
            "awarded": "Megítélt",
        ]
        for (raw, hungarian) in expected {
            XCTAssertEqual(MatchRowView.statusLabel(for: raw), hungarian, "Státusz: \(raw)")
        }
    }

    func testUnknownStatusFallsBackToRawValue() {
        XCTAssertEqual(MatchRowView.statusLabel(for: "valami_uj"), "valami_uj")
        XCTAssertEqual(MatchRowView.statusLabel(for: ""), "")
    }
}
