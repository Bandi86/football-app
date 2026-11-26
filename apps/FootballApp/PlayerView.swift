import SwiftUI
import FootballAppCore

/// Játékosadatlap a globális keresésből. Minden szakasz külön, szövegesen
/// jelölt; a hiány „Nincs adat", nem üres sor és nem kitalált 0.
struct PlayerView: View {
    @State private var model: PlayerModel
    @State private var selectedMatch: SelectedMatch?
    private let playerID: UUID
    @Environment(\.dismiss) private var dismiss

    init(id: UUID) {
        self.playerID = id
        _model = State(initialValue: PlayerModel(id: id))
    }

    private static func ratingText(_ value: Double?) -> String {
        guard let value else { return "" }
        return String(format: " · %.1f", value)
    }

    private static func valueText(_ value: Double?) -> String {
        guard let value else { return "Nincs adat" }
        return String(format: "%.2f", value)
    }

    private static let dateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "hu_HU")
        f.dateFormat = "yyyy. MM. dd. HH:mm"
        return f
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Játékosadatlap")
                    .font(.appLabel(10))
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Button("Bezárás") { dismiss() }
                    .keyboardShortcut(.cancelAction)
            }

            switch model.status {
            case .idle, .loading:
                Text("Betöltés…")
                    .font(.appText(13))
                    .foregroundStyle(AppTheme.secondaryText)
                    .accessibilityIdentifier("player.loading")
            case .notFound:
                Text("Nincs ilyen játékos.")
                    .foregroundStyle(AppTheme.warning)
                    .accessibilityIdentifier("player.notfound")
            case .failed:
                Text("A játékos adatai nem elérhetők. Próbáld újra később.")
                    .foregroundStyle(AppTheme.warning)
                    .accessibilityIdentifier("player.error")
            case .loaded:
                if let feed = model.feed {
                    content(feed)
                }
            }
        }
        .padding(18)
        .frame(width: 680, height: 560)
        .background(AppTheme.background)
        .task { await model.load() }
        .sheet(item: $selectedMatch) { item in
            MatchDetailSheet(matchId: item.id)
        }
    }

    @ViewBuilder
    private func content(_ feed: PlayerFeed) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(feed.player.name)
                        .font(.system(size: 22, weight: .semibold))
                        .foregroundStyle(AppTheme.primaryText)
                        .accessibilityIdentifier("player.name")
                    Text("Ország: \(feed.player.countryCode ?? "Nincs adat")")
                        .font(.appText(12))
                        .foregroundStyle(AppTheme.secondaryText)
                        .accessibilityIdentifier("player.country")
                }

                section("Csapattagság") {
                    if feed.spells.isEmpty {
                        missing("Nincs csapattagság-adat")
                    } else {
                        ForEach(Array(feed.spells.enumerated()), id: \.offset) { _, spell in
                            row(
                                spell.teamName,
                                "\(spell.validFrom) – \(spell.validTo ?? "jelenlegi")\(spell.loan ? " · kölcsön" : "")",
                                identifier: "player.spell"
                            )
                        }
                    }
                }

                section("Fellépések") {
                    if feed.appearances.isEmpty {
                        missing("Nincs fellépés-adat")
                    } else {
                        ForEach(feed.appearances, id: \.matchId) { appearance in
                            matchRow(
                                "\(Self.dateFormatter.string(from: appearance.kickoffAt)) · \(appearance.competitionName ?? "Ismeretlen verseny")",
                                "\(appearance.side ?? "?") · \(appearance.role)\(Self.ratingText(appearance.rating))",
                                matchId: appearance.matchId,
                                identifier: "player.appearance"
                            )
                        }
                    }
                }

                section("Meccs-statisztika") {
                    if feed.stats.isEmpty {
                        missing("Nincs játékosstatisztika")
                    } else {
                        ForEach(Array(feed.stats.enumerated()), id: \.offset) { _, stat in
                            matchRow(
                                stat.statKey,
                                Self.valueText(stat.value),
                                matchId: stat.matchId,
                                identifier: "player.stat"
                            )
                        }
                    }
                }

                section("Események") {
                    if feed.incidents.isEmpty {
                        missing("Nincs esemény-adat")
                    } else {
                        ForEach(Array(feed.incidents.enumerated()), id: \.offset) { _, incident in
                            matchRow(
                                "\(incident.minute.map { "\($0). perc" } ?? "Ismeretlen perc") · \(incident.incidentType)",
                                [incident.playerName, incident.relatedPlayerName]
                                    .compactMap { $0 }
                                    .joined(separator: " ↔ "),
                                matchId: incident.matchId,
                                identifier: "player.incident"
                            )
                        }
                    }
                }
            }
        }
        .accessibilityIdentifier("player.content")
    }

    private func section(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.appLabel(9))
                .foregroundStyle(AppTheme.secondaryText)
            content()
        }
    }

    private func row(_ title: String, _ detail: String, identifier: String) -> some View {
        HStack {
            Text(title)
                .font(.appText(12))
                .foregroundStyle(AppTheme.primaryText)
            Spacer()
            Text(detail)
                .font(.appText(11))
                .foregroundStyle(AppTheme.secondaryText)
        }
        .padding(.vertical, 3)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(title), \(detail)")
        .accessibilityIdentifier(identifier)
    }

    /// A játékosoldal minden meccshez tartozó sora kattintható: a konkrét
    /// meccsrészletezőt nyitja meg (a `matchId` a DTO-ból jön).
    private func matchRow(_ title: String, _ detail: String, matchId: UUID,
                          identifier: String) -> some View {
        Button {
            selectedMatch = SelectedMatch(id: matchId)
        } label: {
            HStack {
                Text(title)
                    .font(.appText(12))
                    .foregroundStyle(AppTheme.primaryText)
                Spacer()
                Text(detail)
                    .font(.appText(11))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.vertical, 3)
        }
        .buttonStyle(.borderless)
        .accessibilityLabel("Meccs megnyitása: \(title), \(detail)")
        .accessibilityIdentifier(identifier)
    }

    private func missing(_ text: String) -> some View {
        Text(text)
            .font(.appText(11))
            .foregroundStyle(AppTheme.secondaryText)
            .accessibilityIdentifier("player.missing")
    }
}
