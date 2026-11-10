import SwiftUI

struct EmptyStateView: View {
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "calendar")
                .font(.largeTitle)
                .foregroundStyle(.secondary)
            Text("Nincs meccs ezen a napon.")
                .foregroundStyle(.secondary)
        }
        .padding()
    }
}
