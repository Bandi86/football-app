import SwiftUI

/// A kliens vizuális alapelemei — „éjféli stadion" nyelv.
///
/// A háttér kikötés szerint mindig OLED-fekete (`#000000`); a felületek
/// hűvös grafitok, a hangsúly borostyán, az élő állapot zöld. A tipográfia
/// három családra bontható: display (nagy, feszes számok), text (olvasható
/// címkék) és label (kis kapitális, ritkított). Az arányok 4 pt-os rácson
/// ülnek, a kártyák 14–18 pt sugarat és 1 px-es hajszálvonalat kapnak.
enum AppTheme {
    static let background = Color.black
    static let primaryText = Color(hex: 0xF5F5F7)
    static let secondaryText = Color(hex: 0x8E8E93)
    static let tertiaryText = Color(hex: 0x5D5D63)
    static let accent = Color(hex: 0xFFB020)
    static let accentSoft = Color(hex: 0xFFB020).opacity(0.14)
    static let liveGreen = Color(hex: 0x30D158)
    static let warning = Color(hex: 0xFF453A)
    static let surface = Color(hex: 0x0E0E10)
    static let surfaceElevated = Color(hex: 0x16161A)
    static let surfaceSunken = Color(hex: 0x08080A)
    static let hairline = Color.white.opacity(0.08)
    static let hairlineStrong = Color.white.opacity(0.14)

    // Előrejelzés (modell) 1X2 színek
    static let predictionHome = Color(hex: 0x34C759)
    static let predictionDraw = Color(hex: 0x8E8E93)
    static let predictionAway = Color(hex: 0x5E9EFF)

    /// Kártya- és panel-sugarak.
    enum Radius {
        static let card: CGFloat = 18
        static let panel: CGFloat = 14
        static let chip: CGFloat = 9
    }

    /// Rácsegység: minden padding/margó ennek többszöröse.
    enum Spacing {
        static let xs: CGFloat = 4
        static let s: CGFloat = 8
        static let m: CGFloat = 12
        static let l: CGFloat = 18
        static let xl: CGFloat = 24
    }
}

extension Font {
    /// Nagy, feszes, tabuláris számok (eredmény, időpont, odds).
    static func scoreboard(_ size: CGFloat, weight: Font.Weight = .bold) -> Font {
        .system(size: size, weight: weight, design: .rounded).monospacedDigit()
    }

    /// Címek és csapatnevek.
    static func appText(_ size: CGFloat, weight: Font.Weight = .medium) -> Font {
        .system(size: size, weight: weight)
    }

    /// Kis kapitális címkék, ritkítva.
    static func appLabel(_ size: CGFloat = 10) -> Font {
        .system(size: size, weight: .semibold)
    }
}

extension Color {
    init(hex: UInt32, opacity: Double = 1) {
        let red = Double((hex >> 16) & 0xFF) / 255
        let green = Double((hex >> 8) & 0xFF) / 255
        let blue = Double(hex & 0xFF) / 255
        self.init(.sRGB, red: red, green: green, blue: blue, opacity: opacity)
    }
}

/// Egységes kártya-háttér: grafit felület, hajszálvonal, opcionális kiemelés.
struct AppCardStyle: ViewModifier {
    var elevated = true
    var accent = false

    func body(content: Content) -> some View {
        content
            .background(
                RoundedRectangle(cornerRadius: AppTheme.Radius.card, style: .continuous)
                    .fill(elevated ? AppTheme.surfaceElevated : AppTheme.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: AppTheme.Radius.card, style: .continuous)
                    .strokeBorder(accent ? AppTheme.accent.opacity(0.45) : AppTheme.hairline)
            )
            .clipShape(RoundedRectangle(cornerRadius: AppTheme.Radius.card, style: .continuous))
    }
}

extension View {
    func appCard(elevated: Bool = true, accent: Bool = false) -> some View {
        modifier(AppCardStyle(elevated: elevated, accent: accent))
    }

    /// Kis, kapitális szakaszcím.
    func sectionLabel() -> some View {
        self
            .font(.appLabel())
            .tracking(1.4)
            .foregroundStyle(AppTheme.secondaryText)
            .textCase(.uppercase)
    }
}
