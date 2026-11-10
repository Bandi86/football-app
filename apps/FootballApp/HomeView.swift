import SwiftUI

/// A főoldal: felső navigáció → változó HEADER → szűrő → liga szerint csoportosított
/// napi lista. Jelenleg **mintaadatból** (SampleData) dolgozik a design teszteléséhez.
struct HomeView: View {
    @State private var selectedDate = Date()
    @State private var filter: MatchFilter = .all
    @State private var country: String? = nil
    @State private var division: Int? = nil

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var appeared = false

    var body: some View {
        VStack(spacing: 0) {
            TopNavigationBar()

            Rectangle().fill(AppTheme.hairline).frame(height: 1)

            HomeHeader(
                date: $selectedDate,
                count: SampleData.totalMatches,
                appeared: appeared,
                onPrevious: { shift(-1) },
                onToday: { selectedDate = Date() },
                onNext: { shift(1) },
                onRefresh: { replayEntrance() }
            )

            FilterBar(filter: $filter, country: $country, division: $division, counts: filterCounts)

            MatchListView(leagues: filteredLeagues)
        }
        .background(AppTheme.background)
        .onAppear {
            withAnimation(reduceMotion ? nil : .easeOut(duration: 0.4)) { appeared = true }
        }
    }

    private func shift(_ delta: Int) {
        selectedDate = Calendar.current.date(byAdding: .day, value: delta, to: selectedDate) ?? selectedDate
    }

    private func replayEntrance() {
        appeared = false
        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.4)) { appeared = true }
    }

    private var baseLeagues: [SampleLeague] {
        SampleData.leagues.filter { league in
            (country == nil || league.country == country) &&
            (division == nil || league.tier == division)
        }
    }

    private var filterCounts: [MatchFilter: Int] {
        let all = baseLeagues.flatMap { $0.matches }
        var result: [MatchFilter: Int] = [:]
        for candidate in MatchFilter.allCases {
            result[candidate] = all.filter { candidate.matches(status: $0.status) }.count
        }
        return result
    }

    private var filteredLeagues: [SampleLeague] {
        baseLeagues.compactMap { $0.filtering(filter) }
    }
}

private struct TopNavigationBar: View {
    private static let clock: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "HH:mm:ss"
        return formatter
    }()

    var body: some View {
        HStack(spacing: 16) {
            HStack(spacing: 8) {
                Circle().fill(AppTheme.accent).frame(width: 9, height: 9)
                Text("FOOTBALL APP")
                    .font(.system(size: 12, weight: .semibold))
                    .tracking(3)
                    .foregroundStyle(AppTheme.primaryText)
            }

            Spacer()

            HStack(spacing: 8) {
                pill("Ma", active: true)
                pill("Meccsek")
                pill("Csapatok")
                pill("Versenyek")
            }

            Spacer()

            HStack(spacing: 14) {
                HStack(spacing: 6) {
                    Circle().fill(AppTheme.accent).frame(width: 7, height: 7)
                    Text("MINTA ADAT")
                        .font(.system(size: 11, weight: .semibold))
                        .tracking(0.5)
                        .foregroundStyle(AppTheme.accent)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(AppTheme.accent.opacity(0.12), in: Capsule())

                TimelineView(.periodic(from: .now, by: 1)) { context in
                    Text(Self.clock.string(from: context.date))
                        .font(.system(size: 12, weight: .medium).monospacedDigit())
                        .foregroundStyle(AppTheme.secondaryText)
                }
            }
        }
        .padding(.horizontal, 20)
        .frame(height: 54)
    }

    private func pill(_ title: String, active: Bool = false) -> some View {
        Text(title)
            .font(.system(size: 12.5, weight: .medium))
            .foregroundStyle(active ? AppTheme.primaryText : AppTheme.secondaryText.opacity(0.45))
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(active ? AppTheme.surface : Color.clear, in: Capsule())
    }
}

private struct HomeHeader: View {
    @Binding var date: Date
    let count: Int?
    let appeared: Bool
    let onPrevious: () -> Void
    let onToday: () -> Void
    let onNext: () -> Void
    let onRefresh: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var showCalendar = false

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

    private var greeting: String {
        switch Calendar.current.component(.hour, from: Date()) {
        case 5..<10: return "Jó reggelt."
        case 10..<18: return "Jó napot."
        case 18..<22: return "Jó estét."
        default: return "Jó éjszakát."
        }
    }

    private var relativeLabel: String? {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) { return "MA" }
        if calendar.isDateInYesterday(date) { return "TEGNAP" }
        if calendar.isDateInTomorrow(date) { return "HOLNAP" }
        return nil
    }

    private var metaText: String {
        let base = "\(Self.year.string(from: date)) · \(Self.weekday.string(from: date))"
        if let count { return "\(base) · \(count) meccs" }
        return base
    }

    private func step(_ delay: Double) -> Animation? {
        reduceMotion ? nil : .easeOut(duration: 0.45).delay(delay)
    }

    var body: some View {
        HStack(alignment: .bottom, spacing: 20) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 10) {
                    Text(greeting)
                        .font(.system(size: 20))
                        .foregroundStyle(AppTheme.secondaryText)
                    if let relativeLabel {
                        Text(relativeLabel)
                            .font(.system(size: 10.5, weight: .bold))
                            .tracking(1.5)
                            .foregroundStyle(AppTheme.accent)
                            .padding(.horizontal, 7)
                            .padding(.vertical, 2)
                            .background(AppTheme.accent.opacity(0.15), in: Capsule())
                    }
                }
                .opacity(appeared ? 1 : 0)
                .offset(y: appeared ? 0 : 8)
                .animation(step(0.04), value: appeared)

                Text(Self.bigDate.string(from: date))
                    .font(.system(size: 60, weight: .bold))
                    .tracking(-2)
                    .lineLimit(1)
                    .minimumScaleFactor(0.55)
                    .foregroundStyle(AppTheme.primaryText)
                    .opacity(appeared ? 1 : 0)
                    .offset(y: appeared ? 0 : 12)
                    .animation(step(0.1), value: appeared)

                Text(metaText)
                    .font(.system(size: 14))
                    .foregroundStyle(AppTheme.secondaryText)
                    .opacity(appeared ? 1 : 0)
                    .offset(y: appeared ? 0 : 12)
                    .animation(step(0.16), value: appeared)
            }

            Spacer()

            HStack(spacing: 8) {
                iconButton("chevron.left", help: "Előző nap", action: onPrevious)
                Button(action: onToday) {
                    Text("Ma")
                        .font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(AppTheme.primaryText)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 7)
                        .background(AppTheme.surface, in: Capsule())
                        .overlay(Capsule().strokeBorder(AppTheme.hairline))
                }
                .buttonStyle(.plain)
                iconButton("chevron.right", help: "Következő nap", action: onNext)

                Rectangle().fill(AppTheme.hairline).frame(width: 1, height: 18).padding(.horizontal, 2)

                Button {
                    showCalendar = true
                } label: {
                    Image(systemName: "calendar")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(AppTheme.secondaryText)
                        .frame(width: 30, height: 30)
                        .background(AppTheme.surface, in: Circle())
                        .overlay(Circle().strokeBorder(AppTheme.hairline))
                }
                .buttonStyle(.plain)
                .help("Naptár")
                .popover(isPresented: $showCalendar, arrowEdge: .bottom) {
                    MonthCalendar(date: $date, onSelect: { showCalendar = false })
                }

                iconButton("arrow.clockwise", help: "Frissítés", action: onRefresh)
            }
            .opacity(appeared ? 1 : 0)
            .animation(step(0.16), value: appeared)
        }
        .padding(.horizontal, 48)
        .padding(.top, 22)
        .padding(.bottom, 16)
    }

    private func iconButton(_ systemName: String, help: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
                .frame(width: 30, height: 30)
                .background(AppTheme.surface, in: Circle())
                .overlay(Circle().strokeBorder(AppTheme.hairline))
        }
        .buttonStyle(.plain)
        .help(help)
    }
}

private struct FilterBar: View {
    @Binding var filter: MatchFilter
    @Binding var country: String?
    @Binding var division: Int?
    let counts: [MatchFilter: Int]

    var body: some View {
        HStack(spacing: 8) {
            ForEach(MatchFilter.allCases) { candidate in
                FilterChip(
                    filter: candidate,
                    count: counts[candidate] ?? 0,
                    isActive: filter == candidate
                ) {
                    filter = candidate
                }
            }

            Spacer()

            Menu {
                Button("Minden ország") { country = nil }
                Divider()
                ForEach(SampleData.countries, id: \.self) { name in
                    Button(name) { country = name }
                }
            } label: {
                menuLabel(country ?? "Minden ország")
            }
            .menuStyle(.borderlessButton)
            .fixedSize()

            Menu {
                Button("Minden osztály") { division = nil }
                Divider()
                Button("1. osztály") { division = 1 }
                Button("2. osztály") { division = 2 }
            } label: {
                menuLabel(division.map { $0 == 1 ? "1. osztály" : "2. osztály" } ?? "Minden osztály")
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
        }
        .padding(.horizontal, 48)
        .padding(.bottom, 6)
    }

    private func menuLabel(_ text: String) -> some View {
        HStack(spacing: 6) {
            Text(text)
                .font(.system(size: 12.5, weight: .medium))
                .foregroundStyle(AppTheme.secondaryText)
            Image(systemName: "chevron.down")
                .font(.system(size: 9, weight: .semibold))
                .foregroundStyle(AppTheme.secondaryText)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(AppTheme.surface, in: Capsule())
        .overlay(Capsule().strokeBorder(AppTheme.hairline))
    }
}

private struct FilterChip: View {
    let filter: MatchFilter
    let count: Int
    let isActive: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(filter.title)
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(isActive ? AppTheme.primaryText : AppTheme.secondaryText)
                Text("\(count)")
                    .font(.system(size: 11, weight: .semibold).monospacedDigit())
                    .foregroundStyle(isActive ? AppTheme.background : AppTheme.secondaryText)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 1)
                    .background(isActive ? AppTheme.accent : AppTheme.surface, in: Capsule())
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(isActive ? AppTheme.surfaceElevated : Color.clear, in: Capsule())
            .overlay(Capsule().strokeBorder(isActive ? AppTheme.hairline : .clear))
        }
        .buttonStyle(.plain)
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
