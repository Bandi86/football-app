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
        HStack(alignment: .bottom, spacing: 20) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Versenyek")
                    .font(.system(size: 44, weight: .bold))
                    .tracking(-1.5)
                    .foregroundStyle(AppTheme.primaryText)
                Text("a nap ellenőrzött versenyei")
                    .font(.system(size: 12))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
            searchField
            Button {
                Task { await model.load() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                    .padding(9)
                    .background(AppTheme.surface, in: Circle())
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 48)
        .padding(.top, 28)
        .padding(.bottom, 18)
    }

    private var searchField: some View {
        HStack(spacing: 7) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
            TextField("Keresés (liga, ország)", text: $model.search)
                .textFieldStyle(.plain)
                .font(.system(size: 12.5))
                .foregroundStyle(AppTheme.primaryText)
                .frame(width: 210)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(AppTheme.surface, in: Capsule())
        .overlay(Capsule().strokeBorder(AppTheme.hairline))
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
                    .font(.system(size: 14.5, weight: .medium))
                    .foregroundStyle(AppTheme.primaryText)
                Text(countryLabel(row))
                    .font(.system(size: 9.5, weight: .semibold))
                    .tracking(1.1)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
            if row.live > 0 {
                Text("ÉLŐ \(row.live)")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(AppTheme.liveGreen)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(AppTheme.liveGreen.opacity(0.14), in: Capsule())
            }
            Text("\(row.matches) meccs")
                .font(.system(size: 11.5).monospacedDigit())
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
                    .font(.system(size: 18))
            }
        }
        .frame(width: 32, height: 32)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .task(id: path) {
            guard let path else { return }
            image = await store.image(for: path)
        }
    }
}
