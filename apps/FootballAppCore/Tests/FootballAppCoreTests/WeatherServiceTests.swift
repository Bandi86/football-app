import Foundation
import Testing
@testable import FootballAppCore

/// Saját protokoll a globális `StubURLProtocol` helyett: a két suite így nem
/// irja felül egymás handlerét párhuzamos futáskor.
private final class WeatherStubURLProtocol: URLProtocol {
    nonisolated(unsafe) static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = WeatherStubURLProtocol.handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.unknown))
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

@Suite(.serialized) struct WeatherServiceTests {
    private func makeService(
        handler: @escaping (URLRequest) throws -> (HTTPURLResponse, Data)
    ) -> OpenMeteoWeatherService {
        WeatherStubURLProtocol.handler = handler
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [WeatherStubURLProtocol.self]
        return OpenMeteoWeatherService(session: URLSession(configuration: config))
    }

    private func response(_ url: URL?, _ code: Int) -> HTTPURLResponse {
        HTTPURLResponse(url: url!, statusCode: code, httpVersion: nil, headerFields: nil)!
    }

    private var sampleBody: Data {
        """
        {"current":{"time":"2026-11-22T15:15","interval":900,"temperature_2m":12.4,
        "apparent_temperature":10.8,"relative_humidity_2m":76,"weather_code":3,
        "wind_speed_10m":14.2,"is_day":1}}
        """.data(using: .utf8)!
    }

    @Test func requestsOpenMeteoCurrentBlockWithoutSecrets() async throws {
        defer { WeatherStubURLProtocol.handler = nil }
        let captured = CapturedRequest()
        let service = makeService { request in
            captured.store(request)
            return (self.response(request.url, 200), self.sampleBody)
        }

        let weather = try await service.currentWeather()
        let request = try #require(captured.value)
        let url = try #require(request.url)
        #expect(url.host == "api.open-meteo.com")
        #expect(url.path == "/v1/forecast")
        let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        #expect(query.contains { $0.name == "latitude" && $0.value == "47.4979" })
        #expect(query.contains { $0.name == "longitude" && $0.value == "19.0402" })
        #expect(query.contains { $0.name == "timezone" && $0.value == "UTC" })
        #expect(query.contains { $0.name == "current" && ($0.value ?? "").contains("weather_code") })
        #expect(request.value(forHTTPHeaderField: "Authorization") == nil)

        #expect(weather.temperatureC == 12.4)
        #expect(weather.temperatureText == "12,4 °C")
        #expect(weather.humidityPct == 76)
        #expect(weather.weatherCode == 3)
        #expect(weather.kind == .overcast)
        #expect(weather.isDay == true)
        #expect(weather.observedAtUTC != nil)
        #expect(service.cityName == "Budapest")
    }

    @Test func malformedResponseBecomesInvalidResponse() async throws {
        defer { WeatherStubURLProtocol.handler = nil }
        let service = makeService { request in
            (self.response(request.url, 200), "{\"current\":{}}".data(using: .utf8)!)
        }
        do {
            _ = try await service.currentWeather()
            Issue.record("Hiányzó kötelező mezőnél nem szabad sikert jelenteni")
        } catch let error as WeatherError {
            #expect(error == .invalidResponse)
        }
    }

    @Test func httpFailureBecomesUnavailable() async throws {
        defer { WeatherStubURLProtocol.handler = nil }
        let service = makeService { request in
            (self.response(request.url, 503), Data())
        }
        do {
            _ = try await service.currentWeather()
            Issue.record("503-as válasznál nem szabad sikert jelenteni")
        } catch let error as WeatherError {
            #expect(error == .unavailable)
        }
    }

    @Test func localServiceUsesLocalApiAndExposesBackendSchedule() async throws {
        defer { WeatherStubURLProtocol.handler = nil }
        let captured = CapturedRequest()
        let now = Date()
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let capturedAt = formatter.string(from: now)
        let nextRefreshAt = formatter.string(from: now.addingTimeInterval(900))
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [WeatherStubURLProtocol.self]
        let service = LocalWeatherService(
            baseURL: URL(string: "http://127.0.0.1:49355")!,
            readToken: "read-teszt-token",
            session: URLSession(configuration: configuration)
        )
        WeatherStubURLProtocol.handler = { request in
            captured.store(request)
            let body = """
            {"temperature_c":7.5,"apparent_c":4.1,"humidity_pct":88,"wind_speed_kmh":9.2,
             "weather_code":61,"is_day":false,"observed_at":"\(capturedAt)",
             "captured_at":"\(capturedAt)","next_refresh_at":"\(nextRefreshAt)",
             "stale":false,"source":"open-meteo"}
            """.data(using: .utf8)!
            return (self.response(request.url, 200), body)
        }

        let weather = try await service.currentWeather()
        #expect(weather.temperatureC == 7.5)
        #expect(weather.kind == .rain)
        #expect(weather.isDay == false)
        #expect(weather.observedAtUTC != nil)

        let request = try #require(captured.value)
        let url = try #require(request.url)
        #expect(url.host == "127.0.0.1")
        #expect(url.path == "/v1/weather")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer read-teszt-token")

        let storedMetadata = await service.refreshMetadata()
        let metadata = try #require(storedMetadata)
        #expect(metadata.nextRefreshAtUTC != nil)
        #expect(metadata.stale == false)
    }
}
