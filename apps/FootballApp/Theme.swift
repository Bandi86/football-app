import SwiftUI

/// A kliens vizuális alapelemei. A háttér kikötés szerint mindig OLED-fekete
/// (`#000000`); a többi szín és a tipográfia ehhez igazodik.
enum AppTheme {
    static let background = Color.black
    static let primaryText = Color(hex: 0xF5F5F7)
    static let secondaryText = Color(hex: 0x8E8E93)
    static let accent = Color(hex: 0xFFB020)
    static let liveGreen = Color(hex: 0x30D158)
    static let warning = Color(hex: 0xFF453A)
    static let surface = Color(hex: 0x0E0E10)
    static let surfaceElevated = Color(hex: 0x16161A)
    static let hairline = Color.white.opacity(0.08)

    // Előrejelzés (modell) 1X2 színek
    static let predictionHome = Color(hex: 0x34C759)
    static let predictionDraw = Color(hex: 0x8E8E93)
    static let predictionAway = Color(hex: 0x5E9EFF)
}

extension Color {
    init(hex: UInt32, opacity: Double = 1) {
        let red = Double((hex >> 16) & 0xFF) / 255
        let green = Double((hex >> 8) & 0xFF) / 255
        let blue = Double(hex & 0xFF) / 255
        self.init(.sRGB, red: red, green: green, blue: blue, opacity: opacity)
    }
}
