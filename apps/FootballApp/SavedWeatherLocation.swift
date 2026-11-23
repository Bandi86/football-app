import Foundation
import FootballAppCore

/// Az időjárás-helyszín a macOS helymeghatározási engedélye nélkül.
///
/// A `CLLocationManager` minden indításkor felugró „engedélyezze a helyet?"
/// ablaka miatt az app egy helyi fájlból olvassa a koordinátákat:
/// `~/Library/Application Support/FootballAppV2/weather-location.json`
/// kulcsai: `name`, `latitude`, `longitude`. A fájl szerkeszthető, újraindítás
/// után azonnal érvényes. Hiány, hibás JSON vagy érvénytelen tartomány esetén
/// Budapest az alaphelyszín — az alkalmazás így soha nem kér TCC-engedélyt.
enum SavedWeatherLocation {
    private struct Config: Decodable {
        let name: String
        let latitude: Double
        let longitude: Double
    }

    static func load() -> WeatherLocation {
        let url = RuntimeInfoLoader.defaultURL
            .deletingLastPathComponent()
            .appendingPathComponent("weather-location.json")
        guard let data = try? Data(contentsOf: url),
              let config = try? JSONDecoder().decode(Config.self, from: data),
              (-90...90).contains(config.latitude),
              (-180...180).contains(config.longitude),
              !config.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return .budapest
        }
        return WeatherLocation(
            name: config.name,
            latitude: config.latitude,
            longitude: config.longitude
        )
    }
}
