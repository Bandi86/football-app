import SwiftUI
import FootballAppCore

/// A meccs esemény-idővonala: percenkénti események, típus szerinti ikonnal,
/// csapatoldal-jelzéssel és a gól utáni állással. A félidő szekció-elválasztó
/// vonal, hogy a két játékrész vizuálisan is elváljon. Csak akkor jelenik meg,
/// ha van esemény; üres állapotban a szakasz rejtve marad.
struct MatchTimeline: View {
    let homeName: String
    let awayName: String
    let incidents: [MatchIncident]

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            HStack(alignment: .firstTextBaseline) {
                Text("Események").sectionLabel()
                Spacer()
                Text(incidentCountLabel)
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.tertiaryText)
            }

            VStack(spacing: 0) {
                ForEach(Array(incidents.enumerated()), id: \.offset) { index, incident in
                    if incident.kind == .half {
                        halfDivider(incident)
                            .accessibilityIdentifier("match.timeline.row.\(index)")
                    } else {
                        row(incident, index: index)
                            .accessibilityIdentifier("match.timeline.row.\(index)")
                    }
                }
            }
            .padding(.horizontal, AppTheme.Spacing.m)
            .padding(.vertical, AppTheme.Spacing.s)
            .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
        }
        .accessibilityIdentifier("match.timeline")
    }

    private var incidentCountLabel: String {
        "\(incidents.count) esemény"
    }

    // MARK: - Sorok

    private func row(_ incident: MatchIncident, index: Int) -> some View {
        HStack(alignment: .top, spacing: AppTheme.Spacing.s) {
            Text(minuteText(incident.minute))
                .font(.scoreboard(12, weight: .semibold))
                .foregroundStyle(incident.minute == nil ? AppTheme.tertiaryText : AppTheme.secondaryText)
                .frame(width: 40, alignment: .trailing)
                .accessibilityHidden(true)

            Image(systemName: iconName(for: incident))
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(iconColor(for: incident))
                .frame(width: 20, height: 18)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(primaryText(incident))
                        .font(.appText(12.5, weight: incident.kind == .goal ? .semibold : .medium))
                        .foregroundStyle(incident.kind == .goal ? AppTheme.primaryText : AppTheme.secondaryText)
                        .lineLimit(2)
                    sideBadge(incident)
                }
                if let secondary = secondaryText(incident) {
                    Text(secondary)
                        .font(.appText(10.5))
                        .foregroundStyle(AppTheme.tertiaryText)
                        .lineLimit(2)
                }
            }

            Spacer(minLength: 0)

            if incident.kind == .goal,
               let home = incident.scoreAfterHome,
               let away = incident.scoreAfterAway {
                Text("\(home)–\(away)")
                    .font(.scoreboard(12, weight: .semibold))
                    .foregroundStyle(AppTheme.accent)
                    .accessibilityHidden(true)
            }
        }
        .padding(.vertical, 5)
        .padding(.horizontal, AppTheme.Spacing.s)
        .overlay(alignment: .leading) {
            if incident.isHome || incident.isAway {
                Capsule()
                    .fill(sideColor(incident))
                    .frame(width: 2)
                    .padding(.vertical, 4)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityLabel(incident))
        .help(tooltip(incident))
    }

    /// A félidő szekció-elválasztó: teljes szélességű hajszálvonal a játékrészek között.
    private func halfDivider(_ incident: MatchIncident) -> some View {
        HStack(spacing: AppTheme.Spacing.s) {
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            HStack(spacing: 5) {
                Image(systemName: "flag.checkered")
                    .font(.system(size: 10, weight: .semibold))
                Text(halfLabel(incident.minute))
                    .font(.appLabel(9))
                    .tracking(1.2)
            }
            .foregroundStyle(AppTheme.secondaryText)
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
        }
        .padding(.vertical, 8)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityLabel(incident))
    }

    // MARK: - Megjelenített szövegek

    private func minuteText(_ minute: Int?) -> String {
        minute.map { "\($0)′" } ?? "–"
    }

    private func halfLabel(_ minute: Int?) -> String {
        guard let minute else { return "FÉLIDŐ" }
        return "FÉLIDŐ · \(minute)′"
    }

    private func primaryText(_ incident: MatchIncident) -> String {
        switch incident.kind {
        case .substitution:
            if let player = incident.playerName, let related = incident.relatedPlayerName {
                return "\(player) ↔ \(related)"
            }
        case .goal:
            return incident.playerName ?? "Gól"
        case .card:
            return incident.playerName ?? "Kártya"
        case .missedPenalty:
            return incident.playerName ?? "Kihagyott büntető"
        case .penaltyShootout:
            return incident.playerName ?? "Tizenegyespárbaj"
        case .addedTime:
            return incident.playerName ?? "Ráadásidő"
        case .varReview:
            return incident.detail ?? "VAR-ellenőrzés"
        case .half:
            return "Félidő"
        case .other:
            break
        }
        if let player = incident.playerName, !player.isEmpty { return player }
        if let related = incident.relatedPlayerName, !related.isEmpty { return related }
        if let detail = incident.detail, !detail.isEmpty { return detail }
        return incident.incidentType.isEmpty ? "Esemény" : incident.incidentType
    }

    private func secondaryText(_ incident: MatchIncident) -> String? {
        switch incident.kind {
        case .substitution:
            return incident.detail
        case .varReview:
            return incident.playerName
        case .card:
            if let detail = cardDetail(incident) { return detail }
            return nil
        default:
            break
        }
        if let detail = incident.detail, !detail.isEmpty { return detail }
        if incident.playerName != nil,
           let related = incident.relatedPlayerName,
           !related.isEmpty {
            return related
        }
        return nil
    }

    /// A kártya színét a forrás `detail` szövegéből mondjuk ki magyarul; ha a
    /// részlet nem nevezi meg a színt, a nyers forrás-szöveget mutatjuk.
    private func cardDetail(_ incident: MatchIncident) -> String? {
        guard let detail = incident.detail, !detail.isEmpty else { return nil }
        let lowered = detail.lowercased()
        if lowered.contains("red") { return "Piros lap" }
        if lowered.contains("yellow") { return "Sárga lap" }
        return detail
    }

    private func accessibilityLabel(_ incident: MatchIncident) -> String {
        AccessibilityLabels.matchIncidentLabel(
            minute: incident.minute,
            kind: incident.kind,
            side: incident.side,
            playerName: incident.playerName,
            relatedPlayerName: incident.relatedPlayerName,
            detail: incident.detail,
            scoreAfterHome: incident.scoreAfterHome,
            scoreAfterAway: incident.scoreAfterAway
        )
    }

    private func tooltip(_ incident: MatchIncident) -> String {
        var parts: [String] = []
        if let minute = incident.minute {
            parts.append("\(minute). perc")
        }
        if incident.isHome {
            parts.append(homeName)
        } else if incident.isAway {
            parts.append(awayName)
        }
        parts.append(AccessibilityLabels.incidentTypeLabel(incident.kind))
        return parts.joined(separator: " · ")
    }

    // MARK: - Ikonok és oldaljelzés

    private func sideColor(_ incident: MatchIncident) -> Color {
        incident.isHome ? AppTheme.accent : AppTheme.predictionAway
    }

    @ViewBuilder
    private func sideBadge(_ incident: MatchIncident) -> some View {
        if incident.isHome || incident.isAway {
            Text(incident.isHome ? "H" : "V")
                .font(.appLabel(8))
                .foregroundStyle(sideColor(incident))
                .frame(width: 16, height: 14)
                .background(
                    sideColor(incident).opacity(0.14),
                    in: RoundedRectangle(cornerRadius: 4, style: .continuous)
                )
                .help(incident.isHome ? "Hazai – \(homeName)" : "Vendég – \(awayName)")
                .accessibilityHidden(true)
        }
    }

    private func iconName(for incident: MatchIncident) -> String {
        switch incident.kind {
        case .goal: return "soccerball"
        case .card: return "rectangle.portrait.fill"
        case .substitution: return "arrow.left.arrow.right"
        case .half: return "flag.checkered"
        case .addedTime: return "clock"
        case .varReview: return "video"
        case .missedPenalty: return "xmark.circle"
        case .penaltyShootout: return "target"
        case .other: return "circle"
        }
    }

    private func iconColor(for incident: MatchIncident) -> Color {
        switch incident.kind {
        case .goal, .penaltyShootout:
            return AppTheme.accent
        case .card:
            return isRedCard(incident) ? AppTheme.warning : AppTheme.accent
        case .missedPenalty:
            return AppTheme.warning
        case .substitution, .half, .addedTime, .varReview:
            return AppTheme.secondaryText
        case .other:
            return AppTheme.tertiaryText
        }
    }

    private func isRedCard(_ incident: MatchIncident) -> Bool {
        guard let detail = incident.detail else { return false }
        return detail.lowercased().contains("red")
    }
}