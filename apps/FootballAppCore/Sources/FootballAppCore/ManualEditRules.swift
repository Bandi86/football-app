import Foundation

/// A kezdés pontossága a szerződés szerint: pontos idő vagy csak naptári nap.
public enum KickoffPrecision: String, CaseIterable, Sendable {
    case exact
    case dateOnly = "date_only"

    public var label: String {
        switch self {
        case .exact:
            return "Pontos idő"
        case .dateOnly:
            return "Csak dátum"
        }
    }
}

/// A teljes meccsszerkesztő tiszta, UI-független szabályai: melyik szekció
/// látszik, mi a számított `decided_by`, és mi kerül a PUT payloadba.
public enum ManualEditRules {
    /// A kilenc támogatott státusz, a szerződés sorrendjében.
    public static let statuses: [String] = [
        "scheduled",
        "confirmed",
        "live",
        "halftime",
        "finished",
        "postponed",
        "canceled",
        "abandoned",
        "awarded",
    ]

    /// A `date_only` kezdésnél használt helyőrző óra (12:00 UTC).
    public static let dateOnlyPlaceholderHourUTC = 12

    public static func statusLabel(for status: String) -> String {
        AccessibilityLabels.statusLabel(for: status)
    }

    /// Csak ezeknél a státuszoknál van értelme eredményt megadni.
    public static func allowsResult(for status: String) -> Bool {
        status == "finished" || status == "awarded"
    }

    /// A félidő-eredmény csak lezárt meccsnél szerkeszthető; megítélt
    /// (awarded) meccsnél a félidő, hosszabbítás és tizenegyesek rejtve vannak.
    public static func showsHalftime(for status: String) -> Bool {
        status == "finished"
    }

    /// Hosszabbítás csak lezárt, rendes játékidőben döntetlen meccsnél.
    public static func showsExtraTime(for status: String, homeFt: Int?, awayFt: Int?) -> Bool {
        status == "finished" && isDraw(homeFt, awayFt)
    }

    /// Tizenegyesek csak akkor, ha a hosszabbítás is döntetlenre végződött.
    public static func showsPenalties(
        for status: String,
        homeFt: Int?,
        awayFt: Int?,
        homeEt: Int?,
        awayEt: Int?
    ) -> Bool {
        showsExtraTime(for: status, homeFt: homeFt, awayFt: awayFt) && isDraw(homeEt, awayEt)
    }

    public static func isDraw(_ home: Int?, _ away: Int?) -> Bool {
        guard let home, let away else { return false }
        return home == away
    }

    /// A kliens által számított `decided_by`. Lezárt meccsnél a rendes
    /// játékidő döntetlenje önmagában még nem jelent hosszabbítást: a liga
    /// döntetlen `ft` marad, amíg nincs kitöltött hosszabbítás.
    public static func decidedBy(
        status: String,
        homeFt: Int?,
        awayFt: Int?,
        homeEt: Int?,
        awayEt: Int?,
        homePens: Int?,
        awayPens: Int?
    ) -> String? {
        guard allowsResult(for: status), let homeFt, let awayFt else { return nil }
        if status == "awarded" { return "ft" }
        if homeFt != awayFt { return "ft" }
        guard let homeEt, let awayEt else { return "ft" }
        if homeEt != awayEt { return "et" }
        if homePens != nil || awayPens != nil { return "pens" }
        return "et"
    }

    public static func decidedByLabel(_ decidedBy: String?) -> String {
        switch decidedBy {
        case "ft":
            return "Rendes játékidő"
        case "et":
            return "Hosszabbítás"
        case "pens":
            return "Tizenegyesek"
        default:
            return "—"
        }
    }

    /// A megjelenítés időzónája: `date_only` esetén UTC (a naptári nap a
    /// 12:00Z helyőrző UTC-dátuma), egyébként a verseny időzónája.
    public static func kickoffTimeZone(competitionTimezone: String?, precision: String) -> TimeZone {
        if precision == KickoffPrecision.dateOnly.rawValue {
            return .gmt
        }
        return timeZone(identifier: competitionTimezone)
    }

    public static func timeZone(identifier: String?) -> TimeZone {
        guard let identifier, let zone = TimeZone(identifier: identifier) else {
            return .current
        }
        return zone
    }

    /// A dátum UTC-naptári napjához tartozó 12:00 UTC helyőrző.
    public static func dateOnlyPlaceholder(for date: Date) -> Date {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .gmt
        let day = calendar.dateComponents([.year, .month, .day], from: date)
        var components = DateComponents()
        components.year = day.year
        components.month = day.month
        components.day = day.day
        components.hour = dateOnlyPlaceholderHourUTC
        return calendar.date(from: components) ?? date
    }

    /// Üres szöveg → nil; érvénytelen vagy negatív szám → nil.
    public static func parseScore(_ text: String) -> Int? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let value = Int(trimmed), value >= 0 else { return nil }
        return value
    }

    // MARK: - Validáció

    public struct ManualEditValidation: Equatable, Sendable {
        public let errors: [String]
        public let decidedBy: String?

        public var isValid: Bool { errors.isEmpty }

        public init(errors: [String], decidedBy: String?) {
            self.errors = errors
            self.decidedBy = decidedBy
        }
    }

    /// A látható mezők ellenőrzése. A rejtett szekciók értékeit figyelmen
    /// kívül hagyja, így a státuszváltás nem hagy maga után „szellemhibát".
    public static func validate(
        status: String,
        homeFt: String,
        awayFt: String,
        homeHt: String,
        awayHt: String,
        homeEt: String,
        awayEt: String,
        homePens: String,
        awayPens: String,
        note: String
    ) -> ManualEditValidation {
        var errors: [String] = []
        if note.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            errors.append("Az indoklás kötelező.")
        }

        guard allowsResult(for: status) else {
            return ManualEditValidation(errors: errors, decidedBy: nil)
        }

        func requiredScore(_ text: String, label: String) -> Int? {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                errors.append("\(label): kötelező.")
                return nil
            }
            guard let value = Int(trimmed), value >= 0 else {
                errors.append("\(label): egész, nem negatív számot adj meg.")
                return nil
            }
            return value
        }

        func optionalScore(_ text: String, label: String) -> Int? {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty { return nil }
            guard let value = Int(trimmed), value >= 0 else {
                errors.append("\(label): egész, nem negatív számot adj meg.")
                return nil
            }
            return value
        }

        let ftHome = requiredScore(homeFt, label: "Hazai gólok (rendes játékidő)")
        let ftAway = requiredScore(awayFt, label: "Vendég gólok (rendes játékidő)")

        if showsHalftime(for: status) {
            let htHome = optionalScore(homeHt, label: "Hazai gólok (félidő)")
            let htAway = optionalScore(awayHt, label: "Vendég gólok (félidő)")
            if (htHome == nil) != (htAway == nil) {
                errors.append("A félidő eredményét mindkét csapatnál add meg, vagy egyiket sem.")
            }
        }

        if showsExtraTime(for: status, homeFt: ftHome, awayFt: ftAway) {
            let etHome = optionalScore(homeEt, label: "Hazai gólok (hosszabbítás)")
            let etAway = optionalScore(awayEt, label: "Vendég gólok (hosszabbítás)")
            if (etHome == nil) != (etAway == nil) {
                errors.append("A hosszabbítás eredményét mindkét csapatnál add meg, vagy egyiket sem.")
            }

            if showsPenalties(
                for: status,
                homeFt: ftHome,
                awayFt: ftAway,
                homeEt: etHome,
                awayEt: etAway
            ) {
                let pensHome = optionalScore(homePens, label: "Hazai tizenegyesek")
                let pensAway = optionalScore(awayPens, label: "Vendég tizenegyesek")
                let pensGiven = !homePens.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    || !awayPens.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                if !pensGiven {
                    errors.append("Hosszabbítás utáni döntetlennél a tizenegyesek eredménye kötelező.")
                } else if (pensHome == nil) != (pensAway == nil) {
                    errors.append("A tizenegyesek eredményét mindkét csapatnál add meg, vagy egyiket sem.")
                } else if let pensHome, let pensAway, pensHome == pensAway {
                    errors.append("A tizenegyes-párbaj eredménye nem lehet döntetlen.")
                }
            }
        }

        let decided = decidedBy(
            status: status,
            homeFt: ftHome,
            awayFt: ftAway,
            homeEt: parseScore(homeEt),
            awayEt: parseScore(awayEt),
            homePens: parseScore(homePens),
            awayPens: parseScore(awayPens)
        )
        return ManualEditValidation(errors: errors, decidedBy: decided)
    }

    // MARK: - Payload

    /// A szerkesztett mezőkből épített PUT payload. `date_only` esetén a
    /// kezdést a 12:00 UTC helyőrzőre normalizálja; eredményt csak olyan
    /// státusznál küld, ahol az értelmes, és akkor is a teljes blokkot.
    public static func makeRequest(
        status: String,
        kickoffAt: Date,
        kickoffPrecision: String,
        homeFt: String,
        awayFt: String,
        homeHt: String,
        awayHt: String,
        homeEt: String,
        awayEt: String,
        homePens: String,
        awayPens: String,
        note: String
    ) -> ManualEditRequest {
        let normalizedKickoff = kickoffPrecision == KickoffPrecision.dateOnly.rawValue
            ? dateOnlyPlaceholder(for: kickoffAt)
            : kickoffAt
        let trimmedNote = note.trimmingCharacters(in: .whitespacesAndNewlines)
        guard allowsResult(for: status) else {
            return ManualEditRequest(
                note: trimmedNote,
                kickoffAt: normalizedKickoff,
                kickoffPrecision: kickoffPrecision,
                status: status
            )
        }

        let ftHome = parseScore(homeFt)
        let ftAway = parseScore(awayFt)
        let htHome = showsHalftime(for: status) ? parseScore(homeHt) : nil
        let htAway = showsHalftime(for: status) ? parseScore(awayHt) : nil
        let etShown = showsExtraTime(for: status, homeFt: ftHome, awayFt: ftAway)
        let etHome = etShown ? parseScore(homeEt) : nil
        let etAway = etShown ? parseScore(awayEt) : nil
        let pensShown = showsPenalties(
            for: status,
            homeFt: ftHome,
            awayFt: ftAway,
            homeEt: etHome,
            awayEt: etAway
        )
        let pensHome = pensShown ? parseScore(homePens) : nil
        let pensAway = pensShown ? parseScore(awayPens) : nil

        return ManualEditRequest(
            note: trimmedNote,
            kickoffAt: normalizedKickoff,
            kickoffPrecision: kickoffPrecision,
            status: status,
            homeFt: ftHome,
            awayFt: ftAway,
            homeHt: htHome,
            awayHt: htAway,
            homeEt: etHome,
            awayEt: etAway,
            homePens: pensHome,
            awayPens: pensAway,
            decidedBy: decidedBy(
                status: status,
                homeFt: ftHome,
                awayFt: ftAway,
                homeEt: etHome,
                awayEt: etAway,
                homePens: pensHome,
                awayPens: pensAway
            )
        )
    }
}