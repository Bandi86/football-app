import SwiftUI
import FootballAppCore

/// Versenyek: a mai nap ellenőrzött versenyei, kereséssel és létszámokkal.
struct CompetitionsView: View {
    @State private var model = CompetitionsModel()
    @State private var selected: SelectedCompetition?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            content
        }
        .background(AppTheme.background)
        .task { await model.load() }
        .sheet(item: $selected) { item in
            CompetitionSheet(competitionId: item.id, name: item.name, date: item.date ?? Date())
        }
    }

    private var header: some View {
        PageHeader(title: "Versenyek", subtitle: "a nap ellenőrzött versenyei") {
            HStack(spacing: AppTheme.Spacing.s) {
                SearchField(text: $model.search, placeholder: "Keresés (liga, ország)")
                RoundIconButton(systemName: "arrow.clockwise", help: "Frissítés") {
                    Task { await model.load() }
                }
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        switch model.state {
        case .idle, .loading:
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: { Task { await model.load() } })
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let rows):
            if rows.isEmpty {
                EmptyStateView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                list
            }
        }
    }

    private var list: some View {
        ScrollView {
            VStack(spacing: 0) {
                ForEach(Array(model.filtered.enumerated()), id: \.element.id) { index, row in
                    Button {
                        selected = SelectedCompetition(id: row.competitionId, name: row.name)
                    } label: {
                        competitionRow(row)
                    }
                    .buttonStyle(.plain)
                    if index < model.filtered.count - 1 {
                        Rectangle().fill(AppTheme.hairline).frame(height: 1).padding(.leading, 64)
                    }
                }
            }
            .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .strokeBorder(AppTheme.hairline)
            )
            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            .padding(.horizontal, 48)
            .padding(.vertical, 16)
        }
    }

    private func competitionRow(_ row: CompetitionSummary) -> some View {
        HStack(spacing: 14) {
            CompetitionLogo(path: row.logo, store: model.logos, fallbackFlag: row.countryCode)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.name)
                    .font(.appText(14.5))
                    .foregroundStyle(AppTheme.primaryText)
                Text(countryLabel(row))
                    .font(.appLabel(9))
                    .tracking(1.1)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
            if row.live > 0 {
                Text("ÉLŐ \(row.live)")
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.liveGreen)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(AppTheme.liveGreen.opacity(0.14), in: Capsule())
            }
            Text("\(row.matches) meccs")
                .font(.scoreboard(11, weight: .medium))
                .foregroundStyle(AppTheme.secondaryText)
            Image(systemName: "chevron.right")
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText.opacity(0.6))
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 11)
        .contentShape(Rectangle())
    }

    private func countryLabel(_ row: CompetitionSummary) -> String {
        if row.countryCode == nil && row.countryName == nil {
            return "NEMZETKÖZI"
        }
        let display = CountryDisplay.display(code: row.countryCode, name: row.countryName)
        return display.name.uppercased()
    }
}

struct SelectedCompetition: Identifiable {
    let id: UUID
    let name: String
    var date: Date? = nil
}

struct CompetitionLogo: View {
    let path: String?
    let store: LogoStore
    let fallbackFlag: String?
    var size: CGFloat = 32
    @State private var image: NSImage?

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: .fit)
                    .padding(3)
            } else {
                Text(CountryDisplay.display(code: fallbackFlag, name: nil).flag)
                    .font(.system(size: size * 0.56))
            }
        }
        .frame(width: size, height: size)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .task(id: path) {
            guard let path else { return }
            image = await store.image(for: path)
        }
    }
}
