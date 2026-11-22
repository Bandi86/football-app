import Foundation
import Testing
@testable import FootballAppCore

@Suite struct HeaderMoodTests {
    private var utcCalendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }

    private func dateAt(_ hour: Int) -> Date {
        utcCalendar.date(from: DateComponents(year: 2026, month: 11, day: 22, hour: hour))!
    }

    @Test func dayPhaseCoversAllBoundaries() {
        #expect(DayPhase.phase(at: dateAt(3), calendar: utcCalendar) == .night)
        #expect(DayPhase.phase(at: dateAt(6), calendar: utcCalendar) == .dawn)
        #expect(DayPhase.phase(at: dateAt(9), calendar: utcCalendar) == .morning)
        #expect(DayPhase.phase(at: dateAt(13), calendar: utcCalendar) == .daytime)
        #expect(DayPhase.phase(at: dateAt(17), calendar: utcCalendar) == .golden)
        #expect(DayPhase.phase(at: dateAt(20), calendar: utcCalendar) == .dusk)
        #expect(DayPhase.phase(at: dateAt(23), calendar: utcCalendar) == .night)
    }

    @Test func noWeatherUsesPhasePaletteAndIsNotMarkedLive() {
        let palette = HeaderMood.resolve(phase: .daytime, weather: nil)
        #expect(palette.isLiveWeather == false)
        #expect(palette.conditionLabel == "Nappal")
        #expect(palette.symbolName == "sun.max.fill")
        #expect(palette.particle == .rays)
    }

    @Test func rainOverridesPhaseColoursAndParticle() {
        let weather = CurrentWeather(
            temperatureC: 11.4,
            apparentC: nil,
            humidityPct: 88,
            windSpeedKmh: 14,
            weatherCode: 63,
            isDay: true,
            observedAtUTC: nil
        )
        let palette = HeaderMood.resolve(phase: .golden, weather: weather)
        #expect(palette.isLiveWeather == true)
        #expect(palette.conditionLabel == "Eső")
        #expect(palette.particle == .rain)
        #expect(palette.symbolName == "cloud.rain.fill")
        // Esőben a napszak arany színe nem maradhat meg.
        #expect(palette.accentHex != HeaderMood.resolve(phase: .golden, weather: nil).accentHex)
    }

    @Test func overcastKeepsTimeOfDayColoursButChangesMotion() {
        let weather = CurrentWeather(
            temperatureC: 7, apparentC: nil, humidityPct: 91,
            windSpeedKmh: 9, weatherCode: 3, isDay: true, observedAtUTC: nil
        )
        let golden = HeaderMood.resolve(phase: .golden, weather: weather)
        let plainGolden = HeaderMood.resolve(phase: .golden, weather: nil)
        #expect(golden.topHex == plainGolden.topHex)
        #expect(golden.accentHex == plainGolden.accentHex)
        #expect(golden.particle == .orbs)
        #expect(golden.symbolName == "cloud.fill")
    }

    @Test func thunderstormAtNightUsesNightSymbolAndStormAnimation() {
        let weather = CurrentWeather(
            temperatureC: 21, apparentC: nil, humidityPct: 80,
            windSpeedKmh: 30, weatherCode: 95, isDay: false, observedAtUTC: nil
        )
        let palette = HeaderMood.resolve(phase: .night, weather: weather)
        #expect(palette.conditionLabel == "Zivatar")
        #expect(palette.particle == .storm)
        #expect(palette.symbolName == "cloud.bolt.fill")
    }

    @Test func resolveIsDeterministic() {
        let weather = CurrentWeather(
            temperatureC: 7, apparentC: nil, humidityPct: nil,
            windSpeedKmh: nil, weatherCode: 2, isDay: true, observedAtUTC: nil
        )
        let first = HeaderMood.resolve(phase: .morning, weather: weather)
        let second = HeaderMood.resolve(phase: .morning, weather: weather)
        #expect(first == second)
    }

    @Test func hungarianTemperatureUsesComma() {
        #expect(CurrentWeather.decimalText(12.5) == "12,5")
        #expect(CurrentWeather.decimalText(12) == "12")
    }

    @Test func wmoCodesMapToExpectedKinds() {
        #expect(WeatherKind(code: 0) == .clear)
        #expect(WeatherKind(code: 2) == .partlyCloudy)
        #expect(WeatherKind(code: 48) == .fog)
        #expect(WeatherKind(code: 61) == .rain)
        #expect(WeatherKind(code: 75) == .snow)
        #expect(WeatherKind(code: 99) == .thunderstorm)
        #expect(WeatherKind(code: -1) == .overcast)
        #expect(WeatherKind.rain.hungarianLabel == "Eső")
        #expect(WeatherKind.clear.nightSymbolName == "moon.stars.fill")
    }
}
