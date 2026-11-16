import SwiftUI
import FootballAppCore

/// Csapat-adatlap: fejléc (logó, ország, nem/ág, Elo), megfigyelt mérleg,
/// majd a meccsek verseny szerint csoportosítva.
struct TeamSheet: View {
    let teamId: UUID
    let name: String
    @State private var model = TeamDetailModel()
    @State private var selectedMatch: SelectedMatch?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(name.uppercased())
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(2)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Button("Bezárás") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                    .buttonStyle(.plain)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 14)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            content
        }
        .frame(minWidth: 760, minHeight: 600)
        .background(AppTheme.background)
        .task(id: teamId) { await model.load(id: teamId) }
        .sheet(item: $selectedMatch) { item in
            MatchDetailSheet(matchId: item.id)
        }
    }

    @ViewBuilder
    private var content: some View {
        switch model.state {
        case .idle, .loading:
            ProgressView().controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: {
                Task { await model.load(id: teamId) }
            })
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let detail):
            loaded(detail)
        }
    }

    private func loaded(_ detail: TeamDetail) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            header(detail)
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            if detail.matches.isEmpty {
                VStack(spacing: 10) {
                    Image(systemName: "calendar.badge.exclamationmark")
                        .font(.largeTitle)
                        .foregroundStyle(AppTheme.secondaryText)
                    Text("Nincs ellenőrzött meccs a megfigyelt ablakban.")
                        .foregroundStyle(AppTheme.secondaryText)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                MatchListView(
                    leagues: LeagueGroup.fromDay(detail.matches),
                    logos: model.logos
                ) { id in
                    if let uuid = UUID(uuidString: id) {
                        selectedMatch = SelectedMatch(id: uuid)
                    }
                }
            }
        }
    }

    private func header(_ detail: TeamDetail) -> some View {
        HStack(alignment: .center, spacing: 16) {
            TeamMark(path: detail.detail.logo, store: model.logos)
                .scaleEffect(2.2)
                .frame(width: 64, height: 64)
                .padding(.leading, 6)
            VStack(alignment: .leading, spacing: 5) {
                Text(detail.detail.name)
                    .font(.system(size: 26, weight: .bold))
                    .foregroundStyle(AppTheme.primaryText)
                HStack(spacing: 8) {
                    Text(country(detail)).font(.system(size: 12))
                        .foregroundStyle(AppTheme.secondaryText)
                    if detail.detail.gender == "female" {
                        badge("NŐI")
                    } else if detail.detail.branch == "youth" {
                        badge("UTÁNPÓLTLÁS")
                    } else if detail.detail.branch == "reserve" {
                        badge("TARTALÉK")
                    }
                    if let elo = detail.detail.elo {
                        badge("ELO \(Int(elo.rounded()))")
                    }
                }
            }
            Spacer()
            HStack(spacing: 8) {
                formChip("M", "\(detail.detail.played)")
                formChip("GY", "\(detail.detail.wins)")
                formChip("D", "\(detail.detail.draws)")
                formChip("V", "\(detail.detail.losses)")
                formChip("GÓL", "\(detail.detail.goalsFor)–\(detail.detail.goalsAgainst)")
            }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
    }

    private func country(_ detail: TeamDetail) -> String {
        CountryDisplay.display(code: detail.detail.countryCode,
                               name: detail.detail.countryName).name
    }

    private func badge(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 8.5, weight: .bold))
            .tracking(0.8)
            .foregroundStyle(AppTheme.accent)
            .padding(.horizontal, 7)
            .padding(.vertical, 2)
            .background(AppTheme.accent.opacity(0.13), in: Capsule())
    }

    private func formChip(_ label: String, _ value: String) -> some View {
        VStack(spacing: 1) {
            Text(value)
                .font(.system(size: 13.5, weight: .semibold).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText)
            Text(label)
                .font(.system(size: 8, weight: .semibold))
                .tracking(0.7)
                .foregroundStyle(AppTheme.secondaryText)
        }
        .frame(width: 52)
        .padding(.vertical, 6)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
    }
}
