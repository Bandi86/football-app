import XCTest
@testable import FootballApp

/// S213: a bal oszlop szélessége pontosan a kártya tartalmi minimumából jön.
/// Ezek a számok a valódi fix oszlopok összegei, nem tetszőleges százalékok.
final class MatchListLayoutTests: XCTestCase {
    func testCardMinimumIsDerivedFromRealColumnWidths() {
        XCTAssertEqual(MatchItemRow.kickoffWidth, 76)
        XCTAssertEqual(MatchItemRow.teamMinWidth, 190)
        XCTAssertEqual(MatchItemRow.scoreMinWidth, 56)
        XCTAssertEqual(MatchItemRow.oddsWidth, 180)
        XCTAssertEqual(MatchItemRow.minimumContentWidth, 574)
    }

    func testWideWindowGivesTheListExactlyTheNeededWidth() {
        let columns = MatchListView.columnWidths(totalWidth: 1440)
        XCTAssertTrue(columns.twoColumns)
        XCTAssertEqual(columns.list, 574)
        XCTAssertEqual(columns.side, 754)
        XCTAssertEqual(columns.list + columns.side
                       + MatchListView.columnSpacing
                       + 2 * MatchListView.listHorizontalPadding, 1440)
    }

    func testNarrowWideEnoughWindowKeepsBothColumnsFitting() {
        let columns = MatchListView.columnWidths(totalWidth: 1200)
        XCTAssertEqual(columns.list, 552) // a felé már kevesebb, mint a minimum
        XCTAssertEqual(columns.side, 536)
    }

    func testAtBreakEvenBothColumnsExactlyFit() {
        let columns = MatchListView.columnWidths(totalWidth: 1284)
        XCTAssertEqual(columns.list, 574)
        XCTAssertEqual(columns.side, 598)
    }

    func testSmallWindowUsesOneFullWidthColumn() {
        let columns = MatchListView.columnWidths(totalWidth: 900)
        XCTAssertFalse(columns.twoColumns)
        XCTAssertEqual(columns.list, 804)
        XCTAssertEqual(columns.side, 0)
    }
}
