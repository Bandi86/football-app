import Foundation

/// Időjárás-lekérés a fejléc számára. A protokoll miatt a nézetmodell tesztben
/// stubbal dolgozhat, és a hálózat nem szivárog be a nézetbe.
public protocol WeatherService: Sendable {
    func currentWeather() async throws -> CurrentWeather

    /// A backend által birtokolt frissítési adatok. A régi, közvetlen Open-Meteo
    /// szolgáltatónál nincs ilyen — az alapértelmezett `nil` megőrzi a teszteket
    /// és a napszak-alapú visszaesést.
    func refreshMetadata() async -> WeatherRefreshMetadata?
}

extension WeatherService {
    public func refreshMetadata() async -> WeatherRefreshMetadata? { nil }
}

/// A fejléc visszaszámlálójának valódi, backendből származó adatai.
public struct WeatherRefreshMetadata: Equatable, Sendable {
    public let capturedAtUTC: Date?
    public let nextRefreshAtUTC: Date?
    public let stale: Bool

    public init(capturedAtUTC: Date?, nextRefreshAtUTC: Date?, stale: Bool) {
        self.capturedAtUTC = capturedAtUTC
        self.nextRefreshAtUTC = nextRefreshAtUTC
        self.stale = stale
    }
}

public enum WeatherError: Error, Equatable, Sendable {
    case unavailable
    case invalidResponse
}

/// Az időjárás-lekérés helyszíne. A felület saját tesszük át a felhasználó
/// aktuális koordinátáira; hozzájárulás hiányában ez marad az alapértelmezés.
public struct WeatherLocation: Equatable, Sendable {
    public let name: String
    public let latitude: Double
    public let longitude: Double

    public init(name: String, latitude: Double, longitude: Double) {
        self.name = name
        self.latitude = latitude
        self.longitude = longitude
    }

    public static let budapest = WeatherLocation(
        name: "Budapest", latitude: 47.4979, longitude: 19.0402
    )
}

/// Nyilvános, kulcs nélküli Open-Meteo current-weather hívás.
///
/// Csak a Core réteg régi tesztei használják közvetlenül: az app a
/// `LocalWeatherService`-t használja, így egyetlen kérés sem megy külső
/// szolgáltatásba a kliensből.
///
/// Read-only, nem hordoz titkot és nem ír semmilyen rendszerbe. A football-adatok
/// továbbra is kizárólag a `127.0.0.1` API-ról jönnek; ez a hívás kizárólag a
/// fejléc dekoratív időjárási hangulatát szolgálja, és hiba esetén a kliens
/// napszak-alapú háttérre esik vissza.
public struct OpenMeteoWeatherService: WeatherService {
    /// A megjelenített város. Alapértelmezés: Budapest (az app magyar felülete).
    public typealias Location = WeatherLocation

    private let location: WeatherLocation
    private let session: URLSession
    private let decoder: JSONDecoder

    public init(location: Location = .budapest, session: URLSession? = nil) {
        self.location = location
        if let session {
            self.session = session
        } else {
            // Rövid, de nem végtelen időkorlát: a fejléc nem blokkolhatja a
            // nap betöltését, ha a gép épp offline.
            let configuration = URLSessionConfiguration.ephemeral
            configuration.timeoutIntervalForRequest = 5
            configuration.timeoutIntervalForResource = 8
            self.session = URLSession(configuration: configuration)
        }
        self.decoder = JSONDecoder()
    }

    public var cityName: String { location.name }

    public func currentWeather() async throws -> CurrentWeather {
        guard let url = endpoint() else {
            throw WeatherError.invalidResponse
        }
        let request = URLRequest(url: url)
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch is CancellationError {
            throw CancellationError()
        } catch let urlError as URLError where urlError.code == .cancelled {
            throw CancellationError()
        } catch {
            throw WeatherError.unavailable
        }
        guard let http = response as? HTTPURLResponse, (200...299).contains(http.statusCode) else {
            throw WeatherError.unavailable
        }
        do {
            let decoded = try decoder.decode(OpenMeteoResponse.self, from: data)
            guard let current = decoded.current else { throw WeatherError.invalidResponse }
            return current
        } catch {
            // Hiányos current blokk és séma-eltérés egyaránt „nincs időjárás”.
            throw WeatherError.invalidResponse
        }
    }

    private func endpoint() -> URL? {
        var components = URLComponents(string: "https://api.open-meteo.com/v1/forecast")
        components?.queryItems = [
            URLQueryItem(name: "latitude", value: String(location.latitude)),
            URLQueryItem(name: "longitude", value: String(location.longitude)),
            URLQueryItem(name: "current", value: "temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m,is_day"),
            URLQueryItem(name: "timezone", value: "UTC"),
        ]
        return components?.url
    }
}

/// Az időjárás a helyi daemon API-ból. A kliens így teljes egészében a
/// `127.0.0.1` API-ról kap adatot; a koordinátákat csak a saját API-nknak
/// küldjük, és nem tároljuk, nem naplózzuk őket.
private actor WeatherMetadataStore {
    private var value: WeatherRefreshMetadata?
    func set(_ newValue: WeatherRefreshMetadata) { value = newValue }
    func get() -> WeatherRefreshMetadata? { value }
}

public final class LocalWeatherService: WeatherService, @unchecked Sendable {
    private let baseURL: URL
    private let readToken: String
    private let location: WeatherLocation
    private let session: URLSession
    private let decoder = APIDecoding.makeDecoder()
    private let metadataStore = WeatherMetadataStore()

    public init(
        baseURL: URL,
        readToken: String,
        location: WeatherLocation = .budapest,
        session: URLSession? = nil
    ) {
        self.baseURL = baseURL
        self.readToken = readToken
        self.location = location
        if let session {
            self.session = session
        } else {
            let configuration = URLSessionConfiguration.ephemeral
            configuration.timeoutIntervalForRequest = 6
            configuration.timeoutIntervalForResource = 10
            self.session = URLSession(configuration: configuration)
        }
    }

    public func refreshMetadata() async -> WeatherRefreshMetadata? {
        await metadataStore.get()
    }

    public func currentWeather() async throws -> CurrentWeather {
        guard var components = URLComponents(
            url: baseURL.appendingPathComponent("v1/weather"),
            resolvingAgainstBaseURL: false
        ) else { throw WeatherError.invalidResponse }
        components.queryItems = [
            URLQueryItem(name: "latitude", value: String(location.latitude)),
            URLQueryItem(name: "longitude", value: String(location.longitude)),
        ]
        guard let url = components.url else { throw WeatherError.invalidResponse }
        var request = URLRequest(url: url)
        request.setValue("Bearer \(readToken)", forHTTPHeaderField: "Authorization")

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch is CancellationError {
            throw CancellationError()
        } catch let urlError as URLError where urlError.code == .cancelled {
            throw CancellationError()
        } catch {
            throw WeatherError.unavailable
        }
        guard let http = response as? HTTPURLResponse, (200...299).contains(http.statusCode) else {
            throw WeatherError.unavailable
        }
        let decoded: LocalWeatherResponse
        do {
            decoded = try decoder.decode(LocalWeatherResponse.self, from: data)
        } catch {
            throw WeatherError.invalidResponse
        }
        let current = CurrentWeather(
            temperatureC: decoded.temperatureC,
            apparentC: decoded.apparentC,
            humidityPct: decoded.humidityPct,
            windSpeedKmh: decoded.windSpeedKmh,
            weatherCode: decoded.weatherCode,
            isDay: decoded.isDay,
            observedAtUTC: decoded.observedAt
        )
        let metadata = WeatherRefreshMetadata(
            capturedAtUTC: decoded.capturedAt,
            nextRefreshAtUTC: decoded.nextRefreshAt,
            stale: decoded.stale
        )
        await metadataStore.set(metadata)
        return current
    }
}

/// A backend `/v1/weather` válasza. A hiányzó opcionális mezők `nil`, nem hamis érték.
struct LocalWeatherResponse: Decodable {
    let temperatureC: Double
    let apparentC: Double?
    let humidityPct: Int?
    let windSpeedKmh: Double?
    let weatherCode: Int
    let isDay: Bool
    let observedAt: Date?
    let capturedAt: Date
    let nextRefreshAt: Date
    let stale: Bool
    let source: String

    enum CodingKeys: String, CodingKey {
        case temperatureC = "temperature_c"
        case apparentC = "apparent_c"
        case humidityPct = "humidity_pct"
        case windSpeedKmh = "wind_speed_kmh"
        case weatherCode = "weather_code"
        case isDay = "is_day"
        case observedAt = "observed_at"
        case capturedAt = "captured_at"
        case nextRefreshAt = "next_refresh_at"
        case stale
        case source
    }
}
