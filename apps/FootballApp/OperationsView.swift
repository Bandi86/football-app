import SwiftUI
import FootballAppCore

/// Üzemeltetés: worker-számlálók, függő review-k és forrásonkénti
/// policy/breaker/kvóta állapot. Read-only, 30 mp-enként frissül.
struct OperationsView: View {
    @State private var model = OpsModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            content
        }
        .background(AppTheme.background)
        .task { await model.refreshLoop() }
    }

    private var header: some View {
        HStack(alignment: .bottom) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Üzemeltetés")
                    .font(.system(size: 44, weight: .bold))
                    .tracking(-1.5)
                    .foregroundStyle(AppTheme.primaryText)
                Text("worker · review · forrásállapot")
                    .font(.system(size: 12))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
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
        case .loaded(let status):
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    statCards(status)
                    sourceList(status.sources)
                }
                .padding(.horizontal, 48)
                .padding(.vertical, 20)
            }
        }
    }

    private func statCards(_ status: OpsStatus) -> some View {
        HStack(spacing: 12) {
            statCard("Függő jobok", value: status.pendingJobs,
                     accent: status.pendingJobs > 0 ? AppTheme.accent : nil)
            statCard("Lease-elt", value: status.leasedJobs, accent: nil)
            statCard("Karantén", value: status.quarantined,
                     accent: status.quarantined > 0 ? .red : nil)
            statCard("Függő review", value: status.pendingReviews,
                     accent: status.pendingReviews > 0 ? .orange : nil)
        }
    }

    private func statCard(_ title: String, value: Int, accent: Color?) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(value)")
                .font(.system(size: 30, weight: .bold).monospacedDigit())
                .foregroundStyle(accent ?? AppTheme.primaryText)
            Text(title.uppercased())
                .font(.system(size: 9.5, weight: .semibold))
                .tracking(1.2)
                .foregroundStyle(AppTheme.secondaryText)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
    }

    private func sourceList(_ sources: [OpsSource]) -> some View {
        VStack(spacing: 0) {
            HStack {
                Text("FORRÁSOK")
                    .font(.system(size: 10, weight: .semibold))
                    .tracking(1.4)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Text("\(sources.count)")
                    .font(.system(size: 11).monospacedDigit())
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .background(AppTheme.surface)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            ForEach(Array(sources.enumerated()), id: \.element.id) { index, source in
                sourceRow(source)
                if index < sources.count - 1 {
                    Rectangle().fill(AppTheme.hairline).frame(height: 1).padding(.leading, 18)
                }
            }
        }
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    private func sourceRow(_ source: OpsSource) -> some View {
        HStack(spacing: 14) {
            Circle()
                .fill(source.enabled ? AppTheme.liveGreen : AppTheme.secondaryText.opacity(0.4))
                .frame(width: 7, height: 7)

            Text(source.sourceSlug)
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(AppTheme.primaryText)
                .frame(width: 150, alignment: .leading)

            modePill(source.mode)

            Spacer()

            quota(source)
            breakerChip(source.breakerState)
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 12)
    }

    private func modePill(_ mode: String) -> some View {
        Text(mode.uppercased())
            .font(.system(size: 9, weight: .bold))
            .tracking(0.8)
            .foregroundStyle(modeColor(mode))
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(modeColor(mode).opacity(0.14), in: Capsule())
    }

    private func modeColor(_ mode: String) -> Color {
        switch mode {
        case "live": return AppTheme.liveGreen
        case "canary": return AppTheme.accent
        case "shadow": return AppTheme.secondaryText
        default: return AppTheme.secondaryText
        }
    }

    private func quota(_ source: OpsSource) -> some View {
        let ratio = source.dailyBudget > 0
            ? Double(source.attemptsToday) / Double(source.dailyBudget) : 0
        return VStack(alignment: .trailing, spacing: 4) {
            Text("\(source.attemptsToday)/\(source.dailyBudget) ma")
                .font(.system(size: 11).monospacedDigit())
                .foregroundStyle(AppTheme.secondaryText)
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    Capsule().fill(AppTheme.surface)
                    Capsule()
                        .fill(ratio >= 1 ? Color.orange : AppTheme.accent)
                        .frame(width: max(2, geometry.size.width * min(ratio, 1)))
                }
            }
            .frame(width: 90, height: 4)
        }
    }

    @ViewBuilder
    private func breakerChip(_ state: String) -> some View {
        if state != "closed" {
            Text(state == "open" ? "BREAKER NYITOTT" : "FÉLIG NYITOTT")
                .font(.system(size: 9, weight: .bold))
                .tracking(0.8)
                .foregroundStyle(.red)
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(.red.opacity(0.14), in: Capsule())
        } else {
            Text("OK")
                .font(.system(size: 9, weight: .semibold))
                .tracking(0.8)
                .foregroundStyle(AppTheme.secondaryText.opacity(0.6))
        }
    }
}
