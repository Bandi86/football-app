import SwiftUI

/// Az app-szintű Tab-sorrend egy állomása.
///
/// Miért saját sorrend: a macOS Tab-fókusz a rendszer „teljes billentyűzet-
/// vezérlés” beállításától függ, a SwiftUI beépített fókuszlánca pedig nem ad
/// determinisztikus, tesztelhető sorrendet. Ezért az app maga tartja nyilván,
/// mi következik Tabbal, és a fókuszált vezérlő saját borostyán gyűrűt rajzol
/// (`focusRing`), hogy a fókusz a rendszerbeállítástól függetlenül látható és
/// mérhető legyen.
enum AppFocus: Hashable {
    case tab(AppTab)
    case previousDay
    case today
    case nextDay
    case calendar
    case refresh
    case filter(MatchFilter)
    case featured
    case match(String)
}

extension AppFocus {
    /// A látható állomások sorrendje. A fejléc és a szűrők csak a nap-nézetben
    /// léteznek; a többi fülön a felső navigáció a teljes sorrend.
    static func order(tab: AppTab, matchIDs: [String]) -> [AppFocus] {
        var order: [AppFocus] = [.tab(.day), .tab(.competitions), .tab(.teams), .tab(.ops)]
        guard tab == .day else { return order }
        order += [.previousDay, .today, .nextDay, .calendar, .refresh]
        order += MatchFilter.allCases.map { .filter($0) }
        order.append(.featured)
        order += matchIDs.map { .match($0) }
        return order
    }
}

/// A soron következő fókusz; üres fókuszból az első/utolsó állomásra lép.
func focusedTarget(from current: AppFocus?, step delta: Int, in order: [AppFocus]) -> AppFocus? {
    guard !order.isEmpty else { return nil }
    guard let current, let index = order.firstIndex(of: current) else {
        return delta >= 0 ? order.first : order.last
    }
    return order[(index + delta + order.count) % order.count]
}

/// Vizuális fókuszjelzés a nem kártya vezérlőkön: 2 pt borostyán gyűrű.
extension View {
    func focusRing(_ isFocused: Bool, shape: some InsettableShape) -> some View {
        overlay {
            if isFocused {
                shape.strokeBorder(AppTheme.accent, lineWidth: 2).padding(-2)
            }
        }
    }

    func capsuleFocusRing(_ isFocused: Bool) -> some View {
        focusRing(isFocused, shape: Capsule())
    }

    func circleFocusRing(_ isFocused: Bool) -> some View {
        focusRing(isFocused, shape: Circle())
    }

    /// AX-jelzés: a fókuszált vezérlő `selected` traitet kap (ezt hirdeti a
    /// VoiceOver, és ezt méri az UI-teszt is).
    func focusAccessibility(_ isFocused: Bool) -> some View {
        accessibilityAddTraits(isFocused ? .isSelected : [])
    }
}