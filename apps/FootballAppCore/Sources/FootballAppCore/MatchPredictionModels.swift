import Foundation

/// Egy shadow predikciós sor a `/v1/match/{id}/prediction` végpontról.
public struct PredictionRowDTO: Decodable, Equatable, Sendable {
    public let modelVersion: String
    public let market: String
    public let outcome: String
    public let line: Double
    public let probability: Double
    public let fairOdds: Double?
    public let edge: Double?
    public let quality: String
    public let inputsHash: String
    public let asOf: Date

    enum CodingKeys: String, CodingKey {
        case modelVersion = "model_version"
        case market, outcome, line, probability
        case fairOdds = "fair_odds"
        case edge, quality
        case inputsHash = "inputs_hash"
        case asOf = "as_of"
    }
}

public struct PredictionResponseDTO: Decodable, Equatable, Sendable {
    public let rows: [PredictionRowDTO]
    public let count: Int

    public init(rows: [PredictionRowDTO], count: Int) {
        self.rows = rows
        self.count = count
    }
}

/// Megjelenítési modell: a legfrissebb modellverzió 1X2 sorai. Hiány esetén
/// `nil` — nem találunk ki valószínűséget, és nem adunk fogadási tanácsot.
public struct ShadowPrediction: Equatable, Sendable {
    public let modelVersion: String
    public let asOf: Date
    public let quality: String
    public let home: Double
    public let draw: Double
    public let away: Double
    public let fairHome: Double?
    public let fairDraw: Double?
    public let fairAway: Double?
    public let over25: Double?
    public let under25: Double?
    public let bttsYes: Double?
    public let bttsNo: Double?

    public init?(dto: PredictionResponseDTO) {
        guard let first = dto.rows.first else { return nil }
        func row(_ outcome: String) -> PredictionRowDTO? {
            dto.rows.first { $0.outcome == outcome && $0.modelVersion == first.modelVersion }
        }
        guard let h = row("home"), let d = row("draw"), let a = row("away") else { return nil }
        self.modelVersion = first.modelVersion
        self.asOf = first.asOf
        self.quality = first.quality
        self.home = h.probability
        self.draw = d.probability
        self.away = a.probability
        self.fairHome = h.fairOdds
        self.fairDraw = d.fairOdds
        self.fairAway = a.fairOdds
        func market(_ name: String, _ outcome: String) -> PredictionRowDTO? {
            dto.rows.first {
                $0.market == name && $0.outcome == outcome && $0.modelVersion == first.modelVersion
            }
        }
        self.over25 = market("ou", "over")?.probability
        self.under25 = market("ou", "under")?.probability
        self.bttsYes = market("btts", "yes")?.probability
        self.bttsNo = market("btts", "no")?.probability
    }

    public var probabilityText: String {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.numberStyle = .percent
        formatter.maximumFractionDigits = 0
        func text(_ value: Double) -> String {
            formatter.string(from: NSNumber(value: value)) ?? "\(Int(value * 100))%"
        }
        return "\(text(home)) / \(text(draw)) / \(text(away))"
    }
}
