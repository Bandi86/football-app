import SwiftUI

/// S220: a főoldal jobb oldali, beágyazott meccs-részletező panelje.
///
/// A bal oldali kártyára kattintva itt jelenik meg a teljes meccsadat —
/// ugyanaz a `MatchDetailPane`, mint a modalis részletezőben, csak panelbe
/// ágyazva (nincs új ablak, nincs sheet a fő kijelöléshez).
///
/// Kijelölt meccs nélkül egyértelmű, de **ideiglenes** üres állapotot mutat:
/// a felhasználó később dönt arról, mi jelenjen meg ilyenkor (pl. napi
/// összefoglaló, kedvencek). Az üres tartalom ezért külön, egy helyen van.
struct MatchDetailPanel: View {
    let matchID: UUID?
    let onClose: () -> Void

    var body: some View {
        Group {
            if let matchID {
                MatchDetailPane(
                    matchId: matchID,
                    onClose: onClose,
                    headerTitle: "Meccs részletei",
                    contentMaxWidth: nil
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                emptySelection
            }
        }
        .background(
            RoundedRectangle(cornerRadius: 19, style: .continuous)
                .fill(AppTheme.surface.opacity(0.35))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 19, style: .continuous)
                .strokeBorder(AppTheme.hairline)
                .allowsHitTesting(false)
        )
        .clipped()
    }

    /// Ideiglenes üres állapot — később itt jöhet a napi összefoglaló.
    private var emptySelection: some View {
        VStack(spacing: 12) {
            Image(systemName: "sportscourt")
                .font(.system(size: 34, weight: .light))
                .foregroundStyle(AppTheme.tertiaryText)
            Text("Válassz meccset")
                .font(.appLabel(13))
                .foregroundStyle(AppTheme.secondaryText)
            Text("A bal oldali listán kattints egy meccsre a részletekért.")
                .font(.appText(11))
                .foregroundStyle(AppTheme.tertiaryText)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(32)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Nincs kijelölt meccs. Válassz egy meccset a bal oldali listán.")
        .accessibilityIdentifier("home.detail.empty")
    }
}
