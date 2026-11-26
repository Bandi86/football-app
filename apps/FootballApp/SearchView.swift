import SwiftUI
import FootballAppCore

/// A `sheet(item:)` hordozójához kell az Identifiable burkoló (UUID önmagában nem az).
private struct SelectedPlayerSheet: Identifiable {
    let id: UUID
}

/// Cmd+F-el megnyitott globális keresés. A találat fajtája minden sorban
/// szövegesen is megjelenik (a szín nem egyedüli hordozó).
struct SearchView: View {
    @Bindable var model: SearchModel
    let onSelect: (SearchResult) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    /// A játékosadatlap a keresési sheetből nyílik (nested sheet) — így nem
    /// két, ugyanahhoz a nézethez kötött sheet verseng a prezentációért.
    @State private var selectedPlayer: SelectedPlayerSheet?
    @State private var selectedEntity: EntityTarget?

    var body: some View {
        VStack(spacing: 12) {
            HStack(spacing: 8) {
                TextField("Csapat, verseny, meccs…", text: $text)
                    .textFieldStyle(.roundedBorder)
                    .font(.appText(13))
                    .accessibilityLabel("Globális keresés")
                    .accessibilityIdentifier("search.field")
                Button("Keresés") {
                    Task { await model.search(text) }
                }
                .keyboardShortcut(.defaultAction)
                Button("Bezárás") { dismiss() }
                    .keyboardShortcut(.cancelAction)
            }

            if model.status == .failed {
                Text("A keresés nem elérhető. Próbáld újra később.")
                    .font(.appText(12))
                    .foregroundStyle(AppTheme.warning)
                    .accessibilityIdentifier("search.error")
            }

            List {
                if let feed = model.feed {
                    Section("\(feed.results.count) találat — \(feed.query)") {
                        ForEach(feed.results) { result in
                            Button {
                                switch result.kind {
                                case "player":
                                    selectedPlayer = SelectedPlayerSheet(id: result.id)
                                case "team", "competition":
                                    // Ugyanaz a nested-sheet minta, mint a játékosnál:
                                    // a szülő dismiss-e néha elveszítheti az új prezentációt.
                                    selectedEntity = EntityTarget(
                                        id: result.id, name: result.title, kind: result.kind
                                    )
                                default:
                                    onSelect(result)
                                    dismiss()
                                }
                            } label: {
                                HStack(spacing: 10) {
                                    Text(result.kindLabel)
                                        .font(.appLabel(9))
                                        .foregroundStyle(AppTheme.secondaryText)
                                        .frame(width: 64, alignment: .leading)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(result.title)
                                            .font(.appText(13))
                                            .foregroundStyle(AppTheme.primaryText)
                                        if let subtitle = result.subtitle, !subtitle.isEmpty {
                                            Text(subtitle)
                                                .font(.appText(11))
                                                .foregroundStyle(AppTheme.secondaryText)
                                        }
                                    }
                                }
                            }
                            .buttonStyle(.borderless)
                            .accessibilityLabel(
                                "\(result.kindLabel): \(result.title)\(result.subtitle.map { ", \($0)" } ?? "")"
                            )
                            .accessibilityIdentifier("search.result.\(result.kind)")
                        }
                    }
                }
            }
            .listStyle(.plain)
            .accessibilityIdentifier("search.results")
        }
        .sheet(item: $selectedPlayer) { selected in
            PlayerView(id: selected.id)
        }
        .sheet(item: $selectedEntity) { entity in
            // A név a keresési találatból jön; a részletadatot a sheet tölti be.
            if entity.kind == "competition" {
                CompetitionSheet(competitionId: entity.id, name: entity.name)
            } else {
                TeamSheet(teamId: entity.id, name: entity.name)
            }
        }
        .padding(16)
        .frame(width: 640, height: 480)
        .background(AppTheme.background)
    }
}
