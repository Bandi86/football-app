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

    private static let statusLabels: [String: String] = [
        "scheduled": "Ütemezett",
        "confirmed": "Megerősített",
        "live": "Élő",
        "halftime": "Félidő",
        "finished": "Lezárt",
        "postponed": "Elhalasztott",
        "canceled": "Törölt",
        "abandoned": "Félbeszakadt",
        "awarded": "Megítélt",
    ]

    /// Ismert állapot magyar címkéje, ismeretlen esetén a nyers érték.
    static func statusLabel(for status: String) -> String {
        statusLabels[status] ?? status
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
    }
}
