import Foundation

/// Időjárás-lekérés a fejléc számára. A protokoll miatt a nézetmodell tesztben
/// stubbal dolgozhat, és a hálózat nem szivárog be a nézetbe.
public protocol WeatherService: Sendable {
    func currentWeather() async throws -> CurrentWeather
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
