import SwiftUI
import FootballAppCore

struct DayView: View {
    @Bindable var model: DayModel
    @State private var selectedMatch: DayMatch?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Button {
                    model.goToPreviousDay()
                } label: {
                    Image(systemName: "chevron.left")
                }
                .help("Előző nap")

                DatePicker("Nap", selection: $model.selectedDate, displayedComponents: .date)
                    .datePickerStyle(.compact)

                Button {
                    model.goToNextDay()
                } label: {
                    Image(systemName: "chevron.right")
                }
                .help("Következő nap")

                Button("Ma") {
                    model.goToToday()
                }
                .help("Ugrás a mai napra")

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
        .onChange(of: model.selectedDate) { _, _ in Task { await model.load() } }
        .sheet(item: $selectedMatch) { match in
            MatchDetailView(match: match)
        }
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
                    .contentShape(Rectangle())
                    .onTapGesture { selectedMatch = match }
            }
        }
    }
}
