import Foundation
import Observation
import FootballAppCore

/// Nincs runtime konfig (pl. daemon még nem indult): a fejléc ilyenkor
/// napszak-alapú háttérre esik vissza, nem próbál külső hálózatot.
private struct UnavailableWeatherService: WeatherService {
    func currentWeather() async throws -> CurrentWeather {
        throw WeatherError.unavailable
    }
}

/// A fejléc időjárásának állapota. A hálózat csak a Core `WeatherService`
/// protokollján át érkezik; hiba esetén a korábban megjelenített adat megmarad
/// (elavultként), a fejléc pedig napszak-alapú hangulatra esik vissza.
@MainActor
@Observable
final class WeatherModel {
    enum Status: Equatable {
        case idle
        case loading
        case loaded
        case failed
    }

    private(set) var current: CurrentWeather?
    private(set) var status: Status = .idle
    private(set) var location: WeatherLocation
    /// A backend által birtokolt következő frissítés — a fejléc countdown-ja erről olvas.
    private(set) var nextRefreshAtUTC: Date?
    private(set) var capturedAtUTC: Date?
    private(set) var weatherIsStale = false

    /// A backend periódusa a két időbélyegből; a countdown ezt használja körként.
    var refreshCycle: TimeInterval? {
        guard let capturedAtUTC, let nextRefreshAtUTC else { return nil }
        let cycle = nextRefreshAtUTC.timeIntervalSince(capturedAtUTC)
        return cycle > 0 ? cycle : nil
    }
    /// Igaz, ha a koordináták a felhasználó engedélyezett eszközhelyzetéből
    /// jönnek — a felület ezt külön kimondja, nem állít alaphelyszínt élőnek.
    private(set) var usesDeviceLocation = false

    private var service: any WeatherService
    /// Teszt-injektált szolgáltatásnál a helyzetváltás nem cseréli le a stubot.
    private let hasInjectedService: Bool

    init(service: (any WeatherService)? = nil, location: WeatherLocation = .budapest) {
        if let service {
            self.service = service
            self.hasInjectedService = true
        } else {
            if let info = try? RuntimeInfoLoader().load(),
               let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
                self.service = LocalWeatherService(
                    baseURL: baseURL,
                    readToken: info.readToken,
                    location: location
                )
            } else {
                self.service = UnavailableWeatherService()
            }
            self.hasInjectedService = false
        }
        self.location = location
    }

    /// A megjeleníthető időjárás akkor élő, ha van sikeresen dekódolt adat.
    var isLive: Bool { current != nil }

    var cityName: String { location.name }

    /// Az eszköz helyzetének átvétele: új szolgáltatás az új koordinátákkal,
    /// majd azonnali újralekérés. Engedély hiányában a hívó nem hívja ezt.
    func useLocation(_ newLocation: WeatherLocation) async {
        location = newLocation
        usesDeviceLocation = true
        if !hasInjectedService {
            if let info = try? RuntimeInfoLoader().load(),
               let baseURL = URL(string: "http://127.0.0.1:\(info.port)") {
                service = LocalWeatherService(
                    baseURL: baseURL,
                    readToken: info.readToken,
                    location: newLocation
                )
            } else {
                service = UnavailableWeatherService()
            }
        }
        await load()
    }

    func load() async {
        status = .loading
        do {
            let weather = try await service.currentWeather()
            current = weather
            let metadata = await service.refreshMetadata()
            capturedAtUTC = metadata?.capturedAtUTC
            nextRefreshAtUTC = metadata?.nextRefreshAtUTC
            weatherIsStale = metadata?.stale ?? false
            status = .loaded
        } catch is CancellationError {
            // A nézet eltűnt vagy új kör vette át: a régi állapot marad.
            status = current == nil ? .idle : status
        } catch {
            // Nem töröljük a már megjelenített adatot: a fejléc korábbi
            // megfigyelése továbbra is pontos, csak elavult lehet.
            weatherIsStale = true
            status = .failed
        }
    }
}
