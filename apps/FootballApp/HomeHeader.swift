import SwiftUI
import FootballAppCore

/// A főoldal dinamikus fejléce.
///
/// A napszak és az élő időjárás együtt határozzák meg a színpalettát, az ikont
/// és a háttér-animációt (`HeaderAtmosphere`). A korábbi szöveges meta-sor
/// helyett minden tény ikonos téglában jelenik meg; alatta már csak a kereső
/// és a szűrők maradnak. A meglévő `home.stat.*` azonosítók megmaradnak, hogy
/// az UI-tesztek és a VoiceOver-címkék stabilak legyenek.
struct HomeHeader: View {
    @Binding var date: Date
    let count: Int?
    let liveCount: Int
    let oddsCount: Int
    let compact: Bool
    let refreshedAt: Date?
    /// A következő tényleges adatfrissítés a backendtől (research napi slot).
    let nextRefreshAt: Date?
    /// A legutóbbi csendes háttérfrissítés hibája; a megjelenített adat ekkor
    /// a korábbi pillanatkép, ezért külön jelzés kell róla (S167).
    let refreshFailed: Bool
    let appeared: Bool
    let focused: AppFocus?
    let weather: WeatherModel
    @Binding var showCalendar: Bool
    let onPrevious: () -> Void
    let onToday: () -> Void
    let onNext: () -> Void
    let onRefresh: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.controlActiveState) private var controlActiveState
    @State private var pulse = false

    private static let bigDate: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "MMMM d."
        return formatter
    }()
    private static let year: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy"
        return formatter
    }()
    private static let weekday: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "EEEE"
        return formatter
    }()
    private static let refreshTime: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()

    private var relativeLabel: String? {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) { return "MA" }
        if calendar.isDateInYesterday(date) { return "TEGNAP" }
        if calendar.isDateInTomorrow(date) { return "HOLNAP" }
        return nil
    }

    private var metaText: String {
        "\(Self.year.string(from: date)) · \(Self.weekday.string(from: date))"
    }

    private func step(_ delay: Double) -> Animation? {
        reduceMotion ? nil : .easeOut(duration: 0.45).delay(delay)
    }

    var body: some View {
        //Másodpercenként újraértékeljük a napszakot és a visszaszámlálót; a
        //részecske-animáció saját TimelineView-ja ettől függetlenül fut.
        TimelineView(.periodic(from: .now, by: 1)) { timeline in
            let now = timeline.date
            let phase = DayPhase.phase(at: now)
            let palette = HeaderMood.resolve(phase: phase, weather: weather.current)
            let refresh = refreshInfo(now: now)
            header(phase: phase, palette: palette, refresh: refresh, reduceMotion: reduceMotion)
        }
    }

    private func header(
        phase: DayPhase,
        palette: HeaderPalette,
        refresh: RefreshInfo,
        reduceMotion: Bool
    ) -> some View {
        ZStack(alignment: .leading) {
            HeaderAtmosphere(palette: palette, reduceMotion: reduceMotion)

            VStack(alignment: .leading, spacing: 2) {
                if compact {
                    HStack(spacing: 8) {
                        compactDateBlock(phase: phase, palette: palette)
                        Spacer(minLength: 8)
                        dateControls(palette: palette)
                    }
                    statsRow(palette: palette, refresh: refresh)
                } else {
                    HStack(alignment: .center, spacing: 10) {
                        dateBlock(phase: phase, palette: palette)
                        Spacer(minLength: 8)
                        dateControls(palette: palette)
                        statsRow(palette: palette, refresh: refresh)
                    }
                }
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 2)
        }
        .background {
            LinearGradient(
                colors: [Color(hex: palette.topHex), Color(hex: palette.bottomHex)],
                startPoint: .topLeading, endPoint: .bottomTrailing
            )
            .animation(reduceMotion ? nil : .easeInOut(duration: 0.8), value: palette)
        }
        .overlay(RoundedRectangle(cornerRadius: 24, style: .continuous).strokeBorder(AppTheme.hairline))
        .clipShape(RoundedRectangle(cornerRadius: 24, style: .continuous))
        .fixedSize(horizontal: false, vertical: true)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("home.header")
        .padding(.horizontal, 48)
        .padding(.top, 2)
        .padding(.bottom, 2)
    }

    // MARK: - Dátum blokk

    private func dateBlock(phase: DayPhase, palette: HeaderPalette) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            HStack(alignment: .center, spacing: 6) {
                Image(systemName: palette.symbolName)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Color(hex: palette.accentHex))
                Text(phase.greeting)
                    .font(.appText(11, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                if let relativeLabel {
                    Text(relativeLabel)
                        .font(.system(size: 8, weight: .bold))
                        .tracking(1)
                        .foregroundStyle(AppTheme.background)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(AppTheme.accent, in: Capsule())
                }
            }
            .opacity(appeared ? 1 : 0)
            .offset(y: appeared ? 0 : 4)
            .animation(step(0.04), value: appeared)

            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(Self.bigDate.string(from: date))
                    .font(.scoreboard(24))
                    .tracking(-0.6)
                    .lineLimit(1)
                    .minimumScaleFactor(0.55)
                    .foregroundStyle(AppTheme.primaryText)
                Text(metaText)
                    .font(.appText(9))
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(1)
            }
            .opacity(appeared ? 1 : 0)
            .offset(y: appeared ? 0 : 5)
            .animation(step(0.1), value: appeared)
        }
    }

    private func compactDateBlock(phase: DayPhase, palette: HeaderPalette) -> some View {
        HStack(spacing: 6) {
            Image(systemName: palette.symbolName)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Color(hex: palette.accentHex))
            Text(phase.greeting)
                .font(.appText(11, weight: .semibold))
                .foregroundStyle(AppTheme.primaryText)
            if let relativeLabel {
                Text(relativeLabel)
                    .font(.system(size: 8, weight: .bold))
                    .tracking(1)
                    .foregroundStyle(AppTheme.background)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(AppTheme.accent, in: Capsule())
            }
            Text(Self.bigDate.string(from: date))
                .font(.scoreboard(20))
                .tracking(-0.4)
                .foregroundStyle(AppTheme.primaryText)
            Text(metaText)
                .font(.appText(9))
                .foregroundStyle(AppTheme.secondaryText)
        }
        .lineLimit(1)
        .minimumScaleFactor(0.65)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("home.header.compactDate")
        .opacity(appeared ? 1 : 0)
        .animation(step(0.08), value: appeared)
    }

    // MARK: - Dátumvezérlők

    private func dateControls(palette: HeaderPalette) -> some View {
        HStack(spacing: 6) {
            iconButton("chevron.left", help: "Előző nap", tint: palette,
                       focus: .previousDay, action: onPrevious)
                .keyboardShortcut(.leftArrow, modifiers: .command)
            Button(action: onToday) {
                Text("Ma")
                    .font(.system(size: 11.5, weight: .medium))
                    .foregroundStyle(AppTheme.primaryText)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(AppTheme.accentSoft, in: Capsule())
                    .overlay(Capsule().strokeBorder(AppTheme.accent.opacity(0.25)))
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Ma")
            .accessibilityIdentifier("nav.today")
            .focusAccessibility(focused == .today)
            .capsuleFocusRing(focused == .today)
            .keyboardShortcut("t", modifiers: .command)
            iconButton("chevron.right", help: "Következő nap", tint: palette,
                       focus: .nextDay, action: onNext)
                .keyboardShortcut(.rightArrow, modifiers: .command)

            Rectangle().fill(AppTheme.hairline).frame(width: 1, height: 16).padding(.horizontal, 2)

            Button {
                showCalendar = true
            } label: {
                Image(systemName: "calendar")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(AppTheme.secondaryText)
                    .frame(width: 28, height: 28)
                    .background(AppTheme.surface, in: Circle())
                    .overlay(Circle().strokeBorder(AppTheme.hairline))
            }
            .buttonStyle(.plain)
            .help("Naptár")
            .accessibilityLabel("Naptár")
            .accessibilityIdentifier("nav.calendar")
            .focusAccessibility(focused == .calendar)
            .circleFocusRing(focused == .calendar)
            .popover(isPresented: $showCalendar, arrowEdge: .bottom) {
                MonthCalendar(date: $date, onSelect: { showCalendar = false })
            }

            iconButton("arrow.clockwise", help: "Frissítés", tint: palette,
                       focus: .refresh, action: onRefresh)
                .keyboardShortcut("r", modifiers: .command)
        }
        .padding(2)
        .background(Color.black.opacity(0.32), in: Capsule())
        .overlay(Capsule().strokeBorder(AppTheme.hairline))
        .opacity(appeared ? 1 : 0)
        .animation(step(0.16), value: appeared)
    }

    // MARK: - Ikonos összefoglaló téglák

    private func statsRow(palette: HeaderPalette, refresh: RefreshInfo) -> some View {
        let tiles = HStack(spacing: 6) {
            statTile(
                icon: "sportscourt",
                value: count.map { "\($0)" } ?? "–",
                label: "mérkőzés",
                tint: AppTheme.primaryText,
                identifier: "home.stat.matches"
            )
            statTile(
                icon: "dot.radiowaves.left.and.right",
                value: "\(liveCount)",
                label: "élő",
                tint: liveCount > 0 ? AppTheme.liveGreen : AppTheme.secondaryText,
                identifier: "home.stat.live",
                pulsing: liveCount > 0
            )
            statTile(
                icon: "chart.bar.fill",
                value: "\(oddsCount)",
                label: "oddsadattal",
                tint: Color(hex: palette.accentHex),
                identifier: "home.stat.odds"
            )
            weatherTile(palette: palette)
            countdownTile(info: refresh, palette: palette)
            refreshFailedTile()
        }
        return Group {
            if compact {
                ScrollView(.horizontal, showsIndicators: false) {
                    tiles.fixedSize(horizontal: true, vertical: false)
                }
            } else {
                tiles
            }
        }
        .opacity(appeared ? 1 : 0)
        .offset(y: appeared ? 0 : 8)
        .animation(step(0.24), value: appeared)
    }

    private func statTile(
        icon: String,
        value: String,
        label: String,
        tint: Color,
        identifier: String,
        pulsing: Bool = false
    ) -> some View {
        HStack(spacing: 5) {
            ZStack {
                if pulsing {
                    Circle()
                        .stroke(tint.opacity(0.55), lineWidth: 1.5)
                        .frame(width: 20, height: 20)
                        .scaleEffect(pulse ? 1.5 : 1)
                        .opacity(pulse ? 0 : 0.9)
                }
                Image(systemName: icon)
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(tint)
            }
            .frame(width: 22, height: 22)
            .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            VStack(alignment: .leading, spacing: 0) {
                Text(value)
                    .font(.scoreboard(14))
                    .foregroundStyle(AppTheme.primaryText)
                Text(label)
                    .font(.appLabel(8))
                    .foregroundStyle(AppTheme.secondaryText)
            }
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(tileBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(AppTheme.hairline))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(value) \(label)")
        .accessibilityIdentifier(identifier)
        .onAppear {
            // Csak aktív ablaknál indítjuk a végtelen pulzálást: a repeatForever
            // animáció különben a háttérben is folyamatos kijelzőfrissítést kér.
            guard pulsing, !reduceMotion, controlActiveState == .key else { return }
            withAnimation(.easeOut(duration: 1.6).repeatForever(autoreverses: false)) {
                pulse = true
            }
        }
    }

    private func weatherTile(palette: HeaderPalette) -> some View {
        let live = weather.isLive
        let origin = weather.usesDeviceLocation ? "saját helyzet" : "alaphelyszín"
        let freshness = weather.weatherIsStale ? "elavult adat" : "élő adat"
        let helpText = live
            ? "Időjárás: \(palette.conditionLabel), \(weather.current?.temperatureText ?? ""), \(weather.cityName) (\(origin)), \(freshness). Forrás: helyi API (Open-Meteo)."
            : "Valódi időjárás nem elérhető; a színezés a napszakot követi (\(palette.conditionLabel))."
        return HStack(spacing: 5) {
            Image(systemName: palette.symbolName)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Color(hex: palette.accentHex))
                .frame(width: 22, height: 22)
                .background(
                    Color(hex: palette.accentHex).opacity(0.12),
                    in: RoundedRectangle(cornerRadius: 7, style: .continuous)
                )
            VStack(alignment: .leading, spacing: 0) {
                Text(weather.current?.temperatureText ?? "–")
                    .font(.scoreboard(14))
                    .foregroundStyle(AppTheme.primaryText)
                Text(palette.conditionLabel)
                    .font(.appLabel(8))
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(tileBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(AppTheme.hairline))
        .help(helpText)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            live
                ? "Időjárás: \(weather.current?.temperatureText ?? ""), \(palette.conditionLabel), \(weather.cityName), \(origin), \(freshness)"
                : "Időjárási adat nem elérhető, napszaki színezés: \(palette.conditionLabel)"
        )
        .accessibilityIdentifier("home.stat.weather")
    }

    private func countdownTile(info: RefreshInfo, palette: HeaderPalette) -> some View {
        HStack(spacing: 5) {
            ZStack {
                Circle()
                    .stroke(Color.white.opacity(0.10), lineWidth: 2)
                Circle()
                    .trim(from: 0, to: max(0.02, min(1, info.progress)))
                    .stroke(
                        Color(hex: palette.accentHex),
                        style: StrokeStyle(lineWidth: 2, lineCap: .round)
                    )
                    .rotationEffect(.degrees(-90))
                Image(systemName: "arrow.triangle.2.circlepath")
                    .font(.system(size: 7, weight: .semibold))
                    .foregroundStyle(Color(hex: palette.accentHex))
            }
            .frame(width: 22, height: 22)
            VStack(alignment: .leading, spacing: 0) {
                Text(info.value)
                    .font(.scoreboard(13))
                    .monospacedDigit()
                    .foregroundStyle(AppTheme.primaryText)
                Text(info.label)
                    .font(.appLabel(8))
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(tileBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(AppTheme.hairline))
        .contentShape(Rectangle())
        .onTapGesture(perform: onRefresh)
        .help(info.help)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("Frissítés: \(info.value), \(info.label). \(info.help)")
        .accessibilityIdentifier("home.stat.refresh")
    }

    /// A csendes frissítés hibája csak akkor jelenik meg, ha van mit megjeleníteni:
    /// teljes hiba esetén az `ErrorStateView` veszi át a place-t, itt nincs mit mutatni.
    @ViewBuilder
    private func refreshFailedTile() -> some View {
        if refreshFailed {
            HStack(spacing: 4) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 9, weight: .semibold))
                Text("Frissítés nem sikerült")
                    .font(.appLabel(8))
                    .lineLimit(1)
            }
            .foregroundStyle(AppTheme.warning)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(tileBackground, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(AppTheme.warning.opacity(0.5))
            )
            .help("Az utolsó háttérfrissítés nem sikerült. A megjelenített adat a korábbi pillanatkép; a következő kör újrapróbálja.")
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Utolsó frissítés nem sikerült")
            .accessibilityIdentifier("home.stat.refreshFailed")
        }
    }

    private var tileBackground: Color {
        Color.black.opacity(0.34)
    }

    // MARK: - Visszaszámlálás

    private struct RefreshInfo {
        let value: String
        let progress: Double
        let label: String
        let help: String
    }

    private static let clientPollHint = "A kliens ezalatt 5 percenként (élő meccsnél 60 mp) csendesen újra lekérdezi az API-t; ez nem hoz új research-adatot."

    private func refreshInfo(now: Date) -> RefreshInfo {
        let anchor = refreshedAt ?? now
        // A meccsadatok a research napi slotjaiban érkeznek — a backend
        // `/v1/day` next_refresh_at mezője mondja meg a következőt. Ez mindig
        // elöl van, még élő meccsek alatt is: a 60 mp-es csendes frissítés
        // csak újra lekérdezi ugyanazt az adatot, nem hoz új research-adatot.
        if let next = nextRefreshAt {
            let cycle: TimeInterval = 2 * 3600
            let remaining = next.timeIntervalSince(now)
            if remaining <= 0 {
                return RefreshInfo(
                    value: "most",
                    progress: 1,
                    label: "frissítés esedékes",
                    help: "A forrás következő slotja esedékes. Kattints a frissítéshez."
                )
            }
            return RefreshInfo(
                value: Self.duration(remaining),
                progress: min(1, max(0, remaining / cycle)),
                label: "forrás-slot",
                help: "A következő meccsadat-frissítés \(Self.refreshTime.string(from: next)) körül várható — ez a FORRÁS következő slotja, nem a kliens frissítése. \(Self.clientPollHint)"
            )
        }
        let cycle: TimeInterval = 6 * 3600
        let next = anchor.addingTimeInterval(cycle)
        let remaining = next.timeIntervalSince(now)
        if remaining <= 0 {
            return RefreshInfo(
                value: "most",
                progress: 1,
                label: "frissítés esedékes",
                help: "A forrás következő slotja esedékes. Kattints a frissítéshez."
            )
        }
        return RefreshInfo(
            value: Self.duration(remaining),
            progress: remaining / cycle,
            label: "becsült forrás-slot",
            help: "Becsült adatfrissítés \(Self.refreshTime.string(from: next)) körül (a forrás nem adott slotot). \(Self.clientPollHint)"
        )
    }

    private static func mmss(_ seconds: TimeInterval) -> String {
        let total = max(0, Int(seconds.rounded(.up)))
        return String(format: "%02d:%02d", total / 60, total % 60)
    }

    private static func duration(_ seconds: TimeInterval) -> String {
        let total = max(0, Int(seconds))
        let hours = total / 3600
        let minutes = (total % 3600) / 60
        if hours > 0 { return "\(hours) ó \(minutes) p" }
        return mmss(TimeInterval(total))
    }

    // MARK: - Gomb

    private func iconButton(
        _ systemName: String,
        help: String,
        tint palette: HeaderPalette,
        focus target: AppFocus,
        action: @escaping () -> Void
    ) -> some View {
        let isFocused = focused == target
        return Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 26, height: 26)
                .background(Color.black.opacity(0.28), in: Circle())
                .overlay(Circle().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(help)
        .accessibilityLabel(help)
        .accessibilityIdentifier("nav.\(systemName)")
        .focusAccessibility(isFocused)
        .circleFocusRing(isFocused)
    }
}

/// Nagy, jól olvasható havi naptár (magyar, hétfő-kezdés), a dark témához.
private struct MonthCalendar: View {
    @Binding var date: Date
    let onSelect: () -> Void

    @State private var month: Date

    private static let calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.firstWeekday = 2
        calendar.locale = Locale(identifier: "hu_HU")
        return calendar
    }()
    private static let monthTitle: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "yyyy. MMMM"
        return formatter
    }()
    private static let weekdaySymbols = ["H", "K", "Sze", "Cs", "P", "Szo", "V"]

    init(date: Binding<Date>, onSelect: @escaping () -> Void) {
        self._date = date
        self.onSelect = onSelect
        let calendar = MonthCalendar.calendar
        let components = calendar.dateComponents([.year, .month], from: date.wrappedValue)
        self._month = State(initialValue: calendar.date(from: components) ?? date.wrappedValue)
    }

    private var calendar: Calendar { Self.calendar }

    var body: some View {
        VStack(spacing: 14) {
            HStack(spacing: 8) {
                monthButton("chevron.left", label: "Előző hónap") { shiftMonth(-1) }
                Text(Self.monthTitle.string(from: month))
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                    .frame(maxWidth: .infinity)
                monthButton("chevron.right", label: "Következő hónap") { shiftMonth(1) }
            }

            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 4), count: 7), spacing: 4) {
                ForEach(Self.weekdaySymbols, id: \.self) { symbol in
                    Text(symbol)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(AppTheme.secondaryText)
                        .frame(maxWidth: .infinity)
                }
                ForEach(Array(cells.enumerated()), id: \.offset) { _, day in
                    if let day {
                        dayCell(day)
                    } else {
                        Color.clear.frame(height: 36)
                    }
                }
            }

            Button {
                date = Date()
                onSelect()
            } label: {
                Text("Ma")
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(AppTheme.primaryText)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                    .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(AppTheme.hairline))
            }
            .buttonStyle(.plain)
        }
        .padding(16)
        .frame(width: 320)
    }

    private func dayCell(_ day: Int) -> some View {
        let cellDate = dateFor(day)
        let isSelected = calendar.isDate(cellDate, inSameDayAs: date)
        let isToday = calendar.isDateInToday(cellDate)
        return Button {
            date = cellDate
            onSelect()
        } label: {
            Text("\(day)")
                .font(.system(size: 14, weight: isSelected ? .bold : .regular).monospacedDigit())
                .foregroundStyle(isSelected ? AppTheme.background : AppTheme.primaryText)
                .frame(maxWidth: .infinity, minHeight: 36)
                .background(
                    isSelected ? AppTheme.accent : (isToday ? AppTheme.surfaceElevated : Color.clear),
                    in: RoundedRectangle(cornerRadius: 8, style: .continuous)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .strokeBorder(isToday && !isSelected ? AppTheme.accent.opacity(0.6) : Color.clear)
                )
        }
        .buttonStyle(.plain)
    }

    private var cells: [Int?] {
        guard let range = calendar.range(of: .day, in: .month, for: month),
              let first = calendar.date(from: calendar.dateComponents([.year, .month], from: month)) else {
            return []
        }
        let weekday = calendar.component(.weekday, from: first)
        let offset = (weekday + 5) % 7
        var result: [Int?] = Array(repeating: nil, count: offset)
        for day in range { result.append(day) }
        return result
    }

    private func dateFor(_ day: Int) -> Date {
        calendar.date(
            from: DateComponents(
                year: calendar.component(.year, from: month),
                month: calendar.component(.month, from: month),
                day: day
            )
        ) ?? month
    }

    private func shiftMonth(_ delta: Int) {
        month = calendar.date(byAdding: .month, value: delta, to: month) ?? month
    }

    private func monthButton(_ systemName: String, label: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 28, height: 28)
                .background(AppTheme.surface, in: Circle())
                .overlay(Circle().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
        .accessibilityIdentifier("calendar.\(systemName)")
    }
}
