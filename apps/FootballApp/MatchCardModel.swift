import Foundation
import FootballAppCore

struct MatchOdds {
    let home: Double
    let draw: Double
    let away: Double
    let bttsYes: Double
    let bttsNo: Double
    let over25: Double
    let under25: Double
}

enum Outcome {
    case home, draw, away
}

/// A kártya display-modellje: valós API-adatból és mintaadatból is építhető.
struct MatchCardModel: Identifiable {
    let id: String
    let home: String
    let away: String
    let time: String
    let status: String
    let homeScore: Int?
    let awayScore: Int?
    let homeHt: Int?
    let awayHt: Int?
    let homeEt: Int?
    let awayEt: Int?
    let homePens: Int?
    let awayPens: Int?
    let minute: Int?
    let odds: MatchOdds?
    let markets: Int
    let oddsSource: String
    let homePosition: Int?
    let awayPosition: Int?

    var winner1x2: Outcome? {
        guard status == "finished", let home = homeScore, let away = awayScore else { return nil }
        if home > away { return .home }
        if home < away { return .away }
        return .draw
    }

    var bttsYesWon: Bool? {
        guard status == "finished", let home = homeScore, let away = awayScore else { return nil }
        return home > 0 && away > 0
    }

    var over25Won: Bool? {
        guard status == "finished", let home = homeScore, let away = awayScore else { return nil }
        return home + away >= 3
    }

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()

    static func from(_ match: DayMatch) -> MatchCardModel {
        MatchCardModel(
            id: match.id.uuidString,
            home: match.home,
            away: match.away,
            time: timeFormatter.string(from: match.kickoffAt),
            status: match.status,
            homeScore: match.homeScore,
            awayScore: match.awayScore,
            homeHt: match.homeHt,
            awayHt: match.awayHt,
            homeEt: match.homeEt,
            awayEt: match.awayEt,
            homePens: match.homePens,
            awayPens: match.awayPens,
            minute: nil,
            odds: match.odds.map {
                MatchOdds(home: $0.home, draw: $0.draw, away: $0.away,
                          bttsYes: $0.bttsYes, bttsNo: $0.bttsNo,
                          over25: $0.over25, under25: $0.under25)
            },
            markets: match.marketCount ?? 0,
            oddsSource: match.oddsSource ?? "–",
            homePosition: match.homePosition,
            awayPosition: match.awayPosition
        )
    }

    static func from(_ match: SampleMatch) -> MatchCardModel {
        MatchCardModel(
            id: match.id, home: match.home, away: match.away, time: match.time,
            status: match.status, homeScore: match.homeScore, awayScore: match.awayScore,
            homeHt: match.homeHt, awayHt: match.awayHt, homeEt: match.homeEt, awayEt: match.awayEt,
            homePens: match.homePens, awayPens: match.awayPens, minute: match.minute,
            odds: match.odds, markets: match.markets, oddsSource: SampleData.oddsSource,
            homePosition: SampleData.position(of: match.home),
            awayPosition: SampleData.position(of: match.away)
        )
    }
}