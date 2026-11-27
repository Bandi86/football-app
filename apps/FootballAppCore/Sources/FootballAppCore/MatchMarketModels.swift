import Foundation

/// Egy részletes odds-piac kimenetele a `/v1/match/{id}/markets` végpontról.
/// A nyers `market`/`outcome` értékek megőrizendők: a kliens a magyar címkét
/// csak megjelenítésre képezi le, visszaírást nem végez.
public struct MarketQuoteDTO: Decodable, Equatable, Sendable {
    public let market: String
    public let outcome: String
    public let line: Double
    public let price: Double
    public let bookmaker: String
    public let phase: String
    public let sourceCode: String
    public let capturedAt: Date

    enum CodingKeys: String, CodingKey {
        case market, outcome, line, price, bookmaker, phase
        case sourceCode = "source_code"
        case capturedAt = "captured_at"
    }
}

public struct MarketsResponseDTO: Decodable, Equatable, Sendable {
    public let quotes: [MarketQuoteDTO]
    public let count: Int

    public init(quotes: [MarketQuoteDTO], count: Int) {
        self.quotes = quotes
        self.count = count
    }
}

/// Megjelenítési csoport: egy piac az összes kimenetelével. A `title` magyar,
/// a `key` a nyers piacazonosító — a tesztek és a VoiceOver is ezeket használja.
public struct MarketGroup: Equatable, Sendable {
    public let key: String
    public let title: String
    public let quotes: [MarketQuoteDTO]

    public init(key: String, title: String, quotes: [MarketQuoteDTO]) {
        self.key = key
        self.title = title
        self.quotes = quotes
    }
}

/// A piacok csoportosítása és magyar címkézése. A párosítás determinisztikus,
/// találgatás nincs: ismeretlen piacnál a nyers azonosítót mutatjuk.
public enum MarketDisplay {
    public static func title(for market: String) -> String {
        switch market {
        case "1x2": return "1X2"
        case "double_chance": return "Kétesély"
        case "dnb": return "Döntetlen nélkül"
        case "btts": return "Mindkét csapat gólt szerez"
        case "ou": return "Gólszám"
        case "total_goals_band": return "Gólszám sáv"
        case "handicap_-1": return "Hendikep −1"
        case "handicap_+1": return "Hendikep +1"
        case "handicap_-2": return "Hendikep −2"
        case "handicap_+2": return "Hendikep +2"
        case "home_clean_sheet": return "Hazai csapat kapott gól nélkül"
        case "away_clean_sheet": return "Vendég csapat kapott gól nélkül"
        case "exact_score": return "Pontos végeredmény"
        case "ht_1x2": return "Félidő — 1X2"
        case "ht_double_chance": return "Félidő — Kétesély"
        default:
            if market.hasPrefix("handicap_") {
                let raw = market.dropFirst("handicap_".count)
                    .split(separator: ":", maxSplits: 1).first.map(String.init) ?? ""
                return "Hendikep \(raw.replacingOccurrences(of: "_", with: " "))"
            }
            return "Egyéb piac"
        }
    }

    public static func outcomeLabel(_ outcome: String) -> String {
        switch outcome {
        case "home": return "Hazai"
        case "draw": return "Döntetlen"
        case "away": return "Vendég"
        case "yes": return "Igen"
        case "no": return "Nem"
        case "over": return "Több"
        case "under": return "Kevesebb"
        case "1x": return "1X"
        case "12": return "12"
        case "2x": return "2X"
        default: return outcome
        }
    }

    public static func priceText(_ price: Double) -> String {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.minimumFractionDigits = 2
        formatter.maximumFractionDigits = 2
        return formatter.string(from: NSNumber(value: price)) ?? String(format: "%.2f", price)
    }

    /// A részletes piacokat azonos című csoportokba rendezve adja vissza; a
    /// sorrend a nyers piacazonosítóé (stabil, a forrástól jön).
    public static func groups(_ quotes: [MarketQuoteDTO]) -> [MarketGroup] {
        var order: [String] = []
        var byKey: [String: [MarketQuoteDTO]] = [:]
        for quote in quotes {
            if byKey[quote.market] == nil { order.append(quote.market) }
            byKey[quote.market, default: []].append(quote)
        }
        return order.compactMap { key in
            guard let rows = byKey[key] else { return nil }
            return MarketGroup(key: key, title: title(for: key), quotes: rows)
        }
    }
}
