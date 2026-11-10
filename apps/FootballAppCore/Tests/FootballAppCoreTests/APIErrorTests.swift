import Testing
@testable import FootballAppCore

@Suite struct APIErrorTests {
    /// A magyar felhasználói szövegek szerződésként viselkednek: a forrásban
    /// szereplő pontos megfogalmazást rögzítjük itt is.
    @Test func userMessagesAreExactHungarianStrings() {
        #expect(
            APIError.runtimeMissing.userMessage
                == "A footballd daemon nem fut (runtime.json nem található). Indítsd el a daemont."
        )
        #expect(APIError.unauthorized.userMessage == "Érvénytelen vagy hiányzó hozzáférési token.")
        #expect(APIError.badRequest.userMessage == "Hibás dátum.")
        #expect(APIError.server.userMessage == "A szerver hibát adott. Próbáld újra.")
        #expect(APIError.network.userMessage == "Nem érhető el a helyi API. Fut a daemon?")
        #expect(APIError.decoding.userMessage == "Váratlan válaszformátum.")
    }
}
