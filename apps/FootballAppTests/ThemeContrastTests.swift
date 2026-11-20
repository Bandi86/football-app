import AppKit
import Foundation
import SwiftUI
import XCTest

@testable import FootballApp

/// S31a — a téma kontraszt-követelményének automatizált ellenőrzése.
///
/// A kliens szándékosan OLED-fekete, egyetlen sötét palettával dolgozik; a
/// világos megjelenés nem külön renderelési út, ezért a kontrasztot a téma
/// tokenjeire mérjük (WCAG 2.1: normál szövegre 4.5:1, kiemelt szövegre 7:1).
final class ThemeContrastTests: XCTestCase {
    private let surfaces: [(name: String, color: Color)] = [
        ("background", AppTheme.background),
        ("surface", AppTheme.surface),
        ("surfaceElevated", AppTheme.surfaceElevated),
    ]

    private let textTokens: [(name: String, color: Color, minimum: Double)] = [
        ("primaryText", AppTheme.primaryText, 7.0),
        ("secondaryText", AppTheme.secondaryText, 4.5),
        ("accent", AppTheme.accent, 4.5),
        ("liveGreen", AppTheme.liveGreen, 4.5),
        ("warning", AppTheme.warning, 4.5),
    ]

    func testTextTokensMeetContrastOnEverySurface() throws {
        for token in textTokens {
            for surface in surfaces {
                let ratio = try contrastRatio(token.color, surface.color)
                XCTAssertGreaterThanOrEqual(
                    ratio, token.minimum,
                    "\(token.name) a \(surface.name) felületen \(String(format: "%.2f", ratio)):1, "
                        + "a követelmény \(token.minimum):1"
                )
            }
        }
    }

    func testBackgroundIsPureOLEDBlack() {
        XCTAssertEqual(AppTheme.background, Color.black)
    }

    // MARK: - WCAG 2.1 segédek

    private func contrastRatio(_ first: Color, _ second: Color) throws -> Double {
        let lighter = max(try relativeLuminance(first), try relativeLuminance(second))
        let darker = min(try relativeLuminance(first), try relativeLuminance(second))
        return (lighter + 0.05) / (darker + 0.05)
    }

    private func relativeLuminance(_ color: Color) throws -> Double {
        let nsColor = try XCTUnwrap(NSColor(color).usingColorSpace(.sRGB))
        func channel(_ value: CGFloat) -> Double {
            let value = Double(value)
            return value <= 0.03928 ? value / 12.92 : pow((value + 0.055) / 1.055, 2.4)
        }
        return 0.2126 * channel(nsColor.redComponent)
            + 0.7152 * channel(nsColor.greenComponent)
            + 0.0722 * channel(nsColor.blueComponent)
    }
}
