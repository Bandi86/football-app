import SwiftUI
import FootballAppCore

struct MatchRowView: View {
    let match: DayMatch

    private static let kickoffFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    // Státusz-szövegek egyedül a FootballAppCore AccessibilityLabels-ben élnek.

    /// Ismeretlen állapot magyar címkéje, ismeretlen esetén a nyers érték.
    /// Az app-target tesztjei ezen keresztül ellenőrzik a közös fordítást.
    static func statusLabel(for status: String) -> String {
        AccessibilityLabels.statusLabel(for: status)
    }

    var body: some View {
        HStack(spacing: 12) {
            Text(Self.kickoffFormatter.string(from: match.kickoffAt))
                .font(.callout.monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(width: 64, alignment: .leading)

            VStack(alignment: .leading, spacing: 2) {
                Text("\(match.home) – \(match.away)")
                Text(Self.statusLabel(for: match.status))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            if let home = match.homeScore, let away = match.awayScore {
                Text("\(home) – \(away)")
                    .font(.headline.monospacedDigit())
            }

            StatusBadge(state: match.publishState)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            AccessibilityLabels.matchRowLabel(
                home: match.home,
                away: match.away,
                kickoffAt: match.kickoffAt,
                status: match.status,
                homeScore: match.homeScore,
                awayScore: match.awayScore,
                publishState: match.publishState
            )
        )
        .accessibilityIdentifier(AccessibilityLabels.matchIdentifier(for: match.id))
    }
}
