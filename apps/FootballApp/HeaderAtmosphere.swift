import SwiftUI
import FootballAppCore

/// A fejléc élő háttere: napszaki színátmenet, vándorló fényfoltok és az
/// időjáráshoz illő részecske-animáció. Minden rajzolás vektoros `Canvas`,
/// nincs benne kép- vagy hálózati függőség. A `reduceMotion` teljesen
/// kikapcsolja a mozgást: ilyenkor egyetlen statikus képkocka látszik.
struct HeaderAtmosphere: View {
    let palette: HeaderPalette
    let reduceMotion: Bool

    var body: some View {
        ZStack {
            LinearGradient(
                colors: [Color(hex: palette.topHex), Color(hex: palette.bottomHex)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )

            if reduceMotion {
                Canvas { context, size in
                    paint(context: &context, size: size, time: 0)
                }
                .opacity(0.55)
            } else {
                TimelineView(.animation(minimumInterval: 1.0 / 30)) { timeline in
                    Canvas { context, size in
                        paint(context: &context, size: size,
                              time: timeline.date.timeIntervalSinceReferenceDate)
                    }
                }
            }
        }
        .clipped()
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    // MARK: - Rajzolás

    private func paint(context: inout GraphicsContext, size: CGSize, time: Double) {
        guard size.width > 0, size.height > 0 else { return }
        paintGlow(context: &context, size: size, time: time)
        paintParticles(context: &context, size: size, time: time)
        paintHorizon(context: &context, size: size)
    }

    private func paintGlow(context: inout GraphicsContext, size: CGSize, time: Double) {
        let glow = Color(hex: palette.glowHex)
        let stops = Gradient(colors: [glow.opacity(0.42), glow.opacity(0.0)])
        // Halvány, fix akcentus-glóbusz a dátum mögött: mélységet ad anélkül,
        // hogy a szöveg olvashatóságát rontaná.
        let accent = Color(hex: palette.accentHex)
        let heroCenter = CGPoint(x: size.width * 0.12, y: size.height * 0.5)
        let heroRadius = size.width * 0.42
        let heroRect = CGRect(x: heroCenter.x - heroRadius, y: heroCenter.y - heroRadius,
                              width: heroRadius * 2, height: heroRadius * 2)
        context.fill(Path(ellipseIn: heroRect), with: .radialGradient(
            Gradient(colors: [accent.opacity(0.16), accent.opacity(0)]),
            center: heroCenter, startRadius: 0, endRadius: heroRadius
        ))

        for index in 0..<3 {
            let x = size.width * (0.18 + 0.32 * unit(index, salt: 1))
                + sin(time * 0.11 + Double(index)) * size.width * 0.05
            let y = size.height * (0.08 + 0.5 * unit(index, salt: 2))
                + cos(time * 0.09 + Double(index)) * size.height * 0.06
            let radius = size.width * (0.22 + 0.2 * unit(index, salt: 3))
            let center = CGPoint(x: x, y: y)
            let rect = CGRect(x: x - radius, y: y - radius,
                              width: radius * 2, height: radius * 2)
            context.fill(Path(ellipseIn: rect), with: .radialGradient(
                stops, center: center, startRadius: 0, endRadius: radius
            ))
        }
    }

    private func paintHorizon(context: inout GraphicsContext, size: CGSize) {
        let accent = Color(hex: palette.accentHex)
        var line = Path()
        line.move(to: CGPoint(x: 0, y: size.height - 1))
        line.addLine(to: CGPoint(x: size.width, y: size.height - 1))
        context.stroke(line, with: .color(accent.opacity(0.22)), lineWidth: 1)
    }

    private func paintParticles(context: inout GraphicsContext, size: CGSize, time: Double) {
        let color = Color(hex: palette.particleHex)
        switch palette.particle {
        case .stars:
            for index in 0..<38 {
                let x = size.width * unit(index, salt: 4)
                let y = size.height * unit(index, salt: 5) * 0.88
                let radius = 0.6 + unit(index, salt: 6) * 1.3
                let twinkle = 0.25 + 0.75 * abs(sin(time * 1.3 + unit(index, salt: 7) * .pi * 2))
                let rect = CGRect(x: x - radius, y: y - radius,
                                  width: radius * 2, height: radius * 2)
                context.fill(Path(ellipseIn: rect), with: .color(color.opacity(twinkle)))
            }

        case .orbs:
            for index in 0..<11 {
                let x = size.width * unit(index, salt: 8)
                    + sin(time * 0.25 + Double(index)) * 12
                let y = size.height * unit(index, salt: 9) * 0.82
                    + cos(time * 0.2 + Double(index)) * 10
                let radius = 1.4 + unit(index, salt: 10) * 2.6
                let rect = CGRect(x: x - radius, y: y - radius,
                                  width: radius * 2, height: radius * 2)
                context.fill(Path(ellipseIn: rect),
                             with: .color(color.opacity(0.18 + 0.3 * unit(index, salt: 11))))
            }

        case .rays:
            let center = CGPoint(x: size.width * 0.86, y: size.height * 0.04)
            for index in 0..<16 {
                let angle = Double(index) / 16 * .pi * 2 + time * 0.045
                let inner = size.height * 0.2
                let outer = size.height * (0.48 + unit(index, salt: 12) * 0.4)
                var ray = Path()
                ray.move(to: CGPoint(x: center.x + cos(angle) * inner,
                                     y: center.y + sin(angle) * inner))
                ray.addLine(to: CGPoint(x: center.x + cos(angle) * outer,
                                        y: center.y + sin(angle) * outer))
                context.stroke(ray, with: .color(color.opacity(0.07)), lineWidth: 1.6)
            }

        case .rain, .storm:
            let count = palette.particle == .storm ? 34 : 48
            for index in 0..<count {
                let speed = 170 + unit(index, salt: 13) * 190
                let x = size.width * unit(index, salt: 14)
                let y = wrapped(unit(index, salt: 15) * size.height + time * speed,
                                size.height)
                let length = 8 + unit(index, salt: 16) * 13
                var drop = Path()
                drop.move(to: CGPoint(x: x, y: y))
                drop.addLine(to: CGPoint(x: x - length * 0.3, y: y + length))
                context.stroke(drop, with: .color(color.opacity(0.26)), lineWidth: 1)
            }
            if palette.particle == .storm {
                // Ritka, rövid villanás: a ciklus egy apró szeletében él.
                let flash = time.truncatingRemainder(dividingBy: 7)
                if flash < 0.1 {
                    context.fill(Path(CGRect(origin: .zero, size: size)),
                                 with: .color(Color(hex: palette.accentHex).opacity(0.12)))
                }
            }

        case .snow:
            for index in 0..<34 {
                let speed = 18 + unit(index, salt: 17) * 28
                let sway = sin(time * 0.8 + unit(index, salt: 18) * .pi * 2) * 9
                let x = size.width * unit(index, salt: 19) + sway
                let y = wrapped(unit(index, salt: 20) * size.height + time * speed,
                                size.height)
                let radius = 1 + unit(index, salt: 21) * 1.7
                let rect = CGRect(x: x - radius, y: y - radius,
                                  width: radius * 2, height: radius * 2)
                context.fill(Path(ellipseIn: rect), with: .color(color.opacity(0.55)))
            }
        }
    }

    // MARK: - Determinista segéd

    /// Stabil pszeudo-véletlen [0,1) érték — nincs `Random`, így a UI-tesztek
    /// és a két képkocka közötti rajzolás is kiszámítható.
    private func unit(_ index: Int, salt: Double) -> Double {
        let value = sin(Double(index) * 12.9898 + salt * 78.233) * 43_758.5453
        return value - floor(value)
    }

    private func wrapped(_ value: Double, _ height: Double) -> Double {
        guard height > 0 else { return 0 }
        return value.truncatingRemainder(dividingBy: height)
    }
}
