import Foundation

struct SampleLeague: Identifiable {
    let id: String
    let name: String
    let country: String
    let flag: String
    let tier: Int
    let priority: Int
    let matches: [SampleMatch]

    var divisionLabel: String {
        switch tier {
        case 1: return "1. osztály"
        case 2: return "2. osztály"
        default: return "Kupa"
        }
    }

    func filtering(_ filter: MatchFilter) -> SampleLeague? {
        let kept = matches.filter { filter.matches(status: $0.status) }
        guard !kept.isEmpty else { return nil }
        return SampleLeague(id: id, name: name, country: country, flag: flag, tier: tier, priority: priority, matches: kept)
    }
}

struct SampleMatch: Identifiable {
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

    init(
        id: String,
        home: String,
        away: String,
        time: String,
        status: String,
        homeScore: Int? = nil,
        awayScore: Int? = nil,
        homeHt: Int? = nil,
        awayHt: Int? = nil,
        homeEt: Int? = nil,
        awayEt: Int? = nil,
        homePens: Int? = nil,
        awayPens: Int? = nil,
        minute: Int? = nil,
        odds: MatchOdds? = nil,
        markets: Int = 18
    ) {
        self.id = id
        self.home = home
        self.away = away
        self.time = time
        self.status = status
        self.homeScore = homeScore
        self.awayScore = awayScore
        self.homeHt = homeHt
        self.awayHt = awayHt
        self.homeEt = homeEt
        self.awayEt = awayEt
        self.homePens = homePens
        self.awayPens = awayPens
        self.minute = minute
        self.odds = odds
        self.markets = markets
    }

    // Nyerő kimenetek (csak lezárt meccsnél), az FT eredményből.
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
}

/// Fake adat a lista-design teszteléséhez. A ligák prioritás-sávokban:
/// 1) a top-5 liga + NB I első osztálya, 2) ezen országok alsóbb osztályai.
enum SampleData {
    static let leagues: [SampleLeague] = raw.sorted {
        $0.priority != $1.priority ? $0.priority < $1.priority : $0.country < $1.country
    }

    static var totalMatches: Int { leagues.reduce(0) { $0 + $1.matches.count } }
    static var allMatches: [SampleMatch] { leagues.flatMap { $0.matches } }
    static var countries: [String] { Array(Set(leagues.map(\.country))).sorted() }
    static let oddsSource = "Tipplap"

    /// Bajnoki tabella-helyezések (minta) csapatnevenként.
    static let positions: [String: Int] = [
        "Arsenal": 1, "Chelsea": 5, "Manchester City": 2, "Liverpool": 3, "Newcastle": 6, "Tottenham": 7,
        "Real Madrid": 1, "Barcelona": 2, "Atlético Madrid": 4, "Sevilla": 9, "Betis": 7, "Valencia": 12,
        "Inter": 1, "Juventus": 3, "Milan": 2, "Napoli": 5,
        "Bayern München": 1, "Dortmund": 4, "RB Leipzig": 3, "Leverkusen": 2,
        "PSG": 1, "Marseille": 3,
        "Ferencváros": 1, "Újpest": 8, "Debrecen": 4, "Győr": 6,
        "Leeds United": 2, "West Brom": 9, "Eibar": 4, "Levante": 6,
        "Palermo": 5, "Sampdoria": 8, "Hamburger SV": 3, "Schalke 04": 7,
        "Saint-Étienne": 5, "Bordeaux": 10, "Vasas": 4, "Szentlőrinc": 12,
        "Manchester United": 6,
    ]

    static func position(of team: String) -> Int? { positions[team] }

    private static func o(_ h: Double, _ d: Double, _ a: Double, _ by: Double, _ bn: Double, _ ov: Double, _ un: Double) -> MatchOdds {
        MatchOdds(home: h, draw: d, away: a, bttsYes: by, bttsNo: bn, over25: ov, under25: un)
    }

    private static let raw: [SampleLeague] = [
        // ── 1. sáv: top-5 liga + NB I, első osztály ──
        SampleLeague(id: "pl", name: "Premier League", country: "Anglia", flag: "🏴󠁧󠁢󠁥󠁮󠁧󠁿", tier: 1, priority: 1, matches: [
            SampleMatch(id: "pl1", home: "Arsenal", away: "Chelsea", time: "16:00", status: "finished", homeScore: 2, awayScore: 1, homeHt: 1, awayHt: 0, odds: o(1.85, 3.40, 4.20, 1.72, 2.05, 2.10, 1.75)),
            SampleMatch(id: "pl2", home: "Manchester City", away: "Liverpool", time: "17:30", status: "live", homeScore: 1, awayScore: 1, homeHt: 1, awayHt: 0, minute: 63, odds: o(2.10, 3.20, 3.40, 1.80, 1.95, 1.85, 1.95)),
            SampleMatch(id: "pl3", home: "Newcastle", away: "Tottenham", time: "18:30", status: "scheduled", odds: o(2.60, 3.30, 2.70, 1.90, 1.90, 2.05, 1.78)),
        ]),
        SampleLeague(id: "ll", name: "La Liga", country: "Spanyolország", flag: "🇪🇸", tier: 1, priority: 2, matches: [
            SampleMatch(id: "ll1", home: "Real Madrid", away: "Barcelona", time: "21:00", status: "finished", homeScore: 3, awayScore: 2, homeHt: 2, awayHt: 1, odds: o(2.30, 3.50, 3.00, 1.55, 2.35, 1.70, 2.15)),
            SampleMatch(id: "ll2", home: "Atlético Madrid", away: "Sevilla", time: "19:00", status: "live", homeScore: 0, awayScore: 0, homeHt: 0, awayHt: 0, minute: 12, odds: o(1.70, 3.60, 5.00, 2.10, 1.70, 2.30, 1.62)),
            SampleMatch(id: "ll3", home: "Betis", away: "Valencia", time: "20:30", status: "scheduled", odds: o(2.40, 3.20, 2.90, 1.85, 1.95, 2.20, 1.65)),
        ]),
        SampleLeague(id: "sa", name: "Serie A", country: "Olaszország", flag: "🇮🇹", tier: 1, priority: 3, matches: [
            SampleMatch(id: "sa1", home: "Inter", away: "Juventus", time: "18:00", status: "scheduled", odds: o(2.05, 3.25, 3.60, 1.95, 1.85, 2.25, 1.62)),
            SampleMatch(id: "sa2", home: "Milan", away: "Napoli", time: "20:45", status: "scheduled", odds: o(2.50, 3.30, 2.75, 1.80, 2.00, 2.05, 1.78)),
        ]),
        SampleLeague(id: "bl", name: "Bundesliga", country: "Németország", flag: "🇩🇪", tier: 1, priority: 4, matches: [
            SampleMatch(id: "bl1", home: "Bayern München", away: "Dortmund", time: "17:30", status: "finished", homeScore: 4, awayScore: 2, homeHt: 2, awayHt: 1, odds: o(1.65, 4.00, 4.60, 1.45, 2.60, 1.55, 2.40)),
            SampleMatch(id: "bl2", home: "RB Leipzig", away: "Leverkusen", time: "18:30", status: "scheduled", odds: o(2.70, 3.40, 2.55, 1.75, 2.05, 1.95, 1.88)),
        ]),
        SampleLeague(id: "l1", name: "Ligue 1", country: "Franciaország", flag: "🇫🇷", tier: 1, priority: 5, matches: [
            SampleMatch(id: "l1a", home: "PSG", away: "Marseille", time: "21:00", status: "scheduled", odds: o(1.55, 4.20, 5.50, 1.60, 2.30, 1.75, 2.05)),
        ]),
        SampleLeague(id: "nb1", name: "NB I", country: "Magyarország", flag: "🇭🇺", tier: 1, priority: 6, matches: [
            SampleMatch(id: "nb1a", home: "Ferencváros", away: "Újpest", time: "19:00", status: "live", homeScore: 1, awayScore: 0, homeHt: 1, awayHt: 0, minute: 78, odds: o(1.45, 4.30, 6.00, 2.00, 1.80, 2.40, 1.55)),
            SampleMatch(id: "nb1b", home: "Debrecen", away: "Győr", time: "17:00", status: "scheduled", odds: o(2.20, 3.30, 3.20, 1.85, 1.95, 2.15, 1.70)),
        ]),
        // ── 2. sáv: ugyanezen országok alsóbb osztályai ──
        SampleLeague(id: "eng2", name: "Championship", country: "Anglia", flag: "🏴󠁧󠁢󠁥󠁮󠁧󠁿", tier: 2, priority: 7, matches: [
            SampleMatch(id: "eng2a", home: "Leeds United", away: "West Brom", time: "20:45", status: "scheduled", odds: o(1.95, 3.30, 3.90, 1.90, 1.90, 2.10, 1.72)),
        ]),
        SampleLeague(id: "esp2", name: "LaLiga 2", country: "Spanyolország", flag: "🇪🇸", tier: 2, priority: 8, matches: [
            SampleMatch(id: "esp2a", home: "Eibar", away: "Levante", time: "21:00", status: "scheduled", odds: o(2.30, 3.10, 3.10, 1.88, 1.92, 2.20, 1.65)),
        ]),
        SampleLeague(id: "ita2", name: "Serie B", country: "Olaszország", flag: "🇮🇹", tier: 2, priority: 9, matches: [
            SampleMatch(id: "ita2a", home: "Palermo", away: "Sampdoria", time: "20:30", status: "scheduled", odds: o(2.45, 3.15, 2.95, 1.85, 1.95, 2.15, 1.68)),
        ]),
        SampleLeague(id: "ger2", name: "2. Bundesliga", country: "Németország", flag: "🇩🇪", tier: 2, priority: 10, matches: [
            SampleMatch(id: "ger2a", home: "Hamburger SV", away: "Schalke 04", time: "18:30", status: "scheduled", odds: o(2.15, 3.40, 3.25, 1.80, 2.00, 2.05, 1.78)),
        ]),
        SampleLeague(id: "fra2", name: "Ligue 2", country: "Franciaország", flag: "🇫🇷", tier: 2, priority: 11, matches: [
            SampleMatch(id: "fra2a", home: "Saint-Étienne", away: "Bordeaux", time: "20:00", status: "scheduled", odds: o(2.35, 3.20, 3.05, 1.85, 1.95, 2.10, 1.72)),
        ]),
        SampleLeague(id: "nb2", name: "NB II", country: "Magyarország", flag: "🇭🇺", tier: 2, priority: 12, matches: [
            SampleMatch(id: "nb2a", home: "Vasas", away: "Szentlőrinc", time: "17:00", status: "scheduled", odds: o(1.80, 3.50, 4.10, 1.95, 1.85, 2.30, 1.62)),
        ]),
        // ── Kupa (hosszabbítás + 11-es) ──
        SampleLeague(id: "facup", name: "FA Kupa", country: "Anglia", flag: "🏴󠁧󠁢󠁥󠁮󠁧󠁿", tier: 0, priority: 13, matches: [
            SampleMatch(id: "facup1", home: "Manchester United", away: "Liverpool", time: "16:30", status: "finished", homeScore: 1, awayScore: 1, homeHt: 1, awayHt: 0, homeEt: 2, awayEt: 2, homePens: 4, awayPens: 3, odds: o(2.40, 3.30, 2.80, 1.50, 2.45, 1.80, 1.95)),
        ]),
    ]
}
