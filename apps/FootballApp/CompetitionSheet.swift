import SwiftUI
import FootballAppCore

/// Verseny-adatlap: fejléc + összesítők, majd meccsek és csapat-mérlegek.
struct CompetitionSheet: View {
    let competitionId: UUID
    let name: String
    var date: Date = Date()
    @State private var model = CompetitionDetailModel()
    @State private var tab: CompetitionTab = .matches
    @State private var selectedMatch: SelectedMatch?
    @Environment(\.dismiss) private var dismiss

    enum CompetitionTab: String, CaseIterable, Identifiable {
        case matches = "Meccsek"
        case teams = "Csapatok"
        case seasons = "Szezonok"
        var id: String { rawValue }
    }

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
        .frame(minWidth: 760, minHeight: 620)
        .background(AppTheme.background)
        .task(id: competitionId) { await model.load(id: competitionId, date: date) }
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
                Task { await model.load(id: competitionId, date: date) }
            })
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let detail):
            loaded(detail)
        }
    }

    private func loaded(_ detail: CompetitionDetail) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            header(detail)
            tabBar(detail)
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            switch tab {
            case .matches:
                MatchListView(
                    leagues: LeagueGroup.fromDay(detail.matches),
                    logos: model.logos
                ) { id in
                    if let uuid = UUID(uuidString: id) {
                        selectedMatch = SelectedMatch(id: uuid)
                    }
                }
            case .teams:
                teamsList(detail.teams)
            case .seasons:
                seasonsList(model.seasons)
            }
        }
    }

    private func header(_ detail: CompetitionDetail) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 14) {
                CompetitionLogo(path: detail.detail.logo, store: model.logos,
                                fallbackFlag: detail.detail.countryCode)
                    .scaleEffect(1.5)
                    .frame(width: 48, height: 48)
                VStack(alignment: .leading, spacing: 3) {
                    Text(detail.detail.name)
                        .font(.system(size: 24, weight: .bold))
                        .foregroundStyle(AppTheme.primaryText)
                    Text(subtitle(detail))
                        .font(.system(size: 10, weight: .semibold))
                        .tracking(1.2)
                        .foregroundStyle(AppTheme.secondaryText)
                }
                Spacer()
            }
            HStack(spacing: 10) {
                aggregate("Meccs", "\(detail.detail.matches)")
                aggregate("Gól", "\(detail.detail.goals)")
                aggregate("BTTS", bttsText(detail))
                aggregate("Hazai", "\(detail.detail.homeWins)")
                aggregate("Döntetlen", "\(detail.detail.draws)")
                aggregate("Vendég", "\(detail.detail.awayWins)")
            }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
    }

    private func subtitle(_ detail: CompetitionDetail) -> String {
        var parts: [String] = []
        let display = CountryDisplay.display(code: detail.detail.countryCode,
                                             name: detail.detail.countryName)
        if display.name != "Ismeretlen" { parts.append(display.name.uppercased()) }
        if let season = detail.detail.seasonLabel { parts.append(season) }
        parts.append("±14 NAP")
        return parts.joined(separator: " · ")
    }

    private func bttsText(_ detail: CompetitionDetail) -> String {
        guard detail.detail.matches > 0 else { return "–" }
        let pct = Double(detail.detail.bttsMatches) / Double(detail.detail.matches) * 100
        return String(format: "%.0f%%", pct)
    }

    private func aggregate(_ label: String, _ value: String) -> some View {
        VStack(spacing: 3) {
            Text(value)
                .font(.system(size: 17, weight: .semibold).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText)
            Text(label.uppercased())
                .font(.system(size: 8.5, weight: .semibold))
                .tracking(0.9)
                .foregroundStyle(AppTheme.secondaryText)
        }
        .frame(minWidth: 86)
        .padding(.vertical, 8)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func tabBar(_ detail: CompetitionDetail) -> some View {
        HStack(spacing: 6) {
            ForEach(CompetitionTab.allCases) { candidate in
                let active = tab == candidate
                Button {
                    tab = candidate
                } label: {
                    HStack(spacing: 5) {
                        Text(candidate.rawValue)
                            .font(.system(size: 12.5, weight: .medium))
                        Text("\(count(candidate, detail))")
                            .font(.system(size: 10).monospacedDigit())
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    .foregroundStyle(active ? AppTheme.primaryText : AppTheme.secondaryText.opacity(0.6))
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(active ? AppTheme.surface : Color.clear, in: Capsule())
                }
                .buttonStyle(.plain)
            }
            Spacer()
        }
        .padding(.horizontal, 24)
        .padding(.bottom, 10)
    }

    private func count(_ tab: CompetitionTab, _ detail: CompetitionDetail) -> Int {
        switch tab {
        case .matches: return detail.matches.count
        case .teams: return detail.teams.count
        case .seasons: return model.seasons.count
        }
    }

    private func seasonsList(_ seasons: [CompetitionSeason]) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if seasons.isEmpty {
                    Text("Nincs szezonadat.")
                        .font(.system(size: 12))
                        .foregroundStyle(AppTheme.secondaryText)
                        .padding(24)
                } else {
                    ForEach(seasons) { season in
                        HStack {
                            Text(season.label)
                                .font(.system(size: 13, weight: .semibold))
                                .frame(width: 90, alignment: .leading)
                            Text("\(season.matches) meccs")
                                .font(.system(size: 12))
                                .foregroundStyle(AppTheme.secondaryText)
                            Spacer()
                            Text("eredmény \(season.withResult)")
                                .font(.system(size: 11))
                                .foregroundStyle(AppTheme.secondaryText)
                            Text("odds \(season.withOdds)")
                                .font(.system(size: 11))
                                .foregroundStyle(AppTheme.secondaryText)
                        }
                        .padding(.horizontal, 24)
                        .padding(.vertical, 10)
                        Rectangle().fill(AppTheme.hairline).frame(height: 1)
                    }
                }
            }
        }
    }

    private func teamsList(_ teams: [CompetitionTeam]) -> some View {
        Group {
            if teams.isEmpty {
                VStack(spacing: 10) {
                    Image(systemName: "person.3")
                        .font(.largeTitle)
                        .foregroundStyle(AppTheme.secondaryText)
                    Text("Nincs eredmény a megfigyelt ablakban — a mérleg nem számolható.")
                        .foregroundStyle(AppTheme.secondaryText)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    VStack(spacing: 0) {
                        ForEach(Array(teams.enumerated()), id: \.element.id) { index, team in
                            teamRow(team)
                            if index < teams.count - 1 {
                                Rectangle().fill(AppTheme.hairline)
                                    .frame(height: 1).padding(.leading, 52)
                            }
                        }
                    }
                    .background(AppTheme.surfaceElevated,
                                in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .strokeBorder(AppTheme.hairline))
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .padding(24)
                }
            }
        }
    }

    private func teamRow(_ team: CompetitionTeam) -> some View {
        HStack(spacing: 12) {
            TeamMark(path: team.logo, store: model.logos)
            Text(team.name)
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(AppTheme.primaryText)
                .frame(maxWidth: .infinity, alignment: .leading)
            statColumn("M", "\(team.played)")
            statColumn("GY", "\(team.wins)")
            statColumn("D", "\(team.draws)")
            statColumn("V", "\(team.losses)")
            statColumn("GÓL", "\(team.goalsFor)–\(team.goalsAgainst)")
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
    }

    private func statColumn(_ label: String, _ value: String) -> some View {
        VStack(spacing: 1) {
            Text(value)
                .font(.system(size: 12.5, weight: .medium).monospacedDigit())
                .foregroundStyle(AppTheme.primaryText)
            Text(label)
                .font(.system(size: 8, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(AppTheme.secondaryText)
        }
        .frame(width: 42, alignment: .trailing)
    }
}

struct TeamMark: View {
    let path: String?
    let store: LogoStore
    @State private var image: NSImage?

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: .fit)
                    .padding(1.5)
            } else {
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(AppTheme.surface)
            }
        }
        .frame(width: 24, height: 24)
        .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous)
            .strokeBorder(AppTheme.hairline))
        .task(id: path) {
            guard let path else { return }
            image = await store.image(for: path)
        }
    }
}
