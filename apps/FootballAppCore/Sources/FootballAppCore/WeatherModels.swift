import Foundation

/// Az Open-Meteo `/v1/forecast` válaszának csak a fejléc által használt része.
///
/// A dekódolás szándékosan szigorú a két kötelező mezőre (hőmérséklet,
/// weather code): ha a válasz séma-szinten hibás, a teljes dekódolás elbukik,
/// és a fejléc a napszak-alapú hangulatra esik vissza — nem mutat hamis
/// időjárást. A többi mező hiányozhat.
public struct CurrentWeather: Codable, Equatable, Sendable {
    public let temperatureC: Double
    public let apparentC: Double?
    public let humidityPct: Int?
    public let windSpeedKmh: Double?
    public let weatherCode: Int
    public let isDay: Bool
    /// Open-Meteo `current.time` mezője UTC-ben (a szolgáltatás `timezone=UTC`
    /// paraméterrel kéri), így minden gépen egységesen értelmezhető.
    public let observedAtUTC: Date?

    private enum CodingKeys: String, CodingKey {
        case temperatureC = "temperature_2m"
        case apparentC = "apparent_temperature"
        case humidityPct = "relative_humidity_2m"
        case windSpeedKmh = "wind_speed_10m"
        case weatherCode = "weather_code"
        case isDay = "is_day"
        case time
    }

    public init(
        temperatureC: Double,
        apparentC: Double? = nil,
        humidityPct: Int? = nil,
        windSpeedKmh: Double? = nil,
        weatherCode: Int,
        isDay: Bool,
        observedAtUTC: Date? = nil
    ) {
        self.temperatureC = temperatureC
        self.apparentC = apparentC
        self.humidityPct = humidityPct
        self.windSpeedKmh = windSpeedKmh
        self.weatherCode = weatherCode
        self.isDay = isDay
        self.observedAtUTC = observedAtUTC
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        temperatureC = try container.decode(Double.self, forKey: .temperatureC)
        weatherCode = try container.decode(Int.self, forKey: .weatherCode)
        apparentC = try container.decodeIfPresent(Double.self, forKey: .apparentC)
        humidityPct = try container.decodeIfPresent(Int.self, forKey: .humidityPct)
        windSpeedKmh = try container.decodeIfPresent(Double.self, forKey: .windSpeedKmh)
        if let rawIsDay = try container.decodeIfPresent(Int.self, forKey: .isDay) {
            isDay = rawIsDay != 0
        } else {
            isDay = true
        }
        if let rawTime = try container.decodeIfPresent(String.self, forKey: .time) {
            observedAtUTC = Self.parseUTCTime(rawTime)
        } else {
            observedAtUTC = nil
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(temperatureC, forKey: .temperatureC)
        try container.encode(weatherCode, forKey: .weatherCode)
        try container.encodeIfPresent(apparentC, forKey: .apparentC)
        try container.encodeIfPresent(humidityPct, forKey: .humidityPct)
        try container.encodeIfPresent(windSpeedKmh, forKey: .windSpeedKmh)
        try container.encode(isDay ? 1 : 0, forKey: .isDay)
    }

    /// A WMO weather code-ból származott megjeleníthető kategória.
    public var kind: WeatherKind { WeatherKind(code: weatherCode) }

    /// Magyar formátumú hőmérséklet, legfeljebb egy tizedesjeggyel („12,4 °C").
    public var temperatureText: String {
        Self.decimalText(temperatureC) + " °C"
    }

    /// Magyar tizedesvessző, locale-tól függetlenül (a régi DTO-k mintájára).
    public static func decimalText(_ value: Double) -> String {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = 0
        formatter.maximumFractionDigits = 1
        formatter.usesGroupingSeparator = false
        return formatter.string(from: NSNumber(value: value)) ?? String(format: "%.1f", value)
    }

    private static func parseUTCTime(_ raw: String) -> Date? {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        formatter.dateFormat = "yyyy-MM-dd'T'HH:mm"
        return formatter.date(from: raw)
    }
}

/// Az Open-Meteo válaszának legfelső objektuma. A `current` hiányozhat
/// (pl. a szolgáltatás épp nem szolgál adatot), ilyenkor a hívás `nil`-t ad.
public struct OpenMeteoResponse: Decodable, Sendable {
    public let current: CurrentWeather?
}

/// WMO weather code → megjeleníthető kategória. A kategória hordozza az
/// SF Symbolt és a magyar feliratot; a színeket a `HeaderMood` adja hozzá,
/// hogy a napszak és az időjárás együtt alakítsa a fejléc hangulatát.
public enum WeatherKind: String, CaseIterable, Sendable {
    case clear
    case partlyCloudy
    case overcast
    case fog
    case drizzle
    case rain
    case snow
    case thunderstorm

    public init(code: Int) {
        switch code {
        case 0: self = .clear
        case 1, 2: self = .partlyCloudy
        case 3: self = .overcast
        case 45, 48: self = .fog
        case 51, 53, 55, 56, 57: self = .drizzle
        case 61, 63, 65, 66, 67, 80, 81, 82: self = .rain
        case 71, 73, 75, 77, 85, 86: self = .snow
        case 95, 96, 99: self = .thunderstorm
        default: self = .overcast
        }
    }

    /// Nappali SF Symbol; a fejléc éjszaka a `nightSymbolName`-t használja.
    public var symbolName: String {
        switch self {
        case .clear: return "sun.max.fill"
        case .partlyCloudy: return "cloud.sun.fill"
        case .overcast: return "cloud.fill"
        case .fog: return "cloud.fog.fill"
        case .drizzle: return "cloud.drizzle.fill"
        case .rain: return "cloud.rain.fill"
        case .snow: return "cloud.snow.fill"
        case .thunderstorm: return "cloud.bolt.fill"
        }
    }

    public var nightSymbolName: String {
        switch self {
        case .clear: return "moon.stars.fill"
        case .partlyCloudy: return "cloud.moon.fill"
        default: return symbolName
        }
    }

    public var hungarianLabel: String {
        switch self {
        case .clear: return "Derült"
        case .partlyCloudy: return "Részben felhős"
        case .overcast: return "Borult"
        case .fog: return "Köd"
        case .drizzle: return "Szitálás"
        case .rain: return "Eső"
        case .snow: return "Havazás"
        case .thunderstorm: return "Zivatar"
        }
    }

    /// Csapadékos időben a fejléc animációja is megváltozik.
    public var hasPrecipitation: Bool {
        switch self {
        case .drizzle, .rain, .snow, .thunderstorm: return true
        default: return false
        }
    }
}
