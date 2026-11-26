import SwiftUI
import FootballAppCore

/// Kattintható forráslinkek a meccsrészletezőben (S181/S2).
/// Csak a backend által eltárolt http(s) URL-ek jelennek meg; a felhasználó
/// nem adhat meg URL-t, és a kliens nem indít új hálózati kérést a linkre.
struct SourceLinkList: View {
    let links: [MatchLink]
    @Environment(\.openURL) private var openURL

    var body: some View {
        VStack(alignment: .leading, spacing: AppTheme.Spacing.s) {
            Text("FORRÁSOK")
                .sectionLabel()
                .accessibilityIdentifier("match.detail.links")
            ForEach(links) { link in
                if let url = URL(string: link.url),
                   ["http", "https"].contains(url.scheme?.lowercased() ?? "") {
                    Button {
                        openURL(url)
                    } label: {
                        HStack(spacing: AppTheme.Spacing.s) {
                            Text(link.label)
                                .font(.appText(12))
                                .foregroundStyle(AppTheme.primaryText)
                            Spacer(minLength: 0)
                            Image(systemName: "arrow.up.right.square")
                                .font(.system(size: 11))
                                .foregroundStyle(AppTheme.accent)
                        }
                        .padding(.vertical, 5)
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel("Forrás megnyitása: \(link.label)")
                    .accessibilityIdentifier("match.link.\(link.kind)")
                }
            }
        }
    }
}
