import SwiftUI
import FootballAppCore

/// A research insight progresszív megjelenítése a meccsrészletezőben.
/// A generált/nem ellenőrzött blokkok szövegesen is jelölve vannak —
/// a szín nem egyedüli hordozó.
struct MatchInsightView: View {
    let insight: MatchInsight

    @State private var showDetails = false
    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy. mm. dd. HH:mm"
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.m) {
            HStack(spacing: 6) {
                Image(systemName: "sparkles")
                    .font(.system(size: 11, weight: .semibold))
                Text("Research · \(Self.timeFormatter.string(from: insight.capturedAt)) · \(insight.source)")
                    .font(.appLabel(9))
                Spacer()
                Text("Számított / nem ellenőrzött")
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.warning)
            }
            .foregroundStyle(AppTheme.secondaryText)
            .accessibilityIdentifier("match.insight.trust")
            .accessibilityIdentifier("match.insight")
            .accessibilityLabel("Research adat, rögzítve \(Self.timeFormatter.string(from: insight.capturedAt)). A számított tartalom nem ellenőrzött.")

            if let preview = insight.previewHu {
                section("Előnézet") {
                    if let title = preview.title, !title.isEmpty {
                        Text(title)
                            .font(.appText(13, weight: .semibold))
                            .foregroundStyle(AppTheme.primaryText)
                    }
                    ForEach(Array((preview.sections ?? []).prefix(6).enumerated()), id: \.offset) { _, item in
                        if let heading = item.heading, let text = item.text {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(heading).font(.appLabel(9)).foregroundStyle(AppTheme.accent)
                                Text(text).font(.appText(12)).foregroundStyle(AppTheme.secondaryText)
                            }
                            .accessibilityIdentifier("match.insight.preview.section")
                        }
                    }
                    if preview.verified == false {
                        Text("Generált előnézet — nem ellenőrzött forrásállítás.")
                            .font(.appText(10))
                            .foregroundStyle(AppTheme.warning)
                            .accessibilityIdentifier("match.insight.preview.warning")
                    }
                }
            }

            if let prediction = insight.prediction {
                section("Előrejelzés") {
                    if let p = prediction.winProbabilityPct {
                        HStack(spacing: 12) {
                            probability("Hazai", p.home)
                            probability("Döntetlen", p.draw)
                            probability("Vendég", p.away)
                        }
                        .accessibilityIdentifier("match.insight.probabilities")
                    }
                    if let goals = prediction.expectedGoals {
                        Text("Várható gólszám: \(Self.number(goals.total)) (hazai \(Self.number(goals.home)) – vendég \(Self.number(goals.away)))")
                            .font(.appText(11))
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    if let score = prediction.mostLikelyScore {
                        Text("Legvalószínűbb eredmény: \(score.home ?? 0)–\(score.away ?? 0) (\(Self.number(score.probabilityPct))%)")
                            .font(.appText(11))
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    Text("Statisztikai modell (\(prediction.model ?? "?")), nem forrás-tény.")
                        .font(.appText(10))
                        .foregroundStyle(AppTheme.warning)
                        .accessibilityIdentifier("match.insight.prediction.warning")
                }
            }

            if let injuries = insight.injuries?.items, !injuries.isEmpty {
                section("Hiányzók") {
                    ForEach(Array(injuries.prefix(6).enumerated()), id: \.offset) { _, injury in
                        row(
                            injury.name ?? "Ismeretlen játékos",
                            [injury.type, injury.expectedReturn].compactMap { $0 }.joined(separator: " · "),
                            identifier: "match.insight.injury"
                        )
                    }
                }
            }

            DisclosureGroup("További research-adatok", isExpanded: $showDetails) {
                VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
                    if let streaks = insight.h2hStreaks {
                        section("Egymás elleni trend") {
                            row("Hazai", "Győzelem nélkül: \(streaks.home?.noWin.map(String.init) ?? "–")", identifier: "match.insight.streak")
                            row("Vendég", "Győzelem nélkül: \(streaks.away?.noWin.map(String.init) ?? "–")", identifier: "match.insight.streak")
                        }
                    }
                    if let standings = insight.standings {
                        section("Tabella-pillanatkép") {
                            row(standings.home?.team ?? "Hazai", "\(standings.home?.position.map { "\($0)." } ?? "–") \(standings.home?.points.map(String.init) ?? "–") pont", identifier: "match.insight.standing")
                            row(standings.away?.team ?? "Vendég", "\(standings.away?.position.map { "\($0)." } ?? "–") \(standings.away?.points.map(String.init) ?? "–") pont", identifier: "match.insight.standing")
                        }
                    }
                    if let markets = insight.oddsMarkets?.marketsRaw {
                        section("Fogadási piacok (\(markets.count))") {
                            ForEach(markets.keys.sorted().prefix(8), id: \.self) { market in
                                let items = markets[market] ?? [:]
                                row(market, items.keys.sorted().prefix(3).map { "\($0): \(Self.number(items[$0]))" }.joined(separator: " · "), identifier: "match.insight.market")
                            }
                        }
                    }
                    if let venue = insight.venueDetails {
                        section("Helyszín részletei") {
                            row("Stadion", [venue.name, venue.city].compactMap { $0 }.joined(separator: ", "), identifier: "match.insight.venue")
                            row("Kapacitás / borítás", "\(venue.capacity.map(String.init) ?? "–") / \(venue.surface ?? "–")", identifier: "match.insight.venue")
                        }
                    }
                }
            }
            .font(.appText(12))
            .foregroundStyle(AppTheme.primaryText)
            .accessibilityIdentifier("match.insight.details")
        }
        .padding(AppTheme.Spacing.m)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func section(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title).font(.appLabel(9)).foregroundStyle(AppTheme.secondaryText)
            content()
        }
    }

    private func probability(_ label: String, _ value: Double?) -> some View {
        VStack(spacing: 1) {
            Text(label).font(.appLabel(9)).foregroundStyle(AppTheme.secondaryText)
            Text(value.map { "\(Int($0.rounded()))%" } ?? "–")
                .font(.scoreboard(14))
                .foregroundStyle(AppTheme.primaryText)
        }
        .accessibilityLabel("\(label): \(value.map { "\(Int($0.rounded())) százalék" } ?? "nincs adat")")
    }

    private func row(_ title: String, _ detail: String, identifier: String) -> some View {
        HStack {
            Text(title).font(.appText(12)).foregroundStyle(AppTheme.primaryText)
            Spacer()
            Text(detail).font(.appText(11)).foregroundStyle(AppTheme.secondaryText)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(title), \(detail)")
        .accessibilityIdentifier(identifier)
    }

    private static func number(_ value: Double?) -> String {
        guard let value else { return "–" }
        return String(format: "%.2f", value)
    }
}
