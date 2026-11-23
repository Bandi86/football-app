import CoreLocation
import Foundation
import FootballAppCore

/// A felhasználó aktuális tartózkodási helye az időjárás-fejiéchez.
///
/// Csak „használat közbeni" engedélyt kér, egyszeri pozíciót vesz (nem követ
/// folyamatosan), és fordított geokódolással emberi városnevet ad vissza.
/// Engedély hiányában, időtúl esetén vagy geokódolási hiba esetén `nil`-t ad —
/// a fejléc ilyenkor a megmaradó alaphelyszínt mutatja, és ezt jelzi is.
@MainActor
final class UserLocationProvider: NSObject, CLLocationManagerDelegate {
    private let manager = CLLocationManager()
    private let geocoder = CLGeocoder()
    private var continuation: CheckedContinuation<WeatherLocation?, Never>?
    private var timeoutTask: Task<Void, Never>?
    private var finished = false

    override init() {
        super.init()
        manager.delegate = self
        // A városnévhez elég a 3 km-es pontosság; így nem kérünk többet, mint
        // amennyi a fejléc időjárásához kell.
        manager.desiredAccuracy = kCLLocationAccuracyThreeKilometers
    }

    /// Egyszeri helymeghatározás. A continuation legkésőbb `timeout`-mp után
    /// `nil`-t kap, hogy a fejléc ne várakozzon végtelenül az engedélyre.
    func resolve(timeout: Double = 12) async -> WeatherLocation? {
        guard continuation == nil else { return nil }
        return await withCheckedContinuation { continuation in
            self.continuation = continuation
            self.finished = false
            self.timeoutTask = Task { [weak self] in
                try? await Task.sleep(for: .seconds(timeout))
                self?.finish(with: nil)
            }
            self.requestPosition()
        }
    }

    private func requestPosition() {
        // Tudatosan NEM hívunk requestWhenInUseAuthorization()-t: az abrakadabra
        // engedélyező ablak minden indításkor előjönne. Csak akkor használunk
        // eszközhelyzetet, ha a felhasználó azt korábban már külön megadta.
        let status = manager.authorizationStatus
        if isAuthorized(status) {
            manager.requestLocation()
        } else {
            finish(with: nil)
        }
    }

    private func isAuthorized(_ status: CLAuthorizationStatus) -> Bool {
        status == .authorized || status == .authorizedAlways
    }

    // A delegate hívásai nem a fő szálról jöhetnek, ezért átugrunk a MainActorra.
    nonisolated func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        // Az engedély értékét még a delegate-szálon olvassuk ki: így csak egy
        // érték (és nem a manager objektum) lép át a MainActorra.
        let status = manager.authorizationStatus
        Task { @MainActor [weak self] in
            guard let self, !self.finished else { return }
            if self.isAuthorized(status) {
                self.manager.requestLocation()
            } else if status == .denied || status == .restricted {
                self.finish(with: nil)
            }
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        guard let location = locations.last else { return }
        Task { @MainActor [weak self] in
            guard let self, !self.finished else { return }
            let described = await self.describe(location)
            self.finish(with: described)
        }
    }

    nonisolated func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        Task { @MainActor [weak self] in
            self?.finish(with: nil)
        }
    }

    private func describe(_ location: CLLocation) async -> WeatherLocation? {
        let placemark = await withCheckedContinuation { (cont: CheckedContinuation<CLPlacemark?, Never>) in
            geocoder.reverseGeocodeLocation(location) { marks, _ in
                cont.resume(returning: marks?.first)
            }
        }
        let name = placemark?.locality
            ?? placemark?.subAdministrativeArea
            ?? "Saját helyzet"
        return WeatherLocation(
            name: name,
            latitude: location.coordinate.latitude,
            longitude: location.coordinate.longitude
        )
    }

    private func finish(with location: WeatherLocation?) {
        guard !finished else { return }
        finished = true
        timeoutTask?.cancel()
        timeoutTask = nil
        continuation?.resume(returning: location)
        continuation = nil
    }
}
