import SwiftUI
import FootballAppCore

/// A szerkesztőlap célpontja: a betöltött meccs pillanatképe, hogy a mentés
/// utáni újratöltés ne tüntesse el a lap tartalmát.
private struct ManualEditTarget: Identifiable {
    let detail: MatchDetail
    var id: UUID { detail.id }
}

/// Meccs-részletező panel: eredménytábla-fejléc, meta-rács, odds-mátrix,
/// statisztika-párbaj és ellenőrzési sáv. Minden szakasz adatvezérelt:
/// ami hiányzik, az nem jelenik meg, hamis értéket nem rajzolunk.
struct MatchDetailSheet: View {
    let matchId: UUID
    @Environment(\.dismiss) private var dismiss
    @State private var model = MatchDetailModel()
    @State private var manualEditTarget: ManualEditTarget?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Meccs")
                    .font(.appLabel(11))
                    .tracking(2.4)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Button {
                    manualEditTarget = model.detail.map(ManualEditTarget.init)
                } label: {
                    Label("Meccs szerkesztése", systemImage: "square.and.pencil")
                }
                .buttonStyle(.plain)
                .foregroundStyle(model.detail == nil ? AppTheme.tertiaryText : AppTheme.accent)
                .disabled(model.detail == nil)
                .help(model.detail == nil
                      ? "A szerkesztés a meccs betöltése után érhető el."
                      : "Kezdés, státusz és eredmény szerkesztése")
                .accessibilityIdentifier("match.detail.manual")
                Button("Bezárás") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                    .buttonStyle(.plain)
                    .foregroundStyle(AppTheme.secondaryText)
            }
            .padding(.horizontal, AppTheme.Spacing.xl)
            .padding(.vertical, AppTheme.Spacing.m)

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            content
        }
        .frame(minWidth: 720, minHeight: 620)
        .background(AppTheme.background)
        .accessibilityIdentifier("match.detail")
        .task(id: matchId) { await model.load(matchId: matchId) }
        .sheet(item: $manualEditTarget) { target in
            ManualResultSheet(model: model, detail: target.detail)
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
            ErrorStateView(message: error.userMessage, retry: {
                Task { await model.load(matchId: matchId) }
            })
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let detail):
            MatchDetailContent(detail: detail, logos: model.logos)
        }
    }
}

/// Teljes meccsszerkesztő lap: kezdés, státusz, pontosság, eredmény-blokkok
/// és a kötelező indoklás. A payloadot a `ManualEditRules` építi, a lap csak
/// megjelenít és összegyűjt; a kiszolgáló hibaszövegét változtatás nélkül
/// jeleníti meg.
struct ManualResultSheet: View {
    let model: MatchDetailModel
    let detail: MatchDetail
    @Environment(\.dismiss) private var dismiss

    @State private var status: String
    @State private var kickoff: Date
    @State private var precision: String
    @State private var homeFt: String
    @State private var awayFt: String
    @State private var homeHt: String
    @State private var awayHt: String
    @State private var homeEt: String
    @State private var awayEt: String
    @State private var homePens: String
    @State private var awayPens: String
    @State private var note = ""
    @State private var showValidation = false

    init(model: MatchDetailModel, detail: MatchDetail) {
        self.model = model
        self.detail = detail
        let initialPrecision = KickoffPrecision(rawValue: detail.kickoffPrecision) ?? .exact
        let initialStatus = ManualEditRules.statuses.contains(detail.status) ? detail.status : "scheduled"
        let initialKickoff = initialPrecision == .dateOnly
            ? ManualEditRules.dateOnlyPlaceholder(for: detail.kickoffAt)
            : detail.kickoffAt
        _status = State(initialValue: initialStatus)
        _kickoff = State(initialValue: initialKickoff)
        _precision = State(initialValue: initialPrecision.rawValue)
        _homeFt = State(initialValue: Self.scoreText(detail.homeFt))
        _awayFt = State(initialValue: Self.scoreText(detail.awayFt))
        _homeHt = State(initialValue: Self.scoreText(detail.homeHt))
        _awayHt = State(initialValue: Self.scoreText(detail.awayHt))
        _homeEt = State(initialValue: Self.scoreText(detail.homeEt))
        _awayEt = State(initialValue: Self.scoreText(detail.awayEt))
        _homePens = State(initialValue: Self.scoreText(detail.homePens))
        _awayPens = State(initialValue: Self.scoreText(detail.awayPens))
    }

    private static func scoreText(_ value: Int?) -> String {
        value.map(String.init) ?? ""
    }

    private var isDateOnly: Bool {
        precision == KickoffPrecision.dateOnly.rawValue
    }

    /// A pickerek a verseny időzónájában jelennek meg; `date_only` esetén
    /// UTC-ben, mert a naptári nap a 12:00Z helyőrző UTC-dátuma.
    private var displayTimeZone: TimeZone {
        ManualEditRules.kickoffTimeZone(
            competitionTimezone: detail.competitionTimezone,
            precision: precision
        )
    }

    private var ftHome: Int? { ManualEditRules.parseScore(homeFt) }
    private var ftAway: Int? { ManualEditRules.parseScore(awayFt) }
    private var etHome: Int? { ManualEditRules.parseScore(homeEt) }
    private var etAway: Int? { ManualEditRules.parseScore(awayEt) }

    private var currentDecidedBy: String? {
        ManualEditRules.decidedBy(
            status: status,
            homeFt: ftHome,
            awayFt: ftAway,
            homeEt: etHome,
            awayEt: etAway,
            homePens: ManualEditRules.parseScore(homePens),
            awayPens: ManualEditRules.parseScore(awayPens)
        )
    }

    private var validation: ManualEditRules.ManualEditValidation {
        ManualEditRules.validate(
            status: status,
            homeFt: homeFt,
            awayFt: awayFt,
            homeHt: homeHt,
            awayHt: awayHt,
            homeEt: homeEt,
            awayEt: awayEt,
            homePens: homePens,
            awayPens: awayPens,
            note: note
        )
    }

    private var errorMessage: String? {
        if let error = model.submitError {
            return error.userMessage
        }
        guard showValidation, !validation.isValid else { return nil }
        return validation.errors.joined(separator: "\n")
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            form
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            footer
        }
        .frame(width: 560)
        .background(AppTheme.background)
        .accessibilityIdentifier("manual.editor")
        .onAppear { model.submitError = nil }
        .onChange(of: precision) { _, newValue in
            guard newValue == KickoffPrecision.dateOnly.rawValue else { return }
            kickoff = ManualEditRules.dateOnlyPlaceholder(for: kickoff)
        }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Meccs szerkesztése")
                    .font(.appText(16, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                Text("\(detail.homeName) – \(detail.awayName)")
                    .font(.appLabel(11))
                    .foregroundStyle(AppTheme.secondaryText)
            }
            Spacer()
            Button("Bezárás") { dismiss() }
                .keyboardShortcut(.cancelAction)
                .buttonStyle(.plain)
                .foregroundStyle(AppTheme.secondaryText)
        }
        .padding(.horizontal, AppTheme.Spacing.xl)
        .padding(.vertical, AppTheme.Spacing.m)
    }

    private var form: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.l) {
            scheduleSection
            if ManualEditRules.allowsResult(for: status) {
                Rectangle().fill(AppTheme.hairline).frame(height: 1)
                resultSection
            }
            Rectangle().fill(AppTheme.hairline).frame(height: 1)
            noteSection
        }
        .padding(AppTheme.Spacing.xl)
    }

    private var scheduleSection: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.m) {
            Text("Kezdés és státusz").sectionLabel()

            HStack(spacing: AppTheme.Spacing.l) {
                Picker("Státusz", selection: $status) {
                    ForEach(ManualEditRules.statuses, id: \.self) { value in
                        Text(ManualEditRules.statusLabel(for: value)).tag(value)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("manual.status")

                Picker("Pontosság", selection: $precision) {
                    ForEach(KickoffPrecision.allCases, id: \.rawValue) { option in
                        Text(option.label).tag(option.rawValue)
                    }
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("manual.precision")
            }

            HStack(spacing: AppTheme.Spacing.l) {
                DatePicker("Dátum", selection: $kickoff, displayedComponents: [.date])
                    .environment(\.timeZone, displayTimeZone)
                    .accessibilityIdentifier("manual.kickoff.date")

                DatePicker("Idő", selection: $kickoff, displayedComponents: [.hourAndMinute])
                    .environment(\.timeZone, displayTimeZone)
                    .disabled(isDateOnly)
                    .accessibilityIdentifier("manual.kickoff.time")
            }

            if isDateOnly {
                Text("Csak dátum: a kezdés 12:00 UTC helyőrzővel kerül a rendszerbe, az idő szerkesztése letiltva.")
                    .font(.appLabel(10))
                    .foregroundStyle(AppTheme.tertiaryText)
            }
        }
    }

    private var resultSection: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.m) {
            Text("Eredmény").sectionLabel()

            scoreRow(
                "Rendes játékidő (FT)",
                home: $homeFt,
                away: $awayFt,
                homeID: "manual.home_ft",
                awayID: "manual.away_ft"
            )
            if ManualEditRules.showsHalftime(for: status) {
                scoreRow(
                    "Félidő (HT)",
                    home: $homeHt,
                    away: $awayHt,
                    homeID: "manual.home_ht",
                    awayID: "manual.away_ht"
                )
            }
            if ManualEditRules.showsExtraTime(for: status, homeFt: ftHome, awayFt: ftAway) {
                scoreRow(
                    "Hosszabbítás (ET)",
                    home: $homeEt,
                    away: $awayEt,
                    homeID: "manual.home_et",
                    awayID: "manual.away_et"
                )
            }
            if ManualEditRules.showsPenalties(
                for: status,
                homeFt: ftHome,
                awayFt: ftAway,
                homeEt: etHome,
                awayEt: etAway
            ) {
                scoreRow(
                    "Tizenegyesek (PENS)",
                    home: $homePens,
                    away: $awayPens,
                    homeID: "manual.home_pens",
                    awayID: "manual.away_pens"
                )
            }

            HStack {
                Text("Döntés módja")
                    .font(.appText(13))
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Text(ManualEditRules.decidedByLabel(currentDecidedBy))
                    .font(.appText(13, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                    .accessibilityIdentifier("manual.decided_by")
            }
            if status == "awarded" {
                Text("Megítélt meccsnél csak a rendes játékidő eredménye szerkeszthető.")
                    .font(.appLabel(10))
                    .foregroundStyle(AppTheme.tertiaryText)
            }
        }
    }

    private func scoreRow(
        _ title: String,
        home: Binding<String>,
        away: Binding<String>,
        homeID: String,
        awayID: String
    ) -> some View {
        HStack(spacing: AppTheme.Spacing.s) {
            Text(title)
                .font(.appText(13))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 190, alignment: .leading)
            TextField("", text: home)
                .textFieldStyle(.roundedBorder)
                .multilineTextAlignment(.center)
                .frame(width: 72)
                .accessibilityLabel("\(title) – hazai")
                .accessibilityIdentifier(homeID)
            Text("–")
                .font(.appText(13))
                .foregroundStyle(AppTheme.tertiaryText)
            TextField("", text: away)
                .textFieldStyle(.roundedBorder)
                .multilineTextAlignment(.center)
                .frame(width: 72)
                .accessibilityLabel("\(title) – vendég")
                .accessibilityIdentifier(awayID)
            Spacer(minLength: 0)
        }
    }

    private var noteSection: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("Indoklás").sectionLabel()
            TextField("Miért módosítod a meccset? (kötelező)", text: $note, axis: .vertical)
                .lineLimit(2...4)
                .textFieldStyle(.roundedBorder)
                .accessibilityLabel("Indoklás (kötelező)")
                .accessibilityIdentifier("manual.note")
        }
    }

    private var footer: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            if let message = errorMessage {
                Text(message)
                    .font(.appLabel(11))
                    .foregroundStyle(AppTheme.warning)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("manual.error")
            }
            HStack {
                Button("Mégse") { dismiss() }
                    .buttonStyle(.plain)
                    .foregroundStyle(AppTheme.secondaryText)
                Spacer()
                Button(model.isSubmitting ? "Mentés…" : "Mentés") { save() }
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .disabled(model.isSubmitting)
                    .accessibilityIdentifier("manual.submit")
            }
        }
        .padding(.horizontal, AppTheme.Spacing.xl)
        .padding(.vertical, AppTheme.Spacing.m)
    }

    private func save() {
        showValidation = true
        guard validation.isValid else { return }
        let request = ManualEditRules.makeRequest(
            status: status,
            kickoffAt: kickoff,
            kickoffPrecision: precision,
            homeFt: homeFt,
            awayFt: awayFt,
            homeHt: homeHt,
            awayHt: awayHt,
            homeEt: homeEt,
            awayEt: awayEt,
            homePens: homePens,
            awayPens: awayPens,
            note: note
        )
        Task {
            await model.submitEdit(matchId: detail.id, request: request)
            if model.submitError == nil { dismiss() }
        }
    }
}

struct MatchDetailContent: View {
    let detail: MatchDetail
    let logos: LogoStore

    private static let kickoff: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy. MMMM d., EEEE HH:mm"
        return formatter
    }()

    private var isLive: Bool {
        ["live", "halftime"].contains(detail.status)
    }

    private var scoreText: String {
        guard let home = detail.homeFt ?? detail.homeEt, let away = detail.awayFt ?? detail.awayEt else {
            return "–"
        }
        return "\(home) – \(away)"
    }

    private var scoreNote: String? {
        switch detail.decidedBy {
        case "et": return "hosszabbítás után"
        case "pens": return "büntetők után"
        default: return nil
        }
    }

    private var hasStats: Bool {
        let counters: [Int?] = [
            detail.homeShots, detail.awayShots, detail.homeShotsOnTarget,
            detail.awayShotsOnTarget, detail.homeCorners, detail.awayCorners,
            detail.homeYellow, detail.awayYellow, detail.homeRed, detail.awayRed,
            detail.homeFouls, detail.awayFouls,
        ]
        return counters.contains { $0 != nil } || detail.homeXg != nil || detail.awayXg != nil
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: AppTheme.Spacing.l) {
                ScoreboardHero(
                    home: detail.homeName,
                    away: detail.awayName,
                    homeLogo: detail.homeLogo,
                    awayLogo: detail.awayLogo,
                    homeElo: detail.homeElo,
                    awayElo: detail.awayElo,
                    homePosition: detail.homePosition,
                    awayPosition: detail.awayPosition,
                    score: scoreText,
                    note: scoreNote,
                    statusLabel: MatchRowView.statusLabel(for: detail.status),
                    isLive: isLive,
                    gateState: detail.gateState,
                    logos: logos
                )

                metaGrid

                if detail.homeHt != nil || detail.homeEt != nil || detail.homePens != nil {
                    periodChips
                }

                if !(detail.homeForm ?? []).isEmpty || !(detail.awayForm ?? []).isEmpty {
                    FormStrip(
                        homeName: detail.homeName,
                        awayName: detail.awayName,
                        homeForm: detail.homeForm ?? [],
                        awayForm: detail.awayForm ?? []
                    )
                    .accessibilityIdentifier("match.detail.form")
                }

                if let headToHead = detail.headToHead, !headToHead.isEmpty {
                    HeadToHeadList(entries: headToHead)
                        .accessibilityIdentifier("match.detail.h2h")
                }

                if let tableSlice = detail.tableSlice, !tableSlice.isEmpty {
                    StandingsSlice(
                        homeName: detail.homeName,
                        awayName: detail.awayName,
                        rows: tableSlice,
                        store: logos
                    )
                    .accessibilityIdentifier("match.detail.table")
                }

                if !(detail.homeNext ?? []).isEmpty || !(detail.awayNext ?? []).isEmpty {
                    NextMatchesStrip(
                        homeName: detail.homeName,
                        awayName: detail.awayName,
                        homeNext: detail.homeNext ?? [],
                        awayNext: detail.awayNext ?? []
                    )
                    .accessibilityIdentifier("match.detail.next")
                }

                if hasStats {
                    StatsDuel(
                        homeName: detail.homeName,
                        awayName: detail.awayName,
                        rows: statRows
                    )
                    .accessibilityIdentifier("match.detail.stats")
                }

                if let odds = detail.odds {
                    OddsMatrix(
                        home: odds.home,
                        draw: odds.draw,
                        away: odds.away,
                        bttsYes: odds.bttsYes,
                        bttsNo: odds.bttsNo,
                        over25: odds.over25,
                        under25: odds.under25,
                        source: detail.oddsSource,
                        capturedAt: detail.oddsCapturedAt
                    )
                    .accessibilityIdentifier("match.detail.odds")
                }

                VerificationStrip(
                    gateState: detail.gateState,
                    ruleSetVersion: detail.gateRuleSetVersion,
                    evaluatedAt: detail.gateEvaluatedAt,
                    blockers: detail.gateBlockers
                )
                .accessibilityIdentifier("match.detail.verification")
            }
            .padding(AppTheme.Spacing.xl)
            .frame(maxWidth: 860, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
    }

    private var metaGrid: some View {
        VStack(spacing: 0) {
            gridRow("Verseny", "\(detail.competitionName) · \(detail.seasonLabel)")
            gridRow("Időpont", Self.kickoff.string(from: detail.kickoffAt) +
                    (detail.kickoffPrecision == "date_only" ? " (csak dátum)" : ""))
            if let round = detail.roundLabel {
                gridRow("Forduló", round)
            }
            gridRow("Játékvezető", detail.referee ?? "–")
            if let source = detail.statsSource {
                gridRow("Statisztika forrása", source)
            }
        }
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func gridRow(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: AppTheme.Spacing.m) {
            Text(label)
                .font(.appLabel(9))
                .tracking(1.1)
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 150, alignment: .leading)
            Text(value)
                .font(.appText(12.5))
                .foregroundStyle(AppTheme.primaryText)
                .frame(maxWidth: .infinity, alignment: .leading)
            Divider().overlay(AppTheme.hairline)
        }
        .padding(.horizontal, AppTheme.Spacing.m)
        .padding(.vertical, 8)
    }

    private var periodChips: some View {
        HStack(spacing: AppTheme.Spacing.s) {
            if let hh = detail.homeHt, let ah = detail.awayHt {
                chip("Félidő", "\(hh)–\(ah)")
            }
            if let he = detail.homeEt, let ae = detail.awayEt {
                chip("120'", "\(he)–\(ae)")
            }
            if let hp = detail.homePens, let ap = detail.awayPens {
                chip("Büntetők", "\(hp)–\(ap)")
            }
            if detail.homeFt != nil {
                chip("90'", scoreText)
            }
            Spacer()
        }
    }

    private func chip(_ label: String, _ value: String) -> some View {
        VStack(spacing: 0) {
            Text(value).font(.scoreboard(16, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
            Text(label).sectionLabel()
        }
        .padding(.horizontal, AppTheme.Spacing.m)
        .padding(.vertical, 7)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.chip, style: .continuous))
    }

    private var statRows: [(label: String, home: Double?, away: Double?, format: String)] {
        var rows: [(String, Double?, Double?, String)] = [
            ("Kapura lövés", detail.homeShotsOnTarget.map(Double.init), detail.awayShotsOnTarget.map(Double.init), "%d"),
            ("Lövés", detail.homeShots.map(Double.init), detail.awayShots.map(Double.init), "%d"),
            ("Szöglet", detail.homeCorners.map(Double.init), detail.awayCorners.map(Double.init), "%d"),
            ("Szabálytalanság", detail.homeFouls.map(Double.init), detail.awayFouls.map(Double.init), "%d"),
            ("Sárga", detail.homeYellow.map(Double.init), detail.awayYellow.map(Double.init), "%d"),
            ("Piros", detail.homeRed.map(Double.init), detail.awayRed.map(Double.init), "%d"),
        ]
        if detail.homeXg != nil || detail.awayXg != nil {
            rows.append(("xG", detail.homeXg, detail.awayXg, "%.2f"))
        }
        return rows.filter { $0.1 != nil || $0.2 != nil }
    }
}


/// Utolsó öt meccs W/D/L chipjei csapatonként; minden chip az ellenfelet és
/// az eredményt is kimondja az akadálymentes felolvasásban.

struct StandingsSlice: View {
    let homeName: String
    let awayName: String
    let rows: [CompetitionStanding]
    let store: LogoStore

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("TABELLA · KÖRNYEZET")
                .font(.appLabel(9))
                .tracking(1.4)
                .foregroundStyle(AppTheme.secondaryText)
            header
            ForEach(rows) { row in
                rowView(row: row)
            }
        }
        .padding(AppTheme.Spacing.m)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text("#").frame(width: 22, alignment: .leading)
            Text("CSAPAT").frame(maxWidth: .infinity, alignment: .leading)
            Text("M").frame(width: 22, alignment: .trailing)
            Text("GY").frame(width: 22, alignment: .trailing)
            Text("D").frame(width: 22, alignment: .trailing)
            Text("V").frame(width: 22, alignment: .trailing)
            Text("GK").frame(width: 30, alignment: .trailing)
            Text("P").frame(width: 26, alignment: .trailing)
        }
        .font(.appLabel(8))
        .tracking(1.1)
        .foregroundStyle(AppTheme.tertiaryText)
    }

    private func rowView(row: CompetitionStanding) -> some View {
        let isHome = row.name == homeName
        let isAway = row.name == awayName
        return HStack(spacing: 8) {
            Text("\(row.position).")
                .font(.appText(11, weight: .semibold))
                .foregroundStyle(isHome || isAway ? AppTheme.accent : AppTheme.secondaryText)
                .frame(width: 22, alignment: .leading)
            HStack(spacing: 6) {
                CrestLogo(name: row.name, path: row.logo, store: store, size: 14)
                Text(row.name)
                    .font(.appText(11, weight: isHome || isAway ? .semibold : .regular))
                    .foregroundStyle(AppTheme.primaryText)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            stat("M", row.played)
            stat("GY", row.wins)
            stat("D", row.draws)
            stat("V", row.losses)
            Text(row.goalDiff > 0 ? "+\(row.goalDiff)" : "\(row.goalDiff)")
                .font(.appText(11))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 30, alignment: .trailing)
            Text("\(row.points)")
                .font(.appText(12, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .frame(width: 26, alignment: .trailing)
        }
        .padding(.vertical, 3)
        .padding(.horizontal, 6)
        .background(
            (isHome || isAway) ? AppTheme.surfaceElevated : Color.clear,
            in: RoundedRectangle(cornerRadius: AppTheme.Radius.chip, style: .continuous)
        )
    }

    private func stat(_ label: String, _ value: Int) -> some View {
        Text("\(value)")
            .font(.appText(11))
            .foregroundStyle(AppTheme.secondaryText)
            .frame(width: 22, alignment: .trailing)
            .accessibilityLabel("\(label) \(value)")
    }
}

struct FormStrip: View {
    let homeName: String
    let awayName: String
    let homeForm: [FormEntry]
    let awayForm: [FormEntry]

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("FORMA · UTOLSÓ 5 MECCS")
                .font(.appLabel(9))
                .tracking(1.4)
                .foregroundStyle(AppTheme.secondaryText)
            row(name: homeName, entries: homeForm)
            row(name: awayName, entries: awayForm)
        }
        .padding(AppTheme.Spacing.m)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func row(name: String, entries: [FormEntry]) -> some View {
        HStack(spacing: AppTheme.Spacing.s) {
            Text(name)
                .font(.appText(12, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .lineLimit(1)
                .frame(width: 190, alignment: .leading)
            if entries.isEmpty {
                Text("nincs korábbi lejátszott meccs")
                    .font(.appText(11))
                    .foregroundStyle(AppTheme.tertiaryText)
            } else {
                ForEach(entries) { entry in
                    Text(entry.outcome)
                        .font(.appText(11, weight: .bold))
                        .foregroundStyle(.black)
                        .frame(width: 20, height: 20)
                        .background(chipColor(entry.outcome), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
                        .help("\(entry.isHome ? "hazai" : "vendég") \(entry.opponentName) ellen \(entry.teamGoals)–\(entry.opponentGoals)")
                        .accessibilityLabel(outcomeLabel(entry))
                }
            }
            Spacer()
        }
    }

    private func chipColor(_ outcome: String) -> Color {
        switch outcome {
        case "W": return AppTheme.predictionHome
        case "L": return AppTheme.warning
        default: return AppTheme.predictionDraw
        }
    }

    private func outcomeLabel(_ entry: FormEntry) -> String {
        let result = entry.outcome == "W" ? "győzelem" : (entry.outcome == "L" ? "vereség" : "döntetlen")
        return "\(entry.isHome ? "Hazai" : "Vendég") \(entry.opponentName) ellen \(entry.teamGoals)–\(entry.opponentGoals), \(result)"
    }
}

/// Korábbi egymás elleni meccsek tömör listája.
struct HeadToHeadList: View {
    let entries: [HeadToHeadEntry]

    private static let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy. MM. dd."
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("EGYMÁS ELLEN")
                .font(.appLabel(9))
                .tracking(1.4)
                .foregroundStyle(AppTheme.secondaryText)
            ForEach(entries) { entry in
                HStack(spacing: AppTheme.Spacing.m) {
                    Text(Self.dayFormatter.string(from: entry.kickoffAt))
                        .font(.appText(11))
                        .foregroundStyle(AppTheme.tertiaryText)
                        .frame(width: 96, alignment: .leading)
                    Text("\(entry.homeName) – \(entry.awayName)")
                        .font(.appText(12))
                        .foregroundStyle(AppTheme.primaryText)
                        .lineLimit(1)
                    Spacer()
                    Text(scoreText(entry))
                        .font(.scoreboard(13, weight: .semibold))
                        .foregroundStyle(AppTheme.primaryText)
                    Text(entry.competitionName)
                        .font(.appLabel(9))
                        .foregroundStyle(AppTheme.tertiaryText)
                        .lineLimit(1)
                        .frame(width: 150, alignment: .trailing)
                }
                .padding(.horizontal, AppTheme.Spacing.m)
                .padding(.vertical, 6)
                .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: AppTheme.Radius.chip, style: .continuous))
            }
        }
        .padding(AppTheme.Spacing.m)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func scoreText(_ entry: HeadToHeadEntry) -> String {
        guard let home = entry.homeGoals, let away = entry.awayGoals else { return "–" }
        return "\(home)–\(away)"
    }
}


/// A két csapat következő meccsei: ki ellen, hol és mikor.
struct NextMatchesStrip: View {
    let homeName: String
    let awayName: String
    let homeNext: [NextMatchEntry]
    let awayNext: [NextMatchEntry]

    private static let formatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "MM. dd. HH:mm"
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("KÖVETKEZŐ MECCS")
                .font(.appLabel(9))
                .tracking(1.4)
                .foregroundStyle(AppTheme.secondaryText)
            row(name: homeName, entries: homeNext)
            row(name: awayName, entries: awayNext)
        }
        .padding(AppTheme.Spacing.m)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: AppTheme.Radius.panel, style: .continuous))
    }

    private func row(name: String, entries: [NextMatchEntry]) -> some View {
        HStack(spacing: AppTheme.Spacing.s) {
            Text(name)
                .font(.appText(12, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
                .lineLimit(1)
                .frame(width: 190, alignment: .leading)
            if entries.isEmpty {
                Text("nincs betárazott következő meccs")
                    .font(.appText(11))
                    .foregroundStyle(AppTheme.tertiaryText)
            } else {
                ForEach(entries) { entry in
                    HStack(spacing: 5) {
                        Text(Self.formatter.string(from: entry.kickoffAt))
                            .font(.appText(10))
                            .foregroundStyle(AppTheme.tertiaryText)
                        Text("\(entry.opponentName)")
                            .font(.appText(11, weight: .medium))
                            .foregroundStyle(AppTheme.primaryText)
                            .lineLimit(1)
                        Text(entry.isHome ? "H" : "V")
                            .font(.appLabel(8))
                            .foregroundStyle(AppTheme.secondaryText)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(AppTheme.surfaceElevated, in: RoundedRectangle(cornerRadius: AppTheme.Radius.chip, style: .continuous))
                    .help(entry.competitionName)
                }
            }
            Spacer()
        }
    }
}
