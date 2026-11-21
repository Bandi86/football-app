import Foundation

/// A HTTP API egységes hibatörzse: minden nem 2xx-es válasz
/// `{"code": "...", "message": "..."}` alakú (a `request_id` opcionális).
public struct APIServerErrorPayload: Decodable, Equatable, Sendable {
    public let code: String?
    public let message: String?

    public init(code: String?, message: String?) {
        self.code = code
        self.message = message
    }
}

public enum APIDecoding {
    public static func makeDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let string = try container.decode(String.self)
            let fractional = ISO8601DateFormatter()
            fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            if let date = fractional.date(from: string) { return date }
            let plain = ISO8601DateFormatter()
            plain.formatOptions = [.withInternetDateTime]
            if let date = plain.date(from: string) { return date }
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Érvénytelen ISO-8601 dátum: \(string)"
            )
        }
        return decoder
    }

    /// A szerver hibaválaszának dekódolása. Csak akkor ad értéket, ha a test
    /// egyáltalán JSON és van nem üres `message` mezője.
    public static func serverErrorMessage(from data: Data) -> String? {
        guard !data.isEmpty,
              let payload = try? JSONDecoder().decode(APIServerErrorPayload.self, from: data)
        else { return nil }
        return normalized(payload.message)
    }

    /// A szerver hibaválaszának teljes tartalma (code + message).
    public static func serverError(from data: Data) -> APIServerErrorPayload? {
        guard !data.isEmpty else { return nil }
        return try? JSONDecoder().decode(APIServerErrorPayload.self, from: data)
    }

    private static func normalized(_ message: String?) -> String? {
        guard let message else { return nil }
        let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}