import SwiftUI
import FootballAppCore

/// Shadow predikció megjelenítése az Odds fülben. Tudatosan **nem** fogadási
/// tanács: a modell nem ellenőrzött, a piacnál gyengébbnek mértük (l. S199).
struct PredictionCard: View {
    let prediction: ShadowPrediction
    let market: OddsSnapshot?

    private var rows: [(String, Double, Double?)] {
        [
            ("Hazai", prediction.home, prediction.fairHome),
            ("Döntetlen", prediction.draw, prediction.fairDraw),
            ("Vendég", prediction.away, prediction.fairAway),
        ]
    }

    private static let percent: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.numberStyle = .percent
        formatter.maximumFractionDigits = 0
        return formatter
    }()

    private static let time: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Text("Modell-előrejelzés")
                    .font(.appText(14, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                Text("NEM ELLENŐRZÖTT · SHADOW")
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.secondaryText)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(AppTheme.surface, in: Capsule())
                    .accessibilityIdentifier("match.prediction")
            }
            ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                HStack(spacing: 8) {
                    Text(row.0)
                        .font(.appText(12.5))
                        .foregroundStyle(AppTheme.primaryText)
                    Text(Self.percent.string(from: NSNumber(value: row.1)) ?? "")
                        .font(.system(size: 12.5, weight: .semibold).monospacedDigit())
                        .foregroundStyle(AppTheme.primaryText)
                    Spacer()
                    if let fair = row.2 {
                        Text("fair \(MarketDisplay.priceText(fair))")
                            .font(.appLabel(10))
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel("\(row.0), modell \(Self.percent.string(from: NSNumber(value: row.1)) ?? ""), nem ellenőrzött")
                .accessibilityIdentifier("match.prediction.row")
            }
            Text("Nem fogadási tanács. A modellt a piacnál gyengébbnek mértük.")
                .font(.appLabel(9))
                .foregroundStyle(AppTheme.secondaryText)
            Text("\(prediction.modelVersion) · \(Self.time.string(from: prediction.asOf))")
                .font(.appLabel(9))
                .foregroundStyle(AppTheme.secondaryText)
                .accessibilityIdentifier("match.prediction.source")
        }
        .padding(12)
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 12))
    }
}
