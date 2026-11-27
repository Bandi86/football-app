import SwiftUI
import FootballAppCore

/// Üzemeltetés: worker-számlálók, hiányjelek, integritás és forrásállapot.
/// A jeleket a napi ellenőrzés rögzíti; a kliens csak olvas (S2).
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
        .sheet(isPresented: Binding(
            get: { model.selectedIssue != nil },
            set: { if !$0 { model.closeIssue() } }
        )) {
            issueDetailSheet
        }
    }

    private var header: some View {
        PageHeader(title: "Üzemeltetés", subtitle: "worker · hiányjelek · forrásállapot") {
            RoundIconButton(systemName: "arrow.clockwise", help: "Frissítés") {
                Task {
                    await model.load()
                    await model.loadIssues()
                    await model.loadSummary()
                    await model.loadAudit()
                }
            }
        }
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
                    summaryBanner
                    statCards(status)
                    issuesSection
                    if let integrity = status.integrity {
                        integritySection(integrity)
                    }
                    sourceList(status.sources)
                    auditSection
                }
                .padding(.horizontal, 48)
                .padding(.vertical, 20)
            }
        }
    }

    private func statCards(_ status: OpsStatus) -> some View {
        HStack(spacing: 12) {
            statCard("Függő jobok", value: status.pendingJobs,
                     accent: status.pendingJobs > 0 ? AppTheme.accent : nil) {
                model.query = IssuesQueryDTO(code: "stale_status", openOnly: true)
            }
            statCard("Karantén", value: status.quarantined,
                     accent: status.quarantined > 0 ? .red : nil) {
                model.query = IssuesQueryDTO(code: "source_conflict", openOnly: true)
            }
            statCard("Függő review", value: status.pendingReviews,
                     accent: status.pendingReviews > 0 ? .orange : nil) {
                model.query = IssuesQueryDTO(code: "identity_review", openOnly: true)
            }
            statCard("Hiányjelek", value: issuesCount,
                     accent: issuesCount > 0 ? AppTheme.accent : nil) {
                model.query = IssuesQueryDTO(openOnly: true)
            }
        }
    }

    private var issuesCount: Int {
        if case .loaded(let response) = model.issuesState { return response.count }
        return 0
    }

    private func statCard(
        _ title: String,
        value: Int,
        accent: Color?,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 6) {
                Text("\(value)")
                    .font(.scoreboard(30))
                    .foregroundStyle(accent ?? AppTheme.primaryText)
                Text(title.uppercased())
                    .font(.appLabel())
                    .tracking(1.2)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(title): \(value). Szűrés erre a jelre.")
        .accessibilityIdentifier("ops.card.\(title.lowercased().replacingOccurrences(of: " ", with: "."))")
        .padding(16)
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
    }

    // MARK: - Összefoglaló és globális audit (S3)

    private var summaryBanner: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("ÖSSZEFOGLALÓ")
                    .font(.appLabel())
                    .tracking(1.4)
                    .foregroundStyle(AppTheme.secondaryText)
                    .accessibilityIdentifier(AccessibilityLabels.opsSummaryIdentifier)
                Spacer()
                summaryDatePicker
            }
            summaryContent
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(AppTheme.accentSoft, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .strokeBorder(AppTheme.hairlineStrong)
        )
    }

    /// A dátumválasztó a mai napot és az előző hat napot kínálja;
    /// a kulcs pontosan az API `date` paramétere (S174).
    private var summaryDatePicker: some View {
        Picker(AccessibilityLabels.opsSummaryDateLabel, selection: Binding(
            get: { model.summaryDate },
            set: { model.summaryDate = $0 }
        )) {
            ForEach(OpsFormatting.summaryDateOptions(), id: \.key) { option in
                Text(option.label).tag(option.key)
            }
        }
        .pickerStyle(.menu)
        .frame(width: 170)
        .accessibilityLabel(AccessibilityLabels.opsSummaryDateLabel)
        .accessibilityIdentifier(AccessibilityLabels.opsSummaryDateIdentifier)
    }

    @ViewBuilder
    private var summaryContent: some View {
        switch model.summaryState {
        case .idle, .loading:
            EmptyView()
        case .error:
            Text("Az összefoglaló nem tölthető be.")
                .font(.appLabel())
                .foregroundStyle(AppTheme.secondaryText)
        case .loaded(let summary):
            VStack(alignment: .leading, spacing: 10) {
                Text(summary.headlineHu)
                    .font(.appText(15, weight: .medium))
                    .foregroundStyle(AppTheme.primaryText)
                HStack(spacing: 10) {
                    Text("Előző nap: \(summary.previousDayTotal)")
                        .font(.appLabel())
                        .foregroundStyle(AppTheme.secondaryText)
                    ForEach(summary.topActions) { action in
                        Button(action.labelHu) {
                            if let issue = model.issue(withID: action.issueId) {
                                model.selectIssue(issue)
                            }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .accessibilityIdentifier("ops.summary.action.\(action.issueId.uuidString)")
                    }
                }
            }
        }
    }

    private var auditSection: some View {
        VStack(spacing: 0) {
            HStack {
                Text("AUDIT")
                    .font(.appLabel())
                    .tracking(1.4)
                    .foregroundStyle(AppTheme.secondaryText)
                    .accessibilityIdentifier(AccessibilityLabels.opsAuditIdentifier)
                Spacer()
                Picker("Tárgy", selection: Binding(
                    get: { model.auditSubjectKind ?? "" },
                    set: { model.auditSubjectKind = $0.isEmpty ? nil : $0 }
                )) {
                    Text("Minden tárgy").tag("")
                    ForEach(["match", "team", "competition", "day"], id: \.self) { kind in
                        Text(kind).tag(kind)
                    }
                }
                .pickerStyle(.menu)
                .frame(width: 160)
                .accessibilityIdentifier("ops.audit.filter")
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .background(AppTheme.surface)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            auditList
        }
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    @ViewBuilder
    private var auditList: some View {
        switch model.auditState {
        case .idle, .loading:
            ProgressView().padding(20)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: { Task { await model.loadAudit() } })
        case .loaded(let response):
            if response.entries.isEmpty {
                Text(AccessibilityLabels.auditEmptyTitle)
                    .foregroundStyle(AppTheme.secondaryText)
                    .padding(20)
                    .accessibilityIdentifier(AccessibilityLabels.auditEmptyIdentifier)
            } else {
                ForEach(response.entries) { entry in
                    auditRow(entry)
                    Rectangle().fill(AppTheme.hairline).frame(height: 1).padding(.leading, 18)
                }
            }
        }
    }

    private func auditRow(_ entry: AuditEntryRowDTO) -> some View {
        let time = OpsFormatting.timestamp(entry.createdAt)
        let subject = "\(entry.subjectKind): \(entry.subjectId?.uuidString.prefix(8) ?? "-")"
        return HStack(spacing: 12) {
            Image(systemName: "clock.arrow.circlepath").foregroundStyle(AppTheme.secondaryText)
            VStack(alignment: .leading, spacing: 3) {
                Text(entry.action)
                    .font(.appText(13))
                    .foregroundStyle(AppTheme.primaryText)
                    .accessibilityIdentifier(AccessibilityLabels.auditEntryIdentifier(for: entry.id))
                Text("\(time) · \(entry.actor) · \(subject)")
                    .font(.appLabel())
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
        .accessibilityLabel(AccessibilityLabels.auditEntryLabel(
            time: time, actor: entry.actor, action: entry.action, subject: subject
        ))
    }

    // MARK: - Hiányjelek

    private var issuesSection: some View {
        VStack(spacing: 0) {
            HStack {
                Text("HIÁNYJELEK")
                    .font(.appLabel())
                    .tracking(1.4)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Text("\(issuesCount)")
                    .font(.scoreboard(11, weight: .medium))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .background(AppTheme.surface)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            filters
            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            issueList
        }
        .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(AppTheme.hairline)
        )
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    private var filters: some View {
        HStack(spacing: 12) {
            Picker("Hiány", selection: Binding(
                get: { model.query.code ?? "" },
                set: { model.query.code = $0.isEmpty ? nil : $0 }
            )) {
                Text("Minden hiány").tag("")
                ForEach(["missing_logo", "missing_odds", "open_past", "missing_result",
                         "duplicate_suspect", "identity_review", "stale_status",
                         "source_conflict"], id: \.self) { code in
                    Text(OpsFormatting.issueCodeLabel(issueCode(from: code))).tag(code)
                }
            }
            .pickerStyle(.menu)
            .frame(width: 190)
            .accessibilityIdentifier(AccessibilityLabels.issuesFilterIdentifier)

            Picker("Súlyosság", selection: Binding(
                get: { model.query.severity ?? "" },
                set: { model.query.severity = $0.isEmpty ? nil : $0 }
            )) {
                Text("Minden súlyosság").tag("")
                ForEach(["info", "warning", "hard"], id: \.self) { value in
                    Text(severityText(value)).tag(value)
                }
            }
            .pickerStyle(.menu)
            .frame(width: 180)

            Toggle("Csak megoldatlan", isOn: Binding(
                get: { model.query.openOnly },
                set: { model.query.openOnly = $0 }
            ))
            .toggleStyle(.switch)
            .accessibilityIdentifier(AccessibilityLabels.issuesOpenOnlyIdentifier)

            Spacer()
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
    }

    @ViewBuilder
    private var issueList: some View {
        switch model.issuesState {
        case .idle, .loading:
            ProgressView().padding(24)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: { Task { await model.loadIssues() } })
        case .loaded(let response):
            if response.issues.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "checkmark.circle").foregroundStyle(.secondary)
                    Text(AccessibilityLabels.issuesEmptyTitle)
                        .foregroundStyle(AppTheme.secondaryText)
                    Text(AccessibilityLabels.issuesEmptySubtitle)
                        .font(.appLabel())
                        .foregroundStyle(AppTheme.tertiaryText)
                }
                .padding(24)
                .accessibilityLabel(AccessibilityLabels.issuesEmptyLabel())
                .accessibilityIdentifier(AccessibilityLabels.issuesEmptyIdentifier)
            } else {
                ForEach(Array(response.issues.enumerated()), id: \.element.id) { index, issue in
                    issueRow(issue)
                    if index < response.issues.count - 1 {
                        Rectangle().fill(AppTheme.hairline).frame(height: 1).padding(.leading, 18)
                    }
                }
            }
        }
    }

    private func issueRow(_ issue: IssueRowDTO) -> some View {
        let code = OpsFormatting.issueCodeLabel(issue.code)
        let severity = OpsFormatting.severityLabel(issue.severity)
        let subject = "\(OpsFormatting.subjectLabel(issue.subjectKind)): \(issue.subjectId.uuidString.prefix(8))"
        let first = OpsFormatting.timestamp(issue.firstSeenAt)
        let last = OpsFormatting.timestamp(issue.lastSeenAt)
        let source = OpsFormatting.sourceLabel(issue.source)
        return HStack(spacing: 0) {
            Button { model.selectIssue(issue) } label: {
                HStack(spacing: 14) {
                    Image(systemName: issue.resolvedAt == nil ? "exclamationmark.triangle" : "checkmark.circle")
                        .foregroundStyle(issue.resolvedAt == nil ? AppTheme.accent : AppTheme.liveGreen)
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 8) {
                            Text(code).font(.appText(14)).foregroundStyle(AppTheme.primaryText)
                            severityPill(issue.severity, text: severity)
                        }
                        Text("\(subject) · első: \(first) · utolsó: \(last) · \(source)")
                            .font(.appLabel())
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    Spacer()
                    if issue.resolvedAt != nil {
                        Text("MEGOLDVA")
                            .font(.appLabel(9))
                            .foregroundStyle(AppTheme.liveGreen)
                    }
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 12)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(AccessibilityLabels.issueLabel(
                code: code, severity: severity, subject: subject,
                firstSeen: first, lastSeen: last, source: source,
                resolved: issue.resolvedAt != nil
            ))
            .accessibilityIdentifier(AccessibilityLabels.issueIdentifier(for: issue.id))

            if issue.resolvedAt == nil {
                Button {
                    Task { await model.resolveIssue(issue.id) }
                } label: {
                    Image(systemName: "checkmark.circle")
                        .foregroundStyle(AppTheme.liveGreen)
                        .frame(width: 44, height: 44)
                }
                .buttonStyle(.plain)
                .help(AccessibilityLabels.issueResolveLabel)
                .accessibilityLabel(AccessibilityLabels.issueResolveLabel)
                .accessibilityIdentifier(AccessibilityLabels.issueResolveIdentifier(for: issue.id))
            }
        }
    }

    @ViewBuilder
    private func severityPill(_ severity: IssueSeverity, text: String) -> some View {
        let color: Color = switch severity {
        case .hard: AppTheme.warning
        case .warning: AppTheme.accent
        case .info: AppTheme.secondaryText
        case .unknown: AppTheme.secondaryText
        }
        Text(text.uppercased())
            .font(.appLabel(9))
            .tracking(0.6)
            .foregroundStyle(color)
            .padding(.horizontal, 7)
            .padding(.vertical, 2)
            .background(color.opacity(0.14), in: Capsule())
    }

    private func severityText(_ raw: String) -> String {
        let severity: IssueSeverity = switch raw {
        case "info": .info
        case "warning": .warning
        case "hard": .hard
        default: .unknown
        }
        return OpsFormatting.severityLabel(severity)
    }

    private func issueCode(from raw: String) -> IssueCode {
        switch raw {
        case "missing_logo": return .missingLogo
        case "missing_odds": return .missingOdds
        case "open_past": return .openPast
        case "missing_result": return .missingResult
        case "duplicate_suspect": return .duplicateSuspect
        case "identity_review": return .identityReview
        case "stale_status": return .staleStatus
        case "source_conflict": return .sourceConflict
        default: return .other(raw)
        }
    }

    // MARK: - Meccs-szintű idővonal (a globális audit feed S3-ban jön)

    @ViewBuilder
    private var issueDetailSheet: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let issue = model.selectedIssue {
                Text(AccessibilityLabels.issueDetailTitle(
                    OpsFormatting.issueCodeLabel(issue.code)
                ))
                .font(.appText(17, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)

                if issue.subjectKind != .match {
                    Text("A részletes idővonal meccsekhez érhető el.")
                        .foregroundStyle(AppTheme.secondaryText)
                        .accessibilityIdentifier(AccessibilityLabels.historyEmptyIdentifier)
                } else {
                    historyContent
                }
            }
            Spacer()
            HStack {
                Spacer()
                Button("Bezárás") { model.closeIssue() }
                    .keyboardShortcut(.cancelAction)
            }
        }
        .padding(24)
        .frame(minWidth: 560, minHeight: 320)
        .background(AppTheme.background)
    }

    @ViewBuilder
    private var historyContent: some View {
        switch model.historyState {
        case .idle, .loading:
            ProgressView().frame(maxWidth: .infinity)
        case .error(let error):
            ErrorStateView(message: error.userMessage, retry: {
                if let issue = model.selectedIssue { Task { await model.loadHistory(for: issue) } }
            })
        case .loaded(let response):
            if response.entries.isEmpty {
                Text(AccessibilityLabels.historyEmptyTitle)
                    .foregroundStyle(AppTheme.secondaryText)
                    .accessibilityLabel(AccessibilityLabels.historyEmptyLabel())
                    .accessibilityIdentifier(AccessibilityLabels.historyEmptyIdentifier)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(response.entries.enumerated()), id: \.offset) { index, entry in
                            historyRow(index: index, entry: entry)
                            if index < response.entries.count - 1 {
                                Rectangle().fill(AppTheme.hairline).frame(height: 1)
                            }
                        }
                    }
                }
            }
        }
    }

    private func historyRow(index: Int, entry: HistoryEntryDTO) -> some View {
        let time = OpsFormatting.timestamp(entry.at)
        let source = entry.source ?? (entry.origin == .audit ? "kézi bejegyzés" : "forrás")
        return VStack(alignment: .leading, spacing: 4) {
            Text(entry.action).font(.appText(13)).foregroundStyle(AppTheme.primaryText)
            HStack(spacing: 8) {
                Text(time)
                Text("·")
                Text(entry.actor)
                Text("·")
                Text(source)
            }
            .font(.appLabel())
            .foregroundStyle(AppTheme.secondaryText)
            if let field = entry.fieldKey {
                Text("mező: \(field)").font(.appLabel()).foregroundStyle(AppTheme.tertiaryText)
            }
            if let before = entry.before, let after = entry.after {
                Text("\(before.displayText) → \(after.displayText)")
                    .font(.appLabel())
                    .foregroundStyle(AppTheme.secondaryText)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 10)
        .accessibilityLabel(AccessibilityLabels.historyEntryLabel(
            time: time, actor: entry.actor, action: entry.action,
            field: entry.fieldKey, source: entry.source
        ))
        .accessibilityIdentifier(AccessibilityLabels.historyEntryIdentifier(for: index))
    }

    // MARK: - Integritás és források (változatlan)

    private func integritySection(_ integrity: OpsIntegrity) -> some View {
        let ok = integrity.verdict == "OK"
        let metrics: [(String, Int)] = [
            ("Duplikátum", integrity.duplicateFixtureGroups + integrity.duplicateKickoffGroups),
            ("Árva bizonyíték", integrity.orphanProvenance),
            ("Withheld", integrity.withheldGates),
            ("Failed job", integrity.failedJobs),
        ]
        return VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("INTEGRITÁS")
                    .font(.appLabel())
                    .tracking(1.4)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Text(integrity.verdict)
                    .font(.system(size: 11, weight: .bold).monospacedDigit())
                    .foregroundStyle(ok ? .green : .red)
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .background(AppTheme.surface)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            HStack(spacing: 0) {
                ForEach(Array(metrics.enumerated()), id: \.offset) { index, metric in
                    VStack(alignment: .leading, spacing: 4) {
                        Text("\(metric.1)")
                            .font(.scoreboard(20))
                            .foregroundStyle(metric.1 > 0 ? .orange : AppTheme.primaryText)
                        Text(metric.0.uppercased())
                            .font(.system(size: 9, weight: .semibold))
                            .tracking(0.8)
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(14)
                    if index < metrics.count - 1 {
                        Rectangle().fill(AppTheme.hairline).frame(width: 1)
                    }
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

    private func sourceList(_ sources: [OpsSource]) -> some View {
        VStack(spacing: 0) {
            HStack {
                Text("FORRÁSOK")
                    .font(.appLabel())
                    .tracking(1.4)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Text("\(sources.count)")
                    .font(.scoreboard(11, weight: .medium))
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
                .font(.appText(14))
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
            .font(.appLabel(9))
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
                .font(.scoreboard(11, weight: .medium))
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
                .font(.appLabel(9))
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
