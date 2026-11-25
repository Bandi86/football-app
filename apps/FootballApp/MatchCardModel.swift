import Foundation
import FootballAppCore

struct MatchOdds {
    let home: Double?
    let draw: Double?
    let away: Double?
    let bttsYes: Double?
    let bttsNo: Double?
    let over25: Double?
    let under25: Double?
}

enum Outcome {
    case home, draw, away
}

/// A kártyán megjelenő végeredmény.
struct MatchScore: Equatable {
    let home: Int
    let away: Int

    var text: String { "\(home) – \(away)" }
}

/// Egy 1X2-es odds-pill a kártyán (a nyertes kimenetel kiemelhető).
struct OddsPill: Identifiable, Equatable {
    let label: String
    let value: String
    let isWinner: Bool

    var id: String { label }
}

/// A kártya display-modellje: valós API-adatból és mintaadatból is építhető.
struct MatchCardModel: Identifiable {
    let id: String
    let home: String
    let away: String
    /// A kezdés abszolút ideje (a hiány-szűrő „lejárt, nincs eredmény" döntéséhez).
    let kickoffAt: Date
    /// A kezdés napja magyar rövid formában (pl. „nov. 20.").
    let dayLabel: String
    let time: String
    /// „exact" vagy „date_only" — utóbbinál a kezdés ideje still placeholder.
    let kickoffPrecision: String
    /// A forduló nyers címkéje az API-ból (pl. „17", „1/8", „final").
    let roundLabel: String?
    let status: String
    let publishState: PublishState
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
    let homeLogo: String?
    let awayLogo: String?
    /// A verseny logója (a „ligalogó nélkül" hiány-szűrőhöz kell a kártyán is).
    let competitionLogo: String?
    let homeElo: Int?
    let awayElo: Int?

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

    /// A kártyán látható végeredmény a domain `final_score` logikája szerint:
    /// hosszabbításnál a 120' állása, különben a rendes játékidő állása.
    /// Tizenegyesek nem számítanak bele, és hiányzó állásnál nincs eredmény.
    var finalScore: MatchScore? {
        Self.finalScore(homeScore: homeScore, awayScore: awayScore,
                        homeEt: homeEt, awayEt: awayEt)
    }

    /// A `finalScore` közös magja: a `DayMatch`-alapú szűrő is ezt használja,
    /// hogy a hiányzó eredmény döntése ne duplikálódjon két helyen.
    static func finalScore(homeScore: Int?, awayScore: Int?,
                           homeEt: Int?, awayEt: Int?) -> MatchScore? {
        if let home = homeEt, let away = awayEt {
            return MatchScore(home: home, away: away)
        }
        if let home = homeScore, let away = awayScore {
            return MatchScore(home: home, away: away)
        }
        return nil
    }

    // MARK: - Hiány- és élő-állapot szabályai

    /// „Lejárt, de nyitott": ennyi idő után számít a kezdés lejártnak
    /// (a szerver `open_past` definíciója). Legyen nevesített konstans, mert a
    /// szűrő és a hiány-panel ugyanezt a határt használja.
    static let openPastThreshold: TimeInterval = 3 * 60 * 60

    /// Az `open_past` döntéshez tartozó nyitott státuszok.
    static let openStatuses: Set<String> = ["scheduled", "confirmed", "live", "halftime"]

    /// Az „élő" felső időkorlátja: 105 perc játék (45 + 15 + 45), legfeljebb
    /// 30 perc hosszabbítással + 15 perc tartalék = 150 perc. Ennél régebbi
    /// `live` státusz nem élő meccs, hanem elavult — eredmény várható.
    static let liveWindow: TimeInterval = 150 * 60

    /// Hiányzó eredmény: lezárt (`finished`) meccs, amelynek nincs végeredménye.
    /// Ez nem azonos a lejárt, de nyitott meccsekkel.
    static func isMissingResult(status: String, finalScore: MatchScore?) -> Bool {
        status == "finished" && finalScore == nil
    }

    /// Lejárt, de nyitott: a kezdés több mint 3 órával ezelőtt volt, és a
    /// meccs státusza még nyitott — a hiány-panel `open_past` fogalma.
    static func isOpenPast(status: String, kickoffAt: Date, now: Date) -> Bool {
        guard openStatuses.contains(status) else { return false }
        return kickoffAt < now.addingTimeInterval(-openPastThreshold)
    }

    /// Ténylegesen élő: élő/félidős státusz, és a kezdés óta legfeljebb 6 óra
    /// telt el. Az ennél régebbi „élő" sor a backend elavult státusza.
    static func isLiveNow(status: String, kickoffAt: Date, now: Date) -> Bool {
        guard ["live", "halftime"].contains(status) else { return false }
        return now.timeIntervalSince(kickoffAt) <= liveWindow
    }

    /// A kártya eredmény-oszlopa: hiányzó eredménynél „–", soha nem 0–0.
    var scoreText: String { finalScore?.text ?? "–" }

    /// Ténylegesen élő: a státusz élő/félidős, és a kezdés óta legfeljebb
    /// `liveWindow` telt el. Az ennél régebbi „élő" sor a backend elavult
    /// státusza, ilyenkor a kártya a szokásos kezdés/állapot kijelzést mutatja.
    var isLive: Bool { Self.isLiveNow(status: status, kickoffAt: kickoffAt, now: Date()) }

    var isFinished: Bool { ["finished", "awarded"].contains(status) }

    /// A meccsnek régen vége kellett volna lenni, de a forrás még mindig
    /// `live`/`halftime`-t mond és nincs eredmény — ilyenkor nem „ÉLŐ"-t
    /// mutatunk, hanem azt, hogy az eredmény még várható (S130).
    var isResultPending: Bool {
        guard ["live", "halftime"].contains(status), !isLive,
              hasExactKickoff, finalScore == nil else { return false }
        return Date().timeIntervalSince(kickoffAt) > Self.liveWindow
    }

    /// A kezdés alatti kis állapotjelzés („ÉLŐ" / „FT" / eredmény várható).
    var phaseLabel: String? {
        if isLive { return "ÉLŐ" }
        if isResultPending { return "EREDMÉNY VÁRHATÓ" }
        if isFinished { return "FT" }
        return nil
    }

    /// Az élő minutaszám; csak ténylegesen élő meccsnél jelenik meg.
    var minuteText: String? { isLive ? minute.map { "\($0)'" } : nil }

    /// Élő meccs fázisa és játékideje, a kezdésből számolva (45 + 15 + 45,
    /// majd várakozás az eredményre). Ha a forrás adott minute-ot, azt használjuk.
    /// A `phaseLabel` ("ÉLŐ") változatlan marad — ez csak a részletesebb jelzés.
    var livePhaseText: String? {
        if isResultPending { return "Várakozás az eredményre" }
        guard isLive else { return nil }
        // A research pontos játékideje, ha rendelkezésre áll.
        if let m = minute {
            if m <= 45 { return "1. félidő · \(m)'" }
            if m <= 90 { return "2. félidő · \(m)'" }
            return "Hosszabbítás · \(m)'"
        }
        guard hasExactKickoff else { return nil }
        let elapsedMin = max(0, Int(Date().timeIntervalSince(kickoffAt) / 60))
        if elapsedMin < 45 { return "1. félidő · \(elapsedMin)'" }
        if elapsedMin < 60 { return "Félidő · 15' szünet" }
        if elapsedMin < 105 { return "2. félidő · \(elapsedMin - 15)'" }
        return "Várakozás az eredményre"
    }

    /// A kijelzett állapot: elavult `live`/`halftime` státusznál a kártya a
    /// szokásos, nem-élő kezdés-kijelzést mutatja, így a felolvasott címke sem
    /// mondhat „Élő"-t — ilyenkor „Ütemezett"-ként olvassuk fel.
    var displayStatus: String {
        if ["live", "halftime"].contains(status) && !isLive { return "result_pending" }
        return status
    }

    /// Csak pontos kezdésnél mutatunk időt; a `date_only` sorok placeholder
    /// időt kaptak az ingesttől, azt nem szabad valódi kezdésként kijelezni.
    var hasExactKickoff: Bool { kickoffPrecision != "date_only" }

    var timeDisplay: String { hasExactKickoff ? time : "" }

    /// A kezdés-oszlop második sora, ha nincs pontos idő.
    var kickoffNote: String? { hasExactKickoff ? nil : "időpont egyeztetés alatt" }

    /// A nyers fordulócímke egységesített kulcsa: ékezet- és kis/nagybetű-
    /// független, trimmelt alak. `nil`, ha nincs használható címke.
    static func foldedRoundLabel(_ raw: String?) -> String? {
        guard let raw = raw?.trimmingCharacters(in: .whitespacesAndNewlines),
              !raw.isEmpty else { return nil }
        return raw.folding(options: [.diacriticInsensitive, .caseInsensitive],
                           locale: Locale(identifier: "hu_HU"))
            .trimmingCharacters(in: .whitespaces)
    }

    /// Tisztán női-markert tartalmazó címke-e (pl. „női", „women", „(W)"):
    /// ez nem forduló, a forduló-feliratoknál hiányzónak számít.
    static func isWomensOnlyLabel(_ foldedLabel: String) -> Bool {
        let compact = foldedLabel.filter { $0.isLetter || $0.isNumber }
        return ["noi", "women", "w"].contains(compact)
    }

    /// Forduló-felirat: a numerikus címkéből „17. forduló", a kupa-kódokból
    /// magyar elnevezés, minden másból a nyers címke. A csoportszintű
    /// összesítés (`LeagueGroup.roundSummary`) is ezt a mappinget használja.
    static func roundText(for rawLabel: String?) -> String? {
        guard let raw = rawLabel?.trimmingCharacters(in: .whitespacesAndNewlines),
              !raw.isEmpty,
              let folded = foldedRoundLabel(raw),
              !isWomensOnlyLabel(folded) else { return nil }
        if folded.allSatisfy({ $0.isNumber }) { return "\(folded). forduló" }
        switch folded {
        case "final": return "Döntő"
        case "1/2", "semi final", "semi-finals", "elődöntő": return "Elődöntő"
        case "1/4", "quarter final", "quarter-final", "negyeddöntő": return "Negyeddöntő"
        case "1/8", "round of 16", "nyolcaddöntő": return "Nyolcaddöntő"
        case "1/16", "round of 32": return "Tizenhatoddöntő"
        case "1st round", "round 1", "1. forduló": return "1. forduló"
        default:
            if folded.hasPrefix("group stage") { return "Csoportkör" }
            return raw
        }
    }

    /// A kártya forduló-felirata a nyers címkéből.
    var roundText: String? { Self.roundText(for: roundLabel) }

    /// Csak az 1X2 piac látszik a kártyán; ha nincs egyik odds sem, a blokk rejtve.
    var hasOdds: Bool {
        guard let odds else { return false }
        return odds.home != nil || odds.draw != nil || odds.away != nil
    }

    var oddsPills: [OddsPill] {
        guard let odds else { return [] }
        return [
            OddsPill(label: "1", value: Self.oddsText(odds.home), isWinner: winner1x2 == .home),
            OddsPill(label: "X", value: Self.oddsText(odds.draw), isWinner: winner1x2 == .draw),
            OddsPill(label: "2", value: Self.oddsText(odds.away), isWinner: winner1x2 == .away),
        ]
    }

    var oddsAccessibilityText: String? {
        guard hasOdds else { return nil }
        return oddsPills.map { "\($0.label): \($0.value)" }.joined(separator: ", ")
    }

    private static func oddsText(_ value: Double?) -> String {
        value.map { String(format: "%.2f", $0).replacingOccurrences(of: ".", with: ",") } ?? "–"
    }

    // MARK: - Kezdés formázása

    private static func makeFormatter(_ dateFormat: String, timeZone: TimeZone) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.timeZone = timeZone
        formatter.dateFormat = dateFormat
        return formatter
    }

    private static let dayFormatter = makeFormatter("MMM d.", timeZone: .current)
    private static let timeFormatter = makeFormatter("HH:mm", timeZone: .current)

    /// Magyar rövid dátum (pl. „nov. 20."); a `timeZone` a tesztelhetőség miatt
    /// paraméter, az app az aktuális időzónát használja.
    static func dayLabel(for kickoffAt: Date, timeZone: TimeZone = .current) -> String {
        if timeZone == .current { return dayFormatter.string(from: kickoffAt) }
        return makeFormatter("MMM d.", timeZone: timeZone).string(from: kickoffAt)
    }

    /// Helyi kezdési idő „HH:mm" formában, monospaced megjelenítéshez.
    static func timeLabel(for kickoffAt: Date, timeZone: TimeZone = .current) -> String {
        if timeZone == .current { return timeFormatter.string(from: kickoffAt) }
        return makeFormatter("HH:mm", timeZone: timeZone).string(from: kickoffAt)
    }

    static func from(_ match: DayMatch) -> MatchCardModel {
        MatchCardModel(
            id: match.id.uuidString,
            home: match.home,
            away: match.away,
            kickoffAt: match.kickoffAt,
            dayLabel: dayLabel(for: match.kickoffAt),
            time: timeLabel(for: match.kickoffAt),
            kickoffPrecision: match.kickoffPrecision ?? "exact",
            roundLabel: match.roundLabel,
            status: match.status,
            publishState: match.publishState,
            homeScore: match.homeScore,
            awayScore: match.awayScore,
            homeHt: match.homeHt,
            awayHt: match.awayHt,
            homeEt: match.homeEt,
            awayEt: match.awayEt,
            homePens: match.homePens,
            awayPens: match.awayPens,
            minute: match.minute,
            odds: match.odds.map {
                MatchOdds(home: $0.home, draw: $0.draw, away: $0.away,
                          bttsYes: $0.bttsYes, bttsNo: $0.bttsNo,
                          over25: $0.over25, under25: $0.under25)
            },
            markets: match.marketCount ?? 0,
            oddsSource: match.oddsSource ?? "–",
            homePosition: match.homePosition,
            awayPosition: match.awayPosition,
            homeLogo: match.homeLogo,
            awayLogo: match.awayLogo,
            competitionLogo: match.competitionLogo,
            homeElo: match.homeElo.map { Int($0.rounded()) },
            awayElo: match.awayElo.map { Int($0.rounded()) }
        )
    }

    static func from(_ match: SampleMatch) -> MatchCardModel {
        MatchCardModel(
            id: match.id, home: match.home, away: match.away, kickoffAt: Date(),
            dayLabel: "", time: match.time,
            kickoffPrecision: "exact", roundLabel: nil,
            status: match.status, publishState: .unknown, homeScore: match.homeScore, awayScore: match.awayScore,
            homeHt: match.homeHt, awayHt: match.awayHt, homeEt: match.homeEt, awayEt: match.awayEt,
            homePens: match.homePens, awayPens: match.awayPens, minute: match.minute,
            odds: match.odds, markets: match.markets, oddsSource: SampleData.oddsSource,
            homePosition: SampleData.position(of: match.home),
            awayPosition: SampleData.position(of: match.away),
            homeLogo: nil,
            awayLogo: nil,
            competitionLogo: nil,
            homeElo: nil,
            awayElo: nil
        )
    }
}