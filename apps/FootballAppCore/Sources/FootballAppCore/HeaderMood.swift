import Foundation

/// A fejléc napszaki fázisa. A színek és animációk alapját adja; az időjárás
/// erre tesz felül egy réteget (lásd `HeaderMood.resolve`).
public enum DayPhase: String, CaseIterable, Sendable {
    case night
    case dawn
    case morning
    case daytime
    case golden
    case dusk

    public static func phase(at date: Date, calendar: Calendar = .current) -> DayPhase {
        switch calendar.component(.hour, from: date) {
        case 5..<8: return .dawn
        case 8..<11: return .morning
        case 11..<16: return .daytime
        case 16..<19: return .golden
        case 19..<21: return .dusk
        default: return .night
        }
    }

    public var greeting: String {
        switch self {
        case .night: return "Jó éjszakát."
        case .dawn: return "Jó reggelt."
        case .morning: return "Jó reggelt."
        case .daytime: return "Jó napot."
        case .golden: return "Jó napot."
        case .dusk: return "Jó estét."
        }
    }

    public var label: String {
        switch self {
        case .night: return "Éjszaka"
        case .dawn: return "Hajnal"
        case .morning: return "Reggel"
        case .daytime: return "Nappal"
        case .golden: return "Aranyóra"
        case .dusk: return "Alkonyat"
        }
    }

    public var symbolName: String {
        switch self {
        case .night: return "moon.stars.fill"
        case .dawn, .morning: return "sunrise.fill"
        case .daytime: return "sun.max.fill"
        case .golden: return "sun.max.fill"
        case .dusk: return "sunset.fill"
        }
    }

    public var isDaylight: Bool {
        switch self {
        case .night, .dawn, .dusk: return false
        case .morning, .daytime, .golden: return true
        }
    }
}

/// A háttérben futó animáció típusa. A nézet ezeket retró stílusú, vektoros
/// rétegekké rajzolja; a típus maga tiszta adat, ezért tesztelhető.
public enum HeaderParticle: String, CaseIterable, Sendable {
    case stars
    case orbs
    case rays
    case rain
    case snow
    case storm
}

/// Egy kiszámolt fejléc-hangulat. A színek hex UInt32-ként élnek, hogy a Core
/// ne függjön SwiftUI-tól; a nézet alakítja őket `Color`-rá.
public struct HeaderPalette: Equatable, Sendable {
    public let topHex: UInt32
    public let bottomHex: UInt32
    public let glowHex: UInt32
    public let accentHex: UInt32
    public let particleHex: UInt32
    public let particle: HeaderParticle
    public let symbolName: String
    public let conditionLabel: String
    /// Igaz, ha valódi időjárás-adatból született; hamis esetén a felület
    /// „napszak alapján" jelzést mutat, nem állít élő időjárást.
    public let isLiveWeather: Bool

    public init(
        topHex: UInt32,
        bottomHex: UInt32,
        glowHex: UInt32,
        accentHex: UInt32,
        particleHex: UInt32,
        particle: HeaderParticle,
        symbolName: String,
        conditionLabel: String,
        isLiveWeather: Bool
    ) {
        self.topHex = topHex
        self.bottomHex = bottomHex
        self.glowHex = glowHex
        self.accentHex = accentHex
        self.particleHex = particleHex
        self.particle = particle
        self.symbolName = symbolName
        self.conditionLabel = conditionLabel
        self.isLiveWeather = isLiveWeather
    }
}

public enum HeaderMood {
    /// Tiszta, determinisztikus összeállítás: ugyanabból a napszakból és
    /// időjárásból mindig ugyanaz a paletta (stabil UI-tesztek, nincs véletlen).
    public static func resolve(phase: DayPhase, weather: CurrentWeather?) -> HeaderPalette {
        let base = palette(for: phase)

        guard let weather else {
            return HeaderPalette(
                topHex: base.top, bottomHex: base.bottom, glowHex: base.glow,
                accentHex: base.accent, particleHex: base.particle,
                particle: base.particleKind,
                symbolName: phase.symbolName,
                conditionLabel: phase.label,
                isLiveWeather: false
            )
        }

        let kind = weather.kind
        let symbol = phase.isDaylight ? kind.symbolName : kind.nightSymbolName
        // Erős időjárás (csapadék, zivatar, köd) felülírja a napszak színeit;
        // derült/részben felhős esetén a napszak palettája marad, csak a
        // kísérő ikon és felirat változik.
        let tint = weatherTint(for: kind, phase: phase)
        return HeaderPalette(
            topHex: tint?.top ?? base.top,
            bottomHex: tint?.bottom ?? base.bottom,
            glowHex: tint?.glow ?? base.glow,
            accentHex: tint?.accent ?? base.accent,
            particleHex: tint?.particle ?? base.particle,
            particle: tint?.particleKind ?? base.particleKind,
            symbolName: symbol,
            conditionLabel: kind.hungarianLabel,
            isLiveWeather: true
        )
    }

    // MARK: - Napszaki alap paletták

    private struct BasePalette {
        let top: UInt32
        let bottom: UInt32
        let glow: UInt32
        let accent: UInt32
        let particle: UInt32
        let particleKind: HeaderParticle
    }

    private static func palette(for phase: DayPhase) -> BasePalette {
        switch phase {
        case .night:
            return BasePalette(top: 0x0B1024, bottom: 0x04050B, glow: 0x465AA8,
                               accent: 0x9EABFF, particle: 0xD7DEFF, particleKind: .stars)
        case .dawn:
            return BasePalette(top: 0x2B1B32, bottom: 0x0E0914, glow: 0xC46C8E,
                               accent: 0xFF9A76, particle: 0xFFC9B0, particleKind: .orbs)
        case .morning:
            return BasePalette(top: 0x1A2B36, bottom: 0x070B10, glow: 0x4A9CC2,
                               accent: 0x7ED0E8, particle: 0xBDE6F5, particleKind: .orbs)
        case .daytime:
            return BasePalette(top: 0x16263A, bottom: 0x05090F, glow: 0x2E6FA8,
                               accent: 0x6FB6FF, particle: 0xBBD9FF, particleKind: .rays)
        case .golden:
            return BasePalette(top: 0x32240F, bottom: 0x0C0804, glow: 0xD98732,
                               accent: 0xFFB020, particle: 0xFFD89A, particleKind: .rays)
        case .dusk:
            return BasePalette(top: 0x251628, bottom: 0x0A0610, glow: 0xA04A7A,
                               accent: 0xFF8FA3, particle: 0xFFC3D2, particleKind: .orbs)
        }
    }

    // MARK: - Időjárási felülírások

    private struct WeatherTint {
        let top: UInt32
        let bottom: UInt32
        let glow: UInt32
        let accent: UInt32
        let particle: UInt32
        let particleKind: HeaderParticle
    }

    private static func weatherTint(for kind: WeatherKind, phase: DayPhase) -> WeatherTint? {
        switch kind {
        case .clear, .partlyCloudy:
            // Derült és részben felhős időben a napszak palettája marad: a
            // színek így továbbra is követik az órát, az ikon pedig az időjárást.
            return nil
        case .overcast:
            // Borultnál a napszak színei megmaradnak, csak a mozgás vált
            // lassan lebegő felhőkre a sugarak/helyett.
            let base = palette(for: phase)
            return WeatherTint(top: base.top, bottom: base.bottom, glow: base.glow,
                               accent: base.accent, particle: base.particle,
                               particleKind: .orbs)
        case .fog:
            return WeatherTint(top: 0x1B2024, bottom: 0x080A0C, glow: 0x7A8894,
                               accent: 0xB4C0C9, particle: 0xD6DEE4, particleKind: .orbs)
        case .drizzle, .rain:
            return WeatherTint(top: 0x0F2030, bottom: 0x040910, glow: 0x2E6F9E,
                               accent: 0x5AA9E6, particle: 0x9CCCF0, particleKind: .rain)
        case .snow:
            return WeatherTint(top: 0x1B2733, bottom: 0x090D12, glow: 0x8FA8C0,
                               accent: 0xDCE8F5, particle: 0xF2F7FF, particleKind: .snow)
        case .thunderstorm:
            return WeatherTint(top: 0x1A1426, bottom: 0x05040A, glow: 0x6B4FA8,
                               accent: 0xB08CFF, particle: 0xD6C7FF, particleKind: .storm)
        }
    }
}
