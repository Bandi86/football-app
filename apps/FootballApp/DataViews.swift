import SwiftUI
import FootballAppCore

/// Kattintható forráslinkek a meccsrészletezőben (S181/S2).
/// Csak a backend által eltárolt http(s) URL-ek jelennek meg; a felhasználó
/// nem adhat meg URL-t, és a kliens nem indít új hálózati kérést a linkre.
struct SourceLinkList: View {
    let links: [MatchLink]
    @Environment(\.openURL) private var openURL

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("FORRÁSOK")
                .sectionLabel()
                .accessibilityIdentifier("match.detail.links")
            ForEach(links) { link in
                if let url = URL(string: link.url),
                   ["http", "https"].contains(url.scheme?.lowercased() ?? "") {
                    Button {
                        openURL(url)
                    } label: {
                        HStack(spacing: AppTheme.Spacing.s) {
                            Text(link.label)
                                .font(.appText(12))
                                .foregroundStyle(AppTheme.primaryText)
                            Spacer(minLength: 0)
                            Image(systemName: "arrow.up.right.square")
                                .font(.system(size: 11))
                                .foregroundStyle(AppTheme.accent)
                        }
                        .padding(.vertical, 5)
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel("Forrás megnyitása: \(link.label)")
                    .accessibilityIdentifier("match.link.\(link.kind)")
                }
            }
        }
    }
}

/// Felállás: formáció, kezdők/cserék és hiányzók. A feloldott játékosok
/// kattinthatók (játékosadatlap), a feloldatlanok neve szöveg marad.
struct LineupPitch: View {
    let lineups: [LineupEntry]
    let homeName: String
    let awayName: String
    let onSelectPlayer: (UUID) -> Void

    private func side(_ side: String) -> [LineupEntry] {
        lineups.filter { $0.side == side }.sorted {
            if $0.role != $1.role { return $0.role == "starter" }
            return ($0.shirtNumber ?? 999) < ($1.shirtNumber ?? 999)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.m) {
            Text("FELÁLLÁS")
                .sectionLabel()
            HStack(alignment: .top, spacing: AppTheme.Spacing.m) {
                sideColumn("Hazai · \(homeName)", entries: side("home"), sideKey: "home")
                sideColumn("Vendég · \(awayName)", entries: side("away"), sideKey: "away")
            }
        }
    }

    private func sideColumn(_ title: String, entries: [LineupEntry], sideKey: String) -> some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text(title).font(.appLabel(10)).foregroundStyle(AppTheme.accent)
            if entries.isEmpty {
                Text("Nincs felállás").font(.appText(11)).foregroundStyle(AppTheme.secondaryText)
            }
            ForEach(entries) { entry in
                let label = "\(entry.shirtNumber.map { "\($0). " } ?? "")\(entry.playerName ?? entry.sourcePlayerId)\(entry.position.map { " · \($0)" } ?? "")"
                if let playerId = entry.playerEntityId {
                    Button {
                        onSelectPlayer(playerId)
                    } label: {
                        Text(label)
                            .font(.appText(11))
                            .foregroundStyle(AppTheme.primaryText)
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel("Játékos megnyitása: \(label)")
                    .accessibilityIdentifier("match.lineup.\(sideKey).\(entry.sourcePlayerId)")
                } else {
                    Text(label)
                        .font(.appText(11))
                        .foregroundStyle(AppTheme.primaryText)
                        .accessibilityLabel(label)
                        .accessibilityIdentifier("match.lineup.\(sideKey).\(entry.sourcePlayerId)")
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Játékosszintű statisztika: oldalszűrő és érték szerinti rendezés.
/// A hiányzó érték «–», nem 0 (spec §3.5).
struct PlayerStatTable: View {
    let stats: [PlayerStatEntry]
    @State private var sideFilter = "Mind"

    private let sides = ["Mind", "Hazai", "Vendég"]

    private var rows: [PlayerStatEntry] {
        let filtered = stats.filter { stat in
            switch sideFilter {
            case "Hazai": return stat.side == "home"
            case "Vendég": return stat.side == "away"
            default: return true
            }
        }
        return filtered.sorted {
            ($0.value ?? -1) > ($1.value ?? -1)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            HStack {
                Text("JÁTÉKOSSTATISZTIKA")
                    .sectionLabel()
                Spacer()
                Picker("Oldal", selection: $sideFilter) {
                    ForEach(sides, id: \.self) { Text($0).tag($0) }
                }
                .pickerStyle(.segmented)
                .frame(width: 220)
            }
            ForEach(Array(rows.prefix(20).enumerated()), id: \.offset) { index, stat in
                HStack {
                    Text(stat.playerName ?? stat.sourcePlayerId)
                        .font(.appText(11))
                        .foregroundStyle(AppTheme.primaryText)
                    Text(stat.statKey)
                        .font(.appLabel(9))
                        .foregroundStyle(AppTheme.secondaryText)
                    Spacer()
                    Text(stat.value.map { String(format: "%.1f", $0) } ?? "–")
                        .font(.scoreboard(12))
                        .foregroundStyle(AppTheme.primaryText)
                }
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("match.playerstat.row.\(index)")
            }
        }
    }
}

/// Lövéstérkép: a research x/y koordinátái és xG értékei. A pálya csak
/// megjelenítés, nem interaktív; a hiány üres üzenetként jelenik meg.
struct ShotMap: View {
    let shots: [ShotEntry]

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("LÖVÉSEK · \(shots.count)")
                .sectionLabel()
            if shots.isEmpty {
                Text("Nincs lövésadat").font(.appText(11)).foregroundStyle(AppTheme.secondaryText)
            } else {
                GeometryReader { geometry in
                    ZStack(alignment: .topLeading) {
                        RoundedRectangle(cornerRadius: AppTheme.Radius.panel)
                            .fill(AppTheme.surface)
                            .overlay(
                                RoundedRectangle(cornerRadius: AppTheme.Radius.panel)
                                    .strokeBorder(AppTheme.hairline)
                            )
                        ForEach(shots) { shot in
                            if let x = shot.x, let y = shot.y {
                                Circle()
                                    .fill(shot.side == "home" ? AppTheme.accent : AppTheme.liveGreen)
                                    .frame(width: circleSize(shot.xg), height: circleSize(shot.xg))
                                    .position(
                                        x: max(8, min(geometry.size.width - 8, x * geometry.size.width)),
                                        y: max(8, min(geometry.size.height - 8, y * geometry.size.height))
                                    )
                                    .accessibilityLabel("Lövés: \(shot.minute.map { "\($0). perc" } ?? "–"), xG \(shot.xg.map { String(format: "%.2f", $0) } ?? "nincs adat")")
                                    .accessibilityIdentifier("match.shot.\(shot.shotNo)")
                            }
                        }
                    }
                }
                .frame(height: 180)
            }
        }
    }

    private func circleSize(_ xg: Double?) -> CGFloat {
        guard let xg else { return 8 }
        return CGFloat(min(28, max(8, 8 + xg * 50)))
    }
}
