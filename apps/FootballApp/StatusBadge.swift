import SwiftUI
import FootballAppCore

struct StatusBadge: View {
    let state: PublishState

    var body: some View {
        Text(label)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(color.opacity(0.15), in: Capsule())
            .foregroundStyle(color)
    }

    private var label: String {
        switch state {
        case .verifiedPublish: return "ellenőrzött"
        case .provisionalPublish: return "ideiglenes"
        case .withheld: return "visszatartott"
        case .quarantined: return "karantén"
        case .unknown: return "ismeretlen"
        }
    }

    private var color: Color {
        switch state {
        case .verifiedPublish: return .green
        case .provisionalPublish: return .orange
        case .withheld, .quarantined: return .red
        case .unknown: return .gray
        }
    }
}
