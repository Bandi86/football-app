import Foundation

public struct HTTPDayService: DayService {
    private let baseURL: URL
    private let readToken: String
    private let session: URLSession
    private let calendar: Calendar

    public init(
        baseURL: URL,
        readToken: String,
        session: URLSession = .shared,
        calendar: Calendar = Calendar(identifier: .gregorian)
    ) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.session = session
        self.calendar = calendar
    }

    public func day(for date: Date) async throws -> [DayMatch] {
        let dateString = Self.formatDate(date, calendar: calendar)
        let request = makeRequest(dateString: dateString)

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw APIError.network
        }
        guard let http = response as? HTTPURLResponse else {
            throw APIError.network
        }
        switch http.statusCode {
        case 200...299:
            do {
                return try APIDecoding.makeDecoder().decode(DayResponse.self, from: data).matches
            } catch {
                throw APIError.decoding
            }
        case 401:
            throw APIError.unauthorized
        case 400:
            throw APIError.badRequest
        default:
            throw APIError.server
        }
    }

    private func makeRequest(dateString: String) -> URLRequest {
        let url = baseURL.appendingPathComponent("v1/day/\(dateString)")
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        request.setValue("Bearer \(readToken)", forHTTPHeaderField: "Authorization")
        return request
    }

    static func formatDate(_ date: Date, calendar: Calendar) -> String {
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter.string(from: date)
    }
}
