import Foundation
import Observation
import FootballAppCore

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
            self.service = OpenMeteoWeatherService(location: location)
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
            service = OpenMeteoWeatherService(location: newLocation)
        }
        await load()
    }

    func load() async {
        status = .loading
        do {
            let weather = try await service.currentWeather()
            current = weather
            status = .loaded
        } catch is CancellationError {
            // A nézet eltűnt vagy új kör vette át: a régi állapot marad.
            status = current == nil ? .idle : status
        } catch {
            // Nem töröljük a már megjelenített adatot: a fejléc korábbi
            // megfigyelése továbbra is pontos, csak elavult lehet.
            status = .failed
        }
    }
}
