import SwiftUI
import FootballAppCore

struct MatchDetailView: View {
    let match: DayMatch
    @Environment(\.dismiss) private var dismiss

    private static let kickoffFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .full
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("Meccs").font(.headline)
                Spacer()
                Button("Bezárás") { dismiss() }
                    .keyboardShortcut(.cancelAction)
            }

            Text("\(match.home) – \(match.away)")
                .font(.title2.weight(.semibold))

            VStack(alignment: .leading, spacing: 8) {
                detailRow("Időpont", Self.kickoffFormatter.string(from: match.kickoffAt))
                detailRow("Állapot", MatchRowView.statusLabel(for: match.status))
                detailRow("Eredmény", scoreText)
                HStack(alignment: .firstTextBaseline) {
                    Text("Publikálás")
                        .foregroundStyle(.secondary)
                        .frame(width: 90, alignment: .leading)
                    StatusBadge(state: match.publishState)
                }
            }

            if !match.blockers.isEmpty {
                Divider()
                Text("Megjegyzések").font(.subheadline.weight(.semibold))
                ForEach(Array(match.blockers.enumerated()), id: \.offset) { _, blocker in
                    Text("• \(blocker.message)")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()
        }
        .padding(20)
        .frame(minWidth: 380, minHeight: 260, alignment: .topLeading)
    }

    private var scoreText: String {
        if let home = match.homeScore, let away = match.awayScore {
            return "\(home) – \(away)"
        }
        return "–"
    }

    private func detailRow(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label)
                .foregroundStyle(.secondary)
                .frame(width: 90, alignment: .leading)
            Text(value)
        }
    }
}
