import Foundation

public enum APIError: Error, Equatable, Sendable {
    case runtimeMissing
    case unauthorized
    case notFound
    case badRequest
    /// A szerver 400-as válasza saját, felhasználónak szánt üzenettel
    /// ({"code","message"}). A nyers üzenetet jelenítjük meg, mert az
    /// pontosabban mondja meg, melyik mezővel van baj.
    case serverMessage(String)
    /// 409: a természetes kulcs ütközése (pl. ugyanarra a meccsre már van
    /// kézi megfigyelés). Az üzenet a szerverről jön.
    case conflict(String)
    case server
    case network
    case decoding

    public var userMessage: String {
        switch self {
        case .runtimeMissing:
            return "A footballd daemon nem fut (runtime.json nem található). Indítsd el a daemont."
        case .unauthorized:
            return "Érvénytelen vagy hiányzó hozzáférési token."
        case .notFound:
            return "A keresett elem nem található."
        case .badRequest:
            return "A kérés érvénytelen. Ellenőrizd a megadott adatokat."
        case .serverMessage(let message):
            return message.isEmpty
                ? "A szerver elutasította a kérést. Ellenőrizd a megadott adatokat."
                : message
        case .conflict(let message):
            return message.isEmpty
                ? "A meccshez már tartozik ilyen kézi megfigyelés."
                : message
        case .server:
            return "A szerver hibát adott. Próbáld újra."
        case .network:
            return "Nem érhető el a helyi API. Fut a daemon?"
        case .decoding:
            return "Váratlan válaszformátum."
        }
    }
}