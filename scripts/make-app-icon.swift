#!/usr/bin/env swift
// FootballApp v2 — determinisztikus macOS app-ikon („éjféli stadion", S39 paletta).
// Egyetlen 1024×1024 PNG-t rajzol; az Xcode asset catalog ebbol generalja a Dock-kisz misconception mereteit.
// Hasznalat: swift scripts/make-app-icon.swift <kimenet.png>
import AppKit
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

let args = CommandLine.arguments
guard args.count == 2 else {
    FileHandle.standardError.write(Data("hasznalat: swift make-app-icon.swift <kimenet.png>\n".utf8))
    exit(2)
}

let side: CGFloat = 1024
let space = CGColorSpaceCreateDeviceRGB()

func szin(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> CGColor {
    CGColor(colorSpace: space, components: [r / 255, g / 255, b / 255, a])!
}

let amber = szin(255, 176, 32)       // #FFB020 — kiemeles
let amberVilagos = szin(255, 205, 110)
let grafit = szin(16, 16, 20)        //_panelvonalak
let zold = szin(48, 209, 88)         // #30D158 — elo allapot

guard let ctx = CGContext(
    data: nil,
    width: Int(side),
    height: Int(side),
    bitsPerComponent: 8,
    bytesPerRow: 0,
    space: space,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else { exit(1) }

// 1. Ejszakai grafit alap
let hatter = CGGradient(
    colorsSpace: space,
    colors: [szin(22, 22, 27), szin(6, 6, 8)] as CFArray,
    locations: [0, 1]
)!
ctx.drawLinearGradient(hatter, start: CGPoint(x: 0, y: side), end: .zero, options: [])

let kozep = CGPoint(x: side / 2, y: side / 2 - 14)

// 2. Meleg felfeny a labda mogott
let halo = CGGradient(
    colorsSpace: space,
    colors: [szin(255, 176, 32, 0.20), szin(255, 176, 32, 0.05), szin(255, 176, 32, 0)] as CFArray,
    locations: [0, 0.55, 1]
)!
ctx.drawRadialGradient(halo, startCenter: kozep, startRadius: 110, endCenter: kozep, endRadius: 480, options: [])

// 3. Halvany stadionfeny-sugarak
ctx.saveGState()
ctx.translateBy(x: side / 2, y: side)
for i in -3...3 {
    ctx.saveGState()
    ctx.rotate(by: CGFloat(i) * 0.13)
    let sugar = CGGradient(
        colorsSpace: space,
        colors: [szin(255, 176, 32, 0.045), szin(255, 176, 32, 0)] as CFArray,
        locations: [0, 1]
    )!
    ctx.drawLinearGradient(sugar, start: CGPoint(x: -10, y: 0), end: CGPoint(x: -120, y: -760), options: [])
    ctx.restoreGState()
}
ctx.restoreGState()

let labda = CGPoint(x: side / 2, y: side / 2 - 30)
let r: CGFloat = 246

// 4. Lagy arnyek a labda alatt
ctx.saveGState()
ctx.translateBy(x: labda.x, y: labda.y - 30)
ctx.scaleBy(x: 1, y: 0.3)
let arnyek = CGGradient(
    colorsSpace: space,
    colors: [szin(0, 0, 0, 0.6), szin(0, 0, 0, 0)] as CFArray,
    locations: [0, 1]
)!
ctx.drawRadialGradient(arnyek, startCenter: .zero, startRadius: 40, endCenter: .zero, endRadius: r + 80, options: [])
ctx.restoreGState()

// 5. Labda alapja (enyhe bal felső megvilagitas)
let labdaKitoltes = CGGradient(
    colorsSpace: space,
    colors: [szin(252, 250, 245), szin(221, 217, 207)] as CFArray,
    locations: [0, 1]
)!
ctx.drawRadialGradient(
    labdaKitoltes,
    startCenter: CGPoint(x: labda.x - 70, y: labda.y + 95),
    startRadius: 10,
    endCenter: labda,
    endRadius: r + 35,
    options: []
)

func sokszog(_ kozep: CGPoint, _ sugar: CGFloat, _ oldal: Int, _ forgatas: CGFloat, _ kitoltes: CGColor) {
    let utvonal = CGMutablePath()
    for i in 0..<oldal {
        let a = forgatas + CGFloat(i) * 2 * CGFloat.pi / CGFloat(oldal)
        let p = CGPoint(x: kozep.x + cos(a) * sugar, y: kozep.y + sin(a) * sugar)
        if i == 0 { utvonal.move(to: p) } else { utvonal.addLine(to: p) }
    }
    utvonal.closeSubpath()
    ctx.saveGState()
    ctx.setFillColor(kitoltes)
    ctx.setStrokeColor(grafit)
    ctx.setLineWidth(9)
    ctx.setLineJoin(.round)
    ctx.addPath(utvonal)
    ctx.drawPath(using: .fillStroke)
    ctx.restoreGState()
}

// 6. Otseg + ot kornyezo panelszakasz — kis meretben is felismerheto labda
ctx.saveGState()
ctx.addEllipse(in: CGRect(x: labda.x - r, y: labda.y - r, width: 2 * r, height: 2 * r))
ctx.clip()

sokszog(labda, 94, 5, .pi / 2, amber)
for i in 0..<5 {
    let a = .pi / 2 + CGFloat(i) * 2 * CGFloat.pi / 5 + CGFloat.pi / 5
    let p = CGPoint(x: labda.x + cos(a) * 166, y: labda.y + sin(a) * 166)
    sokszog(p, 72, 5, a + .pi / 2, amberVilagos)
}

// Varratvonalak a kozponti otseg csucsaibol
ctx.setStrokeColor(grafit)
ctx.setLineWidth(9)
ctx.setLineCap(.round)
for i in 0..<5 {
    let a = .pi / 2 + CGFloat(i) * 2 * CGFloat.pi / 5
    ctx.move(to: CGPoint(x: labda.x + cos(a) * 90, y: labda.y + sin(a) * 90))
    ctx.addLine(to: CGPoint(x: labda.x + cos(a) * 250, y: labda.y + sin(a) * 250))
}
ctx.strokePath()
ctx.restoreGState()

ctx.setStrokeColor(szin(255, 255, 255, 0.32))
ctx.setLineWidth(3)
ctx.strokeEllipse(in: CGRect(x: labda.x - r, y: labda.y - r, width: 2 * r, height: 2 * r))

// 7. Ellenorzo iv + „elo" zold点 a szelen
let ivSugar = r + 50
ctx.setLineCap(.round)
ctx.setStrokeColor(szin(255, 176, 32, 0.92))
ctx.setLineWidth(11)
ctx.addArc(center: labda, radius: ivSugar, startAngle: -2.3, endAngle: 1.0, clockwise: false)
ctx.strokePath()

let pontSzog: CGFloat = 1.28
let pont = CGPoint(x: labda.x + cos(pontSzog) * ivSugar, y: labda.y + sin(pontSzog) * ivSugar)
let zoldHalo = CGGradient(
    colorsSpace: space,
    colors: [szin(48, 209, 88, 0.5), szin(48, 209, 88, 0)] as CFArray,
    locations: [0, 1]
)!
ctx.drawRadialGradient(zoldHalo, startCenter: pont, startRadius: 8, endCenter: pont, endRadius: 46, options: [])
ctx.setFillColor(zold)
ctx.fillEllipse(in: CGRect(x: pont.x - 16, y: pont.y - 16, width: 32, height: 32))
ctx.setStrokeColor(szin(255, 255, 255, 0.85))
ctx.setLineWidth(3)
ctx.strokeEllipse(in: CGRect(x: pont.x - 16, y: pont.y - 16, width: 32, height: 32))

guard let kepm = ctx.makeImage(),
      let cel = CGImageDestinationCreateWithURL(
          URL(fileURLWithPath: args[1]) as CFURL,
          UTType.png.identifier as CFString,
          1,
          nil
      ) else { exit(1) }
CGImageDestinationAddImage(cel, kepm, nil)
guard CGImageDestinationFinalize(cel) else { exit(1) }
print("ikon elkeszult: \(args[1]) (\(Int(side))x\(Int(side)))")
