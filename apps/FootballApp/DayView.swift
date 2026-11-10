import SwiftUI
import FootballAppCore

struct DayView: View {
    @Bindable var model: DayModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                DatePicker("Nap", selection: $model.selectedDate, displayedComponents: .date)
                    .datePickerStyle(.compact)
                Spacer()
                Button {
                    Task { await model.load() }
                } label: {
                    Label("Frissítés", systemImage: "arrow.clockwise")
                }
            }
            .padding([.horizontal, .top])

            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .task { await model.load() }
        .onChange(of: model.selectedDate) { Task { await model.load() } }
    }

    @ViewBuilder
    private var content: some View {
        switch model.state {
        case .idle, .loading:
            ProgressView("Betöltés…")
        case .empty:
            EmptyStateView()
        case .error(let error):
            ErrorStateView(message: error.userMessage) {
                Task { await model.load() }
            }
        case .loaded(let matches):
            List(matches) { match in
                MatchRowView(match: match)
            }
        }
    }
}
