import Foundation

/// Üzemeltetési szövegek egyetlen forrása. A szín itt sem hordoz egyedül
/// információt: minden jelhez tartozik magyar szöveg is.
public enum OpsFormatting {
    private static let timestampFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "hu_HU")
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter
    }()

    public static func issueCodeLabel(_ code: IssueCode) -> String {
        switch code {
        case .missingLogo: return "Hiányzó logó"
        case .missingOdds: return "Hiányzó odds"
        case .openPast: return "Lejárt, nyitott"
        case .duplicateSuspect: return "Duplikátum gyanú"
        case .identityReview: return "Identitás-review"
        case .staleStatus: return "Elavult státusz"
        case .sourceConflict: return "Forrásütközés"
        case .other(let raw): return raw.isEmpty ? "Egyéb hiány" : "Egyéb hiány: \(raw)"
        }
    }

    public static func severityLabel(_ severity: IssueSeverity) -> String {
        switch severity {
        case .info: return "Tájékoztatás"
        case .warning: return "Figyelmeztetés"
        case .hard: return "Súlyos"
        case .unknown: return "Ismeretlen súlyosság"
        }
    }

    public static func sourceLabel(_ source: String) -> String {
        switch source {
        case "daily-check": return "Napi ellenőrzés"
        case "integrity": return "Integritás"
        case "manual": return "Kézi javítás"
        case "agent": return "Ügynök"
        default: return source.isEmpty ? "Ismeretlen forrás" : source
        }
    }

    public static func subjectLabel(_ kind: IssueSubjectKind) -> String {
        switch kind {
        case .match: return "Meccs"
        case .team: return "Csapat"
        case .competition: return "Verseny"
        case .unknown: return "Ismeretlen tárgy"
        }
    }

    public static func noteKindLabel(_ kind: String) -> String {
        switch kind {
        case "info": return "Információ"
        case "problem": return "Probléma"
        case "fix": return "Javítás"
        case "question": return "Kérdés"
        case "resolved": return "Megoldva"
        default: return kind.isEmpty ? "Jegyzet" : kind
        }
    }

    /// Első/utolsó láttatás magyar időbélyege (a nyers UTC helyett).
    public static func timestamp(_ date: Date) -> String {
        timestampFormatter.string(from: date)
    }
}
