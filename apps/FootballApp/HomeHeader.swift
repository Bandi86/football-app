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
    let appeared: Bool
    let focused: AppFocus?
    let weather: WeatherModel
    @Binding var showCalendar: Bool
    let onPrevious: () -> Void
    let onToday: () -> Void
    let onNext: () -> Void
    let onRefresh: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
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

            ZStack(alignment: .leading) {
                HeaderAtmosphere(palette: palette, reduceMotion: reduceMotion)

                VStack(alignment: .leading, spacing: 8) {
                    if compact {
                        VStack(alignment: .leading, spacing: 6) {
                            dateBlock(phase: phase, palette: palette)
                            dateControls(palette: palette)
                        }
                    } else {
                        HStack(alignment: .center, spacing: 16) {
                            dateBlock(phase: phase, palette: palette)
                            Spacer(minLength: 12)
                            dateControls(palette: palette)
                        }
                    }
                    statsRow(palette: palette, refresh: refresh)
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 8)
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
            .padding(.horizontal, 48)
            .padding(.top, 8)
            .padding(.bottom, 6)
        }
    }

    // MARK: - Dátum blokk

    private func dateBlock(phase: DayPhase, palette: HeaderPalette) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .center, spacing: 10) {
                Image(systemName: palette.symbolName)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Color(hex: palette.accentHex))
                Text(phase.greeting)
                    .font(.appText(14, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                if let relativeLabel {
                    Text(relativeLabel)
                        .font(.system(size: 9.5, weight: .bold))
                        .tracking(1.3)
                        .foregroundStyle(AppTheme.background)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(AppTheme.accent, in: Capsule())
                }
            }
            .opacity(appeared ? 1 : 0)
            .offset(y: appeared ? 0 : 8)
            .animation(step(0.04), value: appeared)

            HStack(alignment: .firstTextBaseline, spacing: 12) {
                Text(Self.bigDate.string(from: date))
                    .font(.scoreboard(32))
                    .tracking(-1.0)
                    .lineLimit(1)
                    .minimumScaleFactor(0.55)
                    .foregroundStyle(AppTheme.primaryText)
                Text(metaText)
                    .font(.appText(12))
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(1)
            }
            .opacity(appeared ? 1 : 0)
            .offset(y: appeared ? 0 : 10)
            .animation(step(0.1), value: appeared)
        }
    }

    // MARK: - Dátumvezérlők

    private func dateControls(palette: HeaderPalette) -> some View {
        HStack(spacing: 8) {
            iconButton("chevron.left", help: "Előző nap", tint: palette,
                       focus: .previousDay, action: onPrevious)
                .keyboardShortcut(.leftArrow, modifiers: .command)
            Button(action: onToday) {
                Text("Ma")
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(AppTheme.primaryText)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
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

            Rectangle().fill(AppTheme.hairline).frame(width: 1, height: 20).padding(.horizontal, 4)

            Button {
                showCalendar = true
            } label: {
                Image(systemName: "calendar")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(AppTheme.secondaryText)
                    .frame(width: 34, height: 34)
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
        .padding(4)
        .background(Color.black.opacity(0.32), in: Capsule())
        .overlay(Capsule().strokeBorder(AppTheme.hairline))
        .opacity(appeared ? 1 : 0)
        .animation(step(0.16), value: appeared)
    }

    // MARK: - Ikonos összefoglaló téglák

    private func statsRow(palette: HeaderPalette, refresh: RefreshInfo) -> some View {
        let tiles = HStack(spacing: 12) {
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
            Spacer(minLength: 0)
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
        HStack(spacing: 11) {
            ZStack {
                if pulsing {
                    Circle()
                        .stroke(tint.opacity(0.55), lineWidth: 1.5)
                        .frame(width: 24, height: 24)
                        .scaleEffect(pulse ? 1.5 : 1)
                        .opacity(pulse ? 0 : 0.9)
                }
                Image(systemName: icon)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(tint)
            }
            .frame(width: 28, height: 28)
            .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            VStack(alignment: .leading, spacing: 1) {
                Text(value)
                    .font(.scoreboard(16))
                    .foregroundStyle(AppTheme.primaryText)
                Text(label)
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.secondaryText)
            }
        }
        .padding(.horizontal, 11)
        .padding(.vertical, 8)
        .background(tileBackground, in: RoundedRectangle(cornerRadius: 13, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 13, style: .continuous).strokeBorder(AppTheme.hairline))
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(value) \(label)")
        .accessibilityIdentifier(identifier)
        .onAppear {
            guard pulsing, !reduceMotion else { return }
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
        return HStack(spacing: 11) {
            Image(systemName: palette.symbolName)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(Color(hex: palette.accentHex))
                .frame(width: 28, height: 28)
                .background(
                    Color(hex: palette.accentHex).opacity(0.12),
                    in: RoundedRectangle(cornerRadius: 9, style: .continuous)
                )
            VStack(alignment: .leading, spacing: 1) {
                Text(weather.current?.temperatureText ?? "–")
                    .font(.scoreboard(16))
                    .foregroundStyle(AppTheme.primaryText)
                Text(palette.conditionLabel)
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 11)
        .padding(.vertical, 8)
        .background(tileBackground, in: RoundedRectangle(cornerRadius: 13, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 13, style: .continuous).strokeBorder(AppTheme.hairline))
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
        HStack(spacing: 11) {
            ZStack {
                Circle()
                    .stroke(Color.white.opacity(0.10), lineWidth: 2.5)
                Circle()
                    .trim(from: 0, to: max(0.02, min(1, info.progress)))
                    .stroke(
                        Color(hex: palette.accentHex),
                        style: StrokeStyle(lineWidth: 2.5, lineCap: .round)
                    )
                    .rotationEffect(.degrees(-90))
                Image(systemName: "arrow.triangle.2.circlepath")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(Color(hex: palette.accentHex))
            }
            .frame(width: 28, height: 28)
            VStack(alignment: .leading, spacing: 1) {
                Text(info.value)
                    .font(.scoreboard(15))
                    .monospacedDigit()
                    .foregroundStyle(AppTheme.primaryText)
                Text(info.label)
                    .font(.appLabel(9))
                    .foregroundStyle(AppTheme.secondaryText)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, 11)
        .padding(.vertical, 8)
        .background(tileBackground, in: RoundedRectangle(cornerRadius: 13, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 13, style: .continuous).strokeBorder(AppTheme.hairline))
        .contentShape(Rectangle())
        .onTapGesture(perform: onRefresh)
        .help(info.help)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("Frissítés: \(info.value), \(info.label). \(info.help)")
        .accessibilityIdentifier("home.stat.refresh")
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
                    help: "A következő adatfrissítés esedékes. Kattints a frissítéshez."
                )
            }
            return RefreshInfo(
                value: Self.duration(remaining),
                progress: min(1, max(0, remaining / cycle)),
                label: "következő adatfrissítés",
                help: "A következő meccsadat-frissítés \(Self.refreshTime.string(from: next)) körül várható."
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
                help: "A következő adatfrissítés esedékes. Kattints a frissítéshez."
            )
        }
        return RefreshInfo(
            value: Self.duration(remaining),
            progress: remaining / cycle,
            label: "következő frissítés",
            help: "Várható adatfrissítés \(Self.refreshTime.string(from: next)) körül."
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
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 30, height: 30)
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
                monthButton("chevron.left") { shiftMonth(-1) }
                Text(Self.monthTitle.string(from: month))
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(AppTheme.primaryText)
                    .frame(maxWidth: .infinity)
                monthButton("chevron.right") { shiftMonth(1) }
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

    private func monthButton(_ systemName: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 28, height: 28)
                .background(AppTheme.surface, in: Circle())
                .overlay(Circle().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
    }
}
