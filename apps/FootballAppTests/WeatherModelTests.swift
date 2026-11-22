import XCTest
import FootballAppCore
@testable import FootballApp

private final class MutableWeatherService: WeatherService, @unchecked Sendable {
    var result: Result<CurrentWeather, WeatherError>

    init(result: Result<CurrentWeather, WeatherError>) {
        self.result = result
    }

    func currentWeather() async throws -> CurrentWeather {
        try result.get()
    }
}

@MainActor
final class WeatherModelTests: XCTestCase {
    private var sample: CurrentWeather {
        CurrentWeather(temperatureC: 9.5, humidityPct: 88, weatherCode: 61, isDay: true)
    }

    func testLoadPublishesLiveWeather() async {
        let model = WeatherModel(service: MutableWeatherService(result: .success(sample)))
        await model.load()
        XCTAssertEqual(model.status, .loaded)
        XCTAssertTrue(model.isLive)
        XCTAssertEqual(model.current?.temperatureText, "9,5 °C")
        XCTAssertEqual(model.current?.kind, .rain)
    }

    func testFailureKeepsLastKnownWeatherButMarksFailed() async {
        let service = MutableWeatherService(result: .success(sample))
        let model = WeatherModel(service: service)
        await model.load()
        service.result = .failure(.unavailable)
        await model.load()
        XCTAssertEqual(model.status, .failed)
        XCTAssertTrue(model.isLive, "A korábbi megfigyelés nem vész el hálózati hiba miatt")
        XCTAssertEqual(model.current?.weatherCode, 61)
    }

    func testInitialFailureLeavesHeaderInFallbackMode() async {
        let model = WeatherModel(service: MutableWeatherService(result: .failure(.unavailable)))
        await model.load()
        XCTAssertEqual(model.status, .failed)
        XCTAssertFalse(model.isLive)
        XCTAssertNil(model.current)
    }

    func testDeviceLocationOverridesCityAndIsReportedHonestly() async {
        let model = WeatherModel(service: MutableWeatherService(result: .success(sample)))
        XCTAssertEqual(model.cityName, "Budapest")
        XCTAssertFalse(model.usesDeviceLocation)

        let home = WeatherLocation(name: "Debrecen", latitude: 47.5316, longitude: 21.6273)
        await model.useLocation(home)

        XCTAssertEqual(model.cityName, "Debrecen")
        XCTAssertTrue(model.usesDeviceLocation)
        XCTAssertEqual(model.location.latitude, 47.5316, accuracy: 0.0001)
    }
}
