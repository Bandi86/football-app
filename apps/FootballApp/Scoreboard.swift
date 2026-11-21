import SwiftUI
import FootballAppCore

/// Eredménytábla-szerű, nagy számokkal dolgozó komponensek a nap- és
/// meccsképerhez. Mindegyik adat nélkül is rendezetten viselkedik: a hiányzó
/// érték helyén „–" áll, a szakasz pedig elhagyható.

/// A nap fejléce: dátum, élő/összes számláló, frissítési állapot.
struct DayMasthead: View {
    let date: Date
    let matchCount: Int
    let liveCount: Int
    let oddsCount: Int
    let refreshedAt: Date?
    let loading: Bool

    private static let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "EEEE"
        return formatter
    }()

    private static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "MMMM d."
        return formatter
    }()

    private static let clock: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()

    var body: some View {
        HStack(alignment: .lastTextBaseline, spacing: AppTheme.Spacing.xl) {
            VStack(alignment: .leading, spacing: 2) {
                Text(Self.dayFormatter.string(from: date).uppercased())
                    .sectionLabel()
                Text(Self.dateFormatter.string(from: date))
                    .font(.scoreboard(30))
                    .foregroundStyle(AppTheme.primaryText)
            }

            Spacer(minLength: 24)

            HStack(spacing: AppTheme.Spacing.l) {
                counter(value: "\(matchCount)", label: "meccs")
                if liveCount > 0 {
                    counter(value: "\(liveCount)", label: "élő", tint: AppTheme.liveGreen)
                }
                counter(value: "\(oddsCount)", label: "oddos", tint: oddsCount > 0 ? AppTheme.accent : AppTheme.secondaryText)
            }

            VStack(alignment: .trailing, spacing: 2) {
                if loading {
                    HStack(spacing: 6) {
                        ProgressView().controlSize(.mini)
                        Text("frissítés…")
                    }
                    .font(.appLabel(9.5))
                    .foregroundStyle(AppTheme.secondaryText)
                } else if let refreshedAt {
                    Text("frissítve \(Self.clock.string(from: refreshedAt))")
                        .font(.appLabel(9.5).monospacedDigit())
                        .foregroundStyle(AppTheme.secondaryText)
                }
                Text("frissül a nap folyamán")
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.tertiaryText)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(Self.dateFormatter.string(from: date)), \(matchCount) meccs, \(liveCount) élő")
    }

    private func counter(value: String, label: String, tint: Color = AppTheme.primaryText) -> some View {
        VStack(alignment: .trailing, spacing: 0) {
            Text(value)
                .font(.scoreboard(22))
                .foregroundStyle(tint)
            Text(label)
                .sectionLabel()
        }
        .accessibilityHidden(true)
    }
}

/// Nagy eredmény-blokk (meccs részletező fejléc).
struct ScoreboardHero: View {
    let home: String
    let away: String
    let homeLogo: String?
    let awayLogo: String?
    let homeElo: Double?
    let awayElo: Double?
    var homePosition: Int?
    var awayPosition: Int?
    let score: String
    let note: String?
    let statusLabel: String
    let isLive: Bool
    let gateState: PublishState?
    let logos: LogoStore

    var body: some View {
        VStack(spacing: AppTheme.Spacing.l) {
            HStack(spacing: AppTheme.Spacing.s) {
                if isLive { LiveDot() }
                Text(statusLabel.uppercased())
                    .sectionLabel()
                    .foregroundStyle(isLive ? AppTheme.liveGreen : AppTheme.secondaryText)
                Spacer()
                if let gateState { StatusBadge(state: gateState) }
            }

            HStack(alignment: .center, spacing: AppTheme.Spacing.l) {
                team(name: home, logo: homeLogo, elo: homeElo, position: homePosition, alignment: .trailing)
                VStack(spacing: 4) {
                    Text(score)
                        .font(.scoreboard(44))
                        .foregroundStyle(AppTheme.primaryText)
                        .minimumScaleFactor(0.6)
                        .lineLimit(1)
                    if let note {
                        Text(note.uppercased())
                            .sectionLabel()
                    }
                }
                .frame(minWidth: 140)
                team(name: away, logo: awayLogo, elo: awayElo, position: awayPosition, alignment: .leading)
            }
        }
        .padding(AppTheme.Spacing.l)
        .frame(maxWidth: .infinity)
        .background(
            LinearGradient(
                colors: [AppTheme.surfaceElevated, AppTheme.surfaceSunken],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            ),
            in: RoundedRectangle(cornerRadius: AppTheme.Radius.card, style: .continuous)
        )
        .overlay(
            RoundedRectangle(cornerRadius: AppTheme.Radius.card, style: .continuous)
                .strokeBorder(isLive ? AppTheme.liveGreen.opacity(0.5) : AppTheme.hairline)
        )
    }

    private func team(
        name: String,
        logo: String?,
        elo: Double?,
        position: Int?,
        alignment: HorizontalAlignment
    ) -> some View {
        VStack(alignment: alignment, spacing: AppTheme.Spacing.s) {
            CrestLogo(name: name, path: logo, store: logos, size: 64)
            Text(name)
                .font(.appText(17, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .multilineTextAlignment(alignment == .trailing ? .trailing : .leading)
                .lineLimit(2)
            HStack(spacing: 6) {
                if let position {
                    Text("\(position).")
                        .font(.appLabel(9).monospacedDigit())
                        .foregroundStyle(.black)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 2)
                        .background(AppTheme.accent, in: Capsule())
                        .accessibilityLabel("\(position). helyezés a tabellán")
                }
                if let elo {
                    Text("ELO \(Int(elo.rounded()))")
                        .font(.appLabel(9).monospacedDigit())
                        .foregroundStyle(AppTheme.secondaryText)
                        .padding(.horizontal, 7)
                        .padding(.vertical, 2)
                        .background(AppTheme.surface, in: Capsule())
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: alignment == .trailing ? .trailing : .leading)
    }
}

/// Piaci odds-mátrix: sorban a piacok, oszlopban az kimenetelek.
struct OddsMatrix: View {
    let home: Double?
    let draw: Double?
    let away: Double?
    let bttsYes: Double?
    let bttsNo: Double?
    let over25: Double?
    let under25: Double?
    var source: String?
    var capturedAt: Date?

    private static let stamp: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "MM.dd. HH:mm"
        return formatter
    }()

    private var rows: [(String, [(String, Double?)])] {
        var result: [(String, [(String, Double?)])] = [
            ("1X2", [("1", home), ("X", draw), ("2", away)]),
        ]
        if bttsYes != nil || bttsNo != nil {
            result.append(("Mindkét csapat gól", [("IGEN", bttsYes), ("NEM", bttsNo)]))
        }
        if over25 != nil || under25 != nil {
            result.append(("Gólok 2,5", [("TÖBB", over25), ("KEVESEBB", under25)]))
        }
        return result
    }

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("Odds").sectionLabel()
            VStack(spacing: 0) {
                ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                    HStack(spacing: AppTheme.Spacing.m) {
                        Text(row.0)
                            .font(.appText(11.5))
                            .foregroundStyle(AppTheme.secondaryText)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        ForEach(Array(row.1.enumerated()), id: \.offset) { _, outcome in
                            VStack(spacing: 1) {
                                Text(outcome.1.map { String(format: "%.2f", $0) } ?? "–")
                                    .font(.scoreboard(15, weight: .semibold))
                                    .foregroundStyle(outcome.1 == nil ? AppTheme.tertiaryText : AppTheme.primaryText)
                                Text(outcome.0)
                                    .font(.appLabel(8))
                                    .tracking(0.5)
                                    .foregroundStyle(AppTheme.secondaryText)
                            }
                            .frame(width: 92)
                            .padding(.vertical, 7)
                            .background(AppTheme.surfaceSunken, in: RoundedRectangle(cornerRadius: AppTheme.Radius.chip, style: .continuous))
                        }
                    }
                    .padding(.vertical, 6)
                    if index < rows.count - 1 {
                        Rectangle().fill(AppTheme.hairline).frame(height: 1)
                    }
                }
            }
            .padding(.horizontal, 12)
            .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))

            if source != nil || capturedAt != nil {
                Text("forrás: \(source ?? "–")" + (capturedAt.map { " · \(Self.stamp.string(from: $0))" } ?? ""))
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.tertiaryText)
            }
        }
    }
}

/// Kétoldali statisztika-sáv: minden mutatóhoz arányos csík.
struct StatsDuel: View {
    let homeName: String
    let awayName: String
    let rows: [(label: String, home: Double?, away: Double?, format: String)]

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("Statisztika").sectionLabel()
            VStack(spacing: AppTheme.Spacing.s) {
                HStack {
                    Text(homeName)
                        .font(.appLabel(9))
                        .foregroundStyle(AppTheme.secondaryText)
                        .lineLimit(1)
                    Spacer()
                    Text(awayName)
                        .font(.appLabel(9))
                        .foregroundStyle(AppTheme.secondaryText)
                        .lineLimit(1)
                }
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    statRow(row)
                }
            }
            .padding(AppTheme.Spacing.m)
            .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
        }
    }

    private func statRow(_ row: (label: String, home: Double?, away: Double?, format: String)) -> some View {
        let total = (row.home ?? 0) + (row.away ?? 0)
        let homeShare = total > 0 ? (row.home ?? 0) / total : 0.5

        return VStack(spacing: 4) {
            HStack {
                Text(format(row.home, row.format))
                    .font(.scoreboard(12, weight: .semibold))
                    .foregroundStyle(row.home == nil ? AppTheme.tertiaryText : AppTheme.primaryText)
                Spacer()
                Text(row.label)
                    .font(.appText(10))
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Text(format(row.away, row.format))
                    .font(.scoreboard(12, weight: .semibold))
                    .foregroundStyle(row.away == nil ? AppTheme.tertiaryText : AppTheme.primaryText)
            }
            GeometryReader { geo in
                HStack(spacing: 3) {
                    Capsule().fill(AppTheme.accent.opacity(0.85))
                        .frame(width: max(2, geo.size.width * homeShare - 1.5))
                    Capsule().fill(AppTheme.hairlineStrong)
                        .frame(width: max(2, geo.size.width * (1 - homeShare) - 1.5))
                }
            }
            .frame(height: 4)
        }
    }

    private func format(_ value: Double?, _ format: String) -> String {
        guard let value else { return "–" }
        if format == "%.2f" { return String(format: format, value) }
        return "\(Int(value))"
    }
}

/// Ellenőrzési sáv: forrás, szabálykészlet, blokkolók.
struct VerificationStrip: View {
    let gateState: PublishState?
    let ruleSetVersion: String?
    let evaluatedAt: Date?
    let blockers: [Blocker]

    private static let stamp: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "MM.dd. HH:mm"
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            HStack(spacing: AppTheme.Spacing.s) {
                Text("Ellenőrzés").sectionLabel()
                Spacer()
                if let ruleSetVersion {
                    Text("szabálykészlet \(ruleSetVersion)")
                        .font(.appLabel(9))
                        .foregroundStyle(AppTheme.tertiaryText)
                }
                if let evaluatedAt {
                    Text(Self.stamp.string(from: evaluatedAt))
                        .font(.appLabel(9).monospacedDigit())
                        .foregroundStyle(AppTheme.tertiaryText)
                }
            }
            HStack(alignment: .top, spacing: AppTheme.Spacing.s) {
                if let gateState { StatusBadge(state: gateState) }
                if blockers.isEmpty {
                    Text("Nincs blokkoló megjegyzés.")
                        .font(.appText(11))
                        .foregroundStyle(AppTheme.secondaryText)
                } else {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(blockers.enumerated()), id: \.offset) { _, blocker in
                            HStack(alignment: .firstTextBaseline, spacing: 6) {
                                Text(blocker.severity.uppercased())
                                    .font(.appLabel(8))
                                    .tracking(0.6)
                                    .foregroundStyle(blocker.severity == "blocker" ? AppTheme.warning : AppTheme.accent)
                                Text(blocker.message)
                                    .font(.appText(11))
                                    .foregroundStyle(AppTheme.primaryText)
                            }
                        }
                    }
                }
            }
            .padding(AppTheme.Spacing.m)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
        }
    }
}

/// Kerek csapat-címer nagyobb méretben; a képet a `LogoStore` adja.
struct CrestLogo: View {
    var name: String = ""
    let path: String?
    let store: LogoStore
    let size: CGFloat
    @State private var image: NSImage?

    private var monogram: String {
        let words = name.split(separator: " ")
        if words.count >= 2 {
            let letters = words.prefix(2).compactMap { $0.first }
            return String(letters).uppercased()
        }
        guard let word = words.first else { return "?" }
        return String(word.prefix(3)).uppercased()
    }

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: .fit)
                    .padding(size * 0.1)
            } else {
                Circle()
                    .fill(AppTheme.surface)
                    .overlay(
                        Text(monogram)
                            .font(.scoreboard(size * 0.34, weight: .semibold))
                            .foregroundStyle(AppTheme.secondaryText)
                    )
            }
        }
        .frame(width: size, height: size)
        .overlay(Circle().strokeBorder(AppTheme.hairline))
        .task(id: path) {
            guard let path else { return }
            image = await store.image(for: path)
        }
    }
}

/// Oldal-fejléc a lista-nézetekhez (Versenyek, Csapatok, Üzemeltetés).
struct PageHeader<Trailing: View>: View {
    let title: String
    let subtitle: String
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(alignment: .bottom, spacing: AppTheme.Spacing.l) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.scoreboard(40))
                    .tracking(-1.2)
                    .foregroundStyle(AppTheme.primaryText)
                Text(subtitle)
                    .font(.appText(12.5))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
            trailing
        }
        .padding(.horizontal, 48)
        .padding(.top, 24)
        .padding(.bottom, 16)
    }
}

/// Keresőmező a lista-nézetekhez.
struct SearchField: View {
    @Binding var text: String
    var placeholder: String

    var body: some View {
        HStack(spacing: 7) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .font(.appText(12.5))
                .foregroundStyle(AppTheme.primaryText)
                .frame(width: 210)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(AppTheme.surface, in: Capsule())
        .overlay(Capsule().strokeBorder(AppTheme.hairline))
    }
}

/// Kerek ikon-gomb a fejlécekhez (frissítés, zárás, stb.).
struct RoundIconButton: View {
    let systemName: String
    var help: String = ""
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .frame(width: 32, height: 32)
                .background(AppTheme.surface, in: Circle())
                .overlay(Circle().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityLabel(help.isEmpty ? systemName : help)
    }
}
