import SwiftUI
import FootballAppCore

/// Csapatok: kanonikus csapatok kereséssel, nem/ág jelöléssel.
struct TeamsView: View {
    @State private var model = TeamsModel()
    @State private var selected: SelectedTeam?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            content
        }
        .background(AppTheme.background)
        .task { await model.load() }
        .sheet(item: $selected) { item in
            TeamSheet(teamId: item.id, name: item.name)
        }
    }

    private var header: some View {
        PageHeader(title: "Csapatok", subtitle: "kanonikus csapatok · mérleg és meccsek") {
            HStack(spacing: AppTheme.Spacing.s) {
                SearchField(text: $model.search, placeholder: "Keresés (csapat, ország)")
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
            ProgressView().controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: { Task { await model.load() } })
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let rows):
            if rows.isEmpty {
                EmptyStateView().frame(maxWidth: .infinity, maxHeight: .infinity)
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
                        selected = SelectedTeam(id: row.teamId, name: row.name)
                    } label: {
                        teamRow(row)
                    }
                    .buttonStyle(.plain)
                    if index < model.filtered.count - 1 {
                        Rectangle().fill(AppTheme.hairline).frame(height: 1).padding(.leading, 64)
                    }
                }
            }
            .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(AppTheme.hairline))
            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            .padding(.horizontal, 48)
            .padding(.vertical, 16)
        }
    }

    private func teamRow(_ row: TeamSummary) -> some View {
        HStack(spacing: 14) {
            CrestLogo(name: row.name, path: row.logo, store: model.logos, size: 34)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.name)
                    .font(.appText(14.5))
                    .foregroundStyle(AppTheme.primaryText)
                Text(subtitle(row))
                    .font(.appLabel(9))
                    .tracking(1.1)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
            if row.branch == "youth" || row.gender == "female" {
                Text(row.gender == "female" ? "NŐI" : "UTÁNPÓTLÁS")
                    .font(.appLabel(8.5))
                    .tracking(0.8)
                    .foregroundStyle(AppTheme.accent)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(AppTheme.accent.opacity(0.13), in: Capsule())
            }
            Text("\(row.recentMatches) meccs")
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

    private func subtitle(_ row: TeamSummary) -> String {
        let display = CountryDisplay.display(code: row.countryCode, name: row.countryName)
        return display.name.uppercased()
    }
}

struct SelectedTeam: Identifiable {
    let id: UUID
    let name: String
}
