import SwiftUI
import FootballAppCore

/// A részletes odds-piacok listája az Odds fülben. A fő 1X2/BTTS/OU mátrix
/// alatt jelenik meg; a piacok a forrástól jönnek, a magyar címke csak
/// megjelenítés. Nagy számú piacnál a lista szakaszosan nyílik meg.
struct MarketOddsList: View {
    let response: MarketsResponseDTO
    @State private var showAll = false
    @State private var search = ""

    private static let initialGroups = 10

    private var groups: [MarketGroup] {
        let all = MarketDisplay.groups(response.quotes)
        let needle = search.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !needle.isEmpty else { return all }
        return all.filter { group in
            group.title.localizedCaseInsensitiveContains(needle)
                || group.quotes.contains { $0.outcome.localizedCaseInsensitiveContains(needle) }
        }
    }

    private var visible: [MarketGroup] {
        showAll ? groups : Array(groups.prefix(Self.initialGroups))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("További piacok (\(response.count))")
                .font(.appText(14, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .accessibilityIdentifier("match.markets")

            TextField("Piac keresése…", text: $search)
                .textFieldStyle(.plain)
                .font(.system(size: 12.5))
                .padding(.horizontal, 12)
                .padding(.vertical, 7)
                .background(AppTheme.surface, in: Capsule())
                .accessibilityIdentifier("match.markets.search")

            if groups.isEmpty {
                Text(response.quotes.isEmpty
                     ? "Nincsenek részletes piacok ezen a meccsen."
                     : "Nincs piac a keresésre.")
                    .font(.appLabel())
                    .foregroundStyle(AppTheme.secondaryText)
                    .accessibilityIdentifier("match.markets.empty")
            } else {
                ForEach(visible, id: \.key) { group in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text(group.title)
                                .font(.appLabel(11))
                                .foregroundStyle(AppTheme.secondaryText)
                            Text("\(group.quotes.count)")
                                .font(.system(size: 10, weight: .semibold).monospacedDigit())
                                .foregroundStyle(AppTheme.secondaryText)
                        }
                        .accessibilityIdentifier("match.markets.group")
                        ForEach(Array(group.quotes.enumerated()), id: \.offset) { _, quote in
                            HStack(spacing: 8) {
                                Text(MarketDisplay.outcomeLabel(quote.outcome))
                                    .font(.appText(12.5))
                                    .foregroundStyle(AppTheme.primaryText)
                                if quote.line != 0 {
                                    Text(MarketDisplay.priceText(quote.line))
                                        .font(.system(size: 11).monospacedDigit())
                                        .foregroundStyle(AppTheme.secondaryText)
                                }
                                Spacer()
                                Text(MarketDisplay.priceText(quote.price))
                                    .font(.system(size: 12.5, weight: .semibold).monospacedDigit())
                                    .foregroundStyle(AppTheme.primaryText)
                                Text(quote.sourceCode)
                                    .font(.appLabel(9))
                                    .foregroundStyle(AppTheme.secondaryText)
                            }
                            .accessibilityElement(children: .combine)
                            .accessibilityLabel(
                                "\(group.title), \(MarketDisplay.outcomeLabel(quote.outcome)), "
                                + "\(MarketDisplay.priceText(quote.price)), forrás \(quote.sourceCode)"
                            )
                            .accessibilityIdentifier("match.markets.row")
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
                    .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 12))
                }

                if groups.count > Self.initialGroups {
                    Button(showAll ? "Kevesebb piac" : "Többi piac megjelenítése") {
                        showAll.toggle()
                    }
                    .buttonStyle(.borderless)
                    .font(.appLabel())
                    .accessibilityIdentifier("match.markets.more")
                }
            }
        }
    }
}
