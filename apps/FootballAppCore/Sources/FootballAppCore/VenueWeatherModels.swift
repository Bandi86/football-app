import Foundation

/// A meccs helyszíne a részletező „Stadion" sorához. A backend a match detail
/// válaszban opcionálisan küldi; a régi válaszokból hiányozhat.
public struct VenueInfo: Codable, Equatable, Sendable {
    public let name: String
    public let city: String?
    public let country: String?
    public let capacity: Int?
    public let surface: String?

    public init(
        name: String,
        city: String? = nil,
        country: String? = nil,
        capacity: Int? = nil,
        surface: String? = nil
    ) {
        self.name = name
        self.city = city
        self.country = country
        self.capacity = capacity
        self.surface = surface
    }

    private enum CodingKeys: String, CodingKey {
        case name
        case city
        case country
        case capacity
        case surface
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        // A név a kontraktus szerint kötelező, de egy hiányos válasz miatt a
        // teljes meccs-részletező ne törjön el: üres név esetén a sor kimarad.
        name = try container.decodeIfPresent(String.self, forKey: .name) ?? ""
        city = try container.decodeIfPresent(String.self, forKey: .city)
        country = try container.decodeIfPresent(String.self, forKey: .country)
        capacity = try container.decodeIfPresent(Int.self, forKey: .capacity)
        surface = try container.decodeIfPresent(String.self, forKey: .surface)
    }

    /// A „Stadion" sor szövege: „Camp Nou — Barcelona · 99 354 néző".
    /// Nil, ha nincs megjeleníthető stadionnév.
    public var displayText: String? {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty else { return nil }
        var text = trimmedName
        if let city = city?.trimmingCharacters(in: .whitespacesAndNewlines), !city.isEmpty {
            text += " — \(city)"
        }
        if let capacity, capacity > 0 {
            text += " · \(Self.groupedDigits(capacity)) néző"
        }
        return text
    }

    /// Ezres csoportosítás szóközzel (12 345), locale-tól függetlenül.
    static func groupedDigits(_ value: Int) -> String {
        let sign = value < 0 ? "-" : ""
        let digits = String(abs(value))
        var result = ""
        for (index, character) in digits.enumerated() {
            if index > 0, (digits.count - index) % 3 == 0 {
                result.append(" ")
            }
            result.append(character)
        }
        return sign + result
    }
}

/// Várható időjárás a kezdés helyszínén a részletező „Időjárás" sorához.
/// A backend opcionálisan küldi; minden mező hiányozhat.
public struct WeatherInfo: Codable, Equatable, Sendable {
    public let temperatureC: Double?
    public let condition: String?
    public let windSpeed: Double?
    public let windDirection: String?
    public let humidityPct: Int?
    public let precipitation: Double?
    public let cloudCoverPct: Int?
    public let observedAt: String?

    public init(
        temperatureC: Double? = nil,
        condition: String? = nil,
        windSpeed: Double? = nil,
        windDirection: String? = nil,
        humidityPct: Int? = nil,
        precipitation: Double? = nil,
        cloudCoverPct: Int? = nil,
        observedAt: String? = nil
    ) {
        self.temperatureC = temperatureC
        self.condition = condition
        self.windSpeed = windSpeed
        self.windDirection = windDirection
        self.humidityPct = humidityPct
        self.precipitation = precipitation
        self.cloudCoverPct = cloudCoverPct
        self.observedAt = observedAt
    }

    private enum CodingKeys: String, CodingKey {
        case temperatureC = "temperature_c"
        case condition = "description"
        case windSpeed = "wind_speed"
        case windDirection = "wind_direction"
        case humidityPct = "humidity_pct"
        case precipitation
        case cloudCoverPct = "cloud_cover_pct"
        case observedAt = "observed_at"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        temperatureC = try container.decodeIfPresent(Double.self, forKey: .temperatureC)
        condition = try container.decodeIfPresent(String.self, forKey: .condition)
        windSpeed = try container.decodeIfPresent(Double.self, forKey: .windSpeed)
        windDirection = try container.decodeIfPresent(String.self, forKey: .windDirection)
        humidityPct = try container.decodeIfPresent(Int.self, forKey: .humidityPct)
        precipitation = try container.decodeIfPresent(Double.self, forKey: .precipitation)
        cloudCoverPct = try container.decodeIfPresent(Int.self, forKey: .cloudCoverPct)
        observedAt = try container.decodeIfPresent(String.self, forKey: .observedAt)
    }

    /// Az „Időjárás" sor szövege, csak a meglévő részekkel:
    /// „12 °C · enyhe eső · szél 3,4 m/s · páratartalom 70%".
    /// Nil, ha egyetlen megjeleníthető rész sincs.
    public var displayText: String? {
        var parts: [String] = []
        if let temperatureC, temperatureC.isFinite {
            parts.append("\(Self.decimalText(temperatureC)) °C")
        }
        if let condition = Self.hungarianCondition(condition) {
            parts.append(condition)
        }
        if let windSpeed, windSpeed.isFinite {
            var wind = "szél \(Self.decimalText(windSpeed)) m/s"
            if let direction = Self.hungarianDirection(windDirection) {
                wind += " (\(direction))"
            }
            parts.append(wind)
        }
        if let humidityPct {
            parts.append("páratartalom \(humidityPct)%")
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// Magyar tizedesvessző, legfeljebb egy tizedesjegy (12 / 12,5 / 3,4).
    static func decimalText(_ value: Double) -> String {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = 0
        formatter.maximumFractionDigits = 1
        formatter.usesGroupingSeparator = false
        return formatter.string(from: NSNumber(value: value)) ?? String(format: "%.1f", value)
    }

    /// A forrás (jellemzően OpenWeather) angol időjárás-leírásának magyar
    /// megfelelője; ismeretlen szöveget változatlanul adunk vissza.
    static func hungarianCondition(_ raw: String?) -> String? {
        guard let raw = raw?.trimmingCharacters(in: .whitespacesAndNewlines), !raw.isEmpty else {
            return nil
        }
        let key = raw.lowercased()
        return conditionTranslations[key] ?? raw
    }

    /// Magyar égtáj-rövidítés az angol irányból; ismeretlennél nil (kihagyjuk).
    static func hungarianDirection(_ raw: String?) -> String? {
        guard let raw = raw?.trimmingCharacters(in: .whitespacesAndNewlines), !raw.isEmpty else {
            return nil
        }
        return directionTranslations[raw.uppercased()]
    }

    private static let conditionTranslations: [String: String] = [
        "clear sky": "derült",
        "few clouds": "kevés felhő",
        "scattered clouds": "szórt felhőzet",
        "broken clouds": "megosztott felhőzet",
        "overcast clouds": "borult",
        "light rain": "enyhe eső",
        "moderate rain": "mérsékelt eső",
        "heavy intensity rain": "heves eső",
        "very heavy rain": "nagyon heves eső",
        "extreme rain": "szélsőséges eső",
        "freezing rain": "fagyos eső",
        "light intensity shower rain": "enyhe zápor",
        "shower rain": "zápor",
        "heavy intensity shower rain": "heves zápor",
        "ragged shower rain": "szakadozott zápor",
        "light intensity drizzle": "enyhe szitálás",
        "drizzle": "szitálás",
        "heavy intensity drizzle": "heves szitálás",
        "thunderstorm": "zivatar",
        "light thunderstorm": "enyhe zivatar",
        "heavy thunderstorm": "heves zivatar",
        "ragged thunderstorm": "szakadozott zivatar",
        "light snow": "enyhe havazás",
        "snow": "havazás",
        "heavy snow": "heves havazás",
        "sleet": "ónos eső",
        "light shower sleet": "enyhe havas eső",
        "shower sleet": "havas eső",
        "light rain and snow": "enyhe eső és hó",
        "rain and snow": "eső és hó",
        "mist": "pára",
        "fog": "köd",
        "haze": "enyhe köd",
        "smoke": "füst",
        "dust": "por",
        "sand": "homok",
        "ash": "hamu",
        "squalls": "széllökés",
        "tornado": "tornádó",
    ]

    private static let directionTranslations: [String: String] = [
        "N": "É",
        "NNE": "ÉÉK",
        "NE": "ÉK",
        "ENE": "KÉK",
        "E": "K",
        "ESE": "KDK",
        "SE": "DK",
        "SSE": "DDK",
        "S": "D",
        "SSW": "DDNY",
        "SW": "DNY",
        "WSW": "NYDNY",
        "W": "NY",
        "WNW": "NYNY",
        "NW": "ÉNY",
        "NNW": "ÉNYNY",
    ]
}