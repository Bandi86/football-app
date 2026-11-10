import Foundation

public enum APIError: Error, Equatable, Sendable {
    case runtimeMissing
    case unauthorized
    case badRequest
    case server
    case network
    case decoding

    public var userMessage: String {
        switch self {
        case .runtimeMissing:
            return "A footballd daemon nem fut (runtime.json nem található). Indítsd el a daemont."
        case .unauthorized:
            return "Érvénytelen vagy hiányzó hozzáférési token."
        case .badRequest:
            return "Hibás dátum."
        case .server:
            return "A szerver hibát adott. Próbáld újra."
        case .network:
            return "Nem érhető el a helyi API. Fut a daemon?"
        case .decoding:
            return "Váratlan válaszformátum."
        }
    }
}
