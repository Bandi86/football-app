use uuid::Uuid;

pub const FUZZY_THRESHOLD: f64 = 0.6;

pub fn normalize_name(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Ékezet-független, kisbetűs alak — crosswalk-seed összehasonlításhoz.
/// A linker kulcsa a `normalize_name` (lower), ezért a seed mindkét alakot
/// eltárolja, ha a forrás neve ékezetes.
pub fn fold_name(s: &str) -> String {
    normalize_name(s)
        .chars()
        .filter_map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ą' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' | 'ę' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' | 'ı' => Some('i'),
            'ó' | 'ò' | 'ô' | 'ö' | 'ő' | 'õ' | 'ø' => Some('o'),
            'ú' | 'ù' | 'û' | 'ü' | 'ű' => Some('u'),
            'ç' | 'ć' | 'č' => Some('c'),
            'ñ' | 'ń' => Some('n'),
            'š' | 'ś' | 'ş' => Some('s'),
            'ž' | 'ź' | 'ż' => Some('z'),
            'ý' | 'ÿ' => Some('y'),
            'đ' => Some('d'),
            'ł' => Some('l'),
            'ř' => Some('r'),
            'ť' => Some('t'),
            'ğ' => Some('g'),
            // A török nagy İ kisbetűsítése "i" + combining dot lesz.
            '\u{0307}' => None,
            other => Some(other),
        })
        .collect()
}

/// Crosswalk-seed: egy reviewed alias sor a kanonikus entitás kontextusával.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasSeed {
    pub entity_id: Uuid,
    pub alias: String,
    pub lang: String,
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityContext {
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
}

fn field_matches(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        _ => true,
    }
}

/// Brit tagország-kódok (GB-WLS/GB-SCT/GB-ENG/GB-NIR) a GB-vel egyenértékűek
/// az identitás-kontextusban: a v1 import alrégió-kódot ad, a footballdata
/// GB-t — azonos csapatnév mellett ez nem jelenthet külön entitást.
///
/// Ugyanez a helyzet az ISO3 ↔ ISO2 kódokkal (S37a): a v1-import háromjegyű
/// kódot hoz (`GER`, `ESP`, `ITA`, `FRA`, `HUN`, `NED`, `POR`), az új források
/// kétjegyűt (`DE`, `ES`, `IT`, `FR`, `HU`, `NL`, `PT`). A `core.country`
/// táblában mindkét kód él, ezért a kontextus-egyeztetésnek kód-függetlennek
/// kell lennie, különben az egzakt névvel megtalált csapat is „ismeretlen”
/// marad. Az `ENG` szándékosan NEM képeződik `GB`-re: az angol és a brit
/// (jelenleg skótként nevezett) sor két külön ország-rekord a v2-ben.
pub fn normalize_country_code(code: &str) -> String {
    let upper = code.trim().to_ascii_uppercase();
    if upper.starts_with("GB-") {
        return "GB".to_string();
    }
    let iso2 = match upper.as_str() {
        "FRA" => "FR",
        "GER" | "DEU" => "DE",
        "HUN" => "HU",
        "ITA" => "IT",
        "ESP" => "ES",
        "NED" | "NLD" => "NL",
        "POR" | "PRT" => "PT",
        "SUI" | "CHE" => "CH",
        "SWE" => "SE",
        "DEN" | "DNK" => "DK",
        "NOR" => "NO",
        "CRO" | "HRV" => "HR",
        "SRB" => "RS",
        "POL" => "PL",
        "GRE" | "GRC" => "GR",
        "TUR" => "TR",
        "BEL" => "BE",
        "AUT" => "AT",
        "CZE" => "CZ",
        "SVK" => "SK",
        "SVN" => "SI",
        "UKR" => "UA",
        "RUS" => "RU",
        "ISR" => "IL",
        "ARG" => "AR",
        "BRA" => "BR",
        "URU" | "URY" => "UY",
        "COL" => "CO",
        "CHL" => "CL",
        "MEX" => "MX",
        "USA" => "US",
        "JPN" => "JP",
        "KOR" => "KR",
        "CHN" => "CN",
        "AUS" => "AU",
        "IRN" => "IR",
        "IRQ" => "IQ",
        "SAU" => "SA",
        "QAT" => "QA",
        "EGY" => "EG",
        "MAR" => "MA",
        "ALG" | "DZA" => "DZ",
        "TUN" => "TN",
        "RSA" | "ZAF" => "ZA",
        "NGA" => "NG",
        "GHA" => "GH",
        other => other,
    };
    iso2.to_string()
}

fn country_matches(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => normalize_country_code(x) == normalize_country_code(y),
        _ => true,
    }
}

pub fn context_matches(a: &IdentityContext, b: &IdentityContext) -> bool {
    country_matches(&a.country_code, &b.country_code)
        && field_matches(&a.gender, &b.gender)
        && field_matches(&a.age_group, &b.age_group)
        && field_matches(&a.branch, &b.branch)
}

pub fn token_overlap(a: &str, b: &str) -> f64 {
    let ta: std::collections::HashSet<String> = normalize_name(a)
        .split(' ')
        .map(String::from)
        .filter(|t| !t.is_empty())
        .collect();
    let tb: std::collections::HashSet<String> = normalize_name(b)
        .split(' ')
        .map(String::from)
        .filter(|t| !t.is_empty())
        .collect();
    let union = ta.union(&tb).count();
    if union == 0 {
        return 0.0;
    }
    ta.intersection(&tb).count() as f64 / union as f64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveMethod {
    ExactExternalId,
    CrosswalkChain,
    ReviewedAlias,
    ExactName,
    ManualReview,
}

impl ResolveMethod {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::ExactExternalId => "exact_external_id",
            Self::CrosswalkChain => "crosswalk_chain",
            Self::ReviewedAlias => "reviewed_alias",
            Self::ExactName => "exact_name",
            Self::ManualReview => "manual_review",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub entity_id: Uuid,
    pub score: f64,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub enum Resolution {
    Exact {
        entity_id: Uuid,
        method: ResolveMethod,
    },
    Candidates {
        candidates: Vec<Candidate>,
        queue_id: Uuid,
    },
    NotFound {
        queue_id: Uuid,
    },
}

/// Klub-szuffixek, amelyeket a név-hasonlítás előtt elhagyunk (v1
/// `link_names` mintája): a források hol kiírják, hol nem.
const CLUB_TOKENS: [&str; 19] = [
    "fc", "cf", "sc", "afc", "club", "cd", "ud", "sv", "ac", "as", "ss", "us", "ca", "fk", "sk",
    "bk", "if", "calcio", "cfc",
];

fn name_tokens(name: &str) -> Vec<String> {
    fold_name(name)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty() && !CLUB_TOKENS.contains(t))
        .map(String::from)
        .collect()
}

fn token_equal(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    // Rövidítés: legalább 5 karakteres tő, ami a másik előtagja.
    if short.len() >= 5 && long.starts_with(short) {
        return true;
    }
    // Egy elgéppelés (pl. „tbilisi" ↔ „tbiliszi"), csak hosszú tokeneken.
    if long.len() >= 6 && short.len() + 1 == long.len() {
        return edit_distance_one(short, long);
    }
    false
}

fn edit_distance_one(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        if a.len() > b.len() {
            i += 1;
        } else {
            j += 1;
        }
    }
    edits + (a.len() - i) + (b.len() - j) <= 1
}

/// Két forrás-csapatnév nagy valószínűséggel ugyanaz a klub (S37b).
///
/// Önmagában nem dönt: csak a hívó kontextusával (verseny, nap, időpont)
/// együtt használható jelölésre, ezért a kanonikalizáló csak akkor linkel,
/// ha pontosan egy jelölt marad.
pub fn names_likely_same(a: &str, b: &str) -> bool {
    if fold_name(a) == fold_name(b) {
        return true;
    }
    let (ta, tb) = (name_tokens(a), name_tokens(b));
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    if ta.len() == tb.len() && ta.iter().zip(&tb).all(|(x, y)| token_equal(x, y)) {
        return true;
    }
    let subset = |x: &[String], y: &[String]| {
        x.iter()
            .all(|t| y.iter().any(|other| token_equal(t, other)))
    };
    if subset(&ta, &tb) || subset(&tb, &ta) {
        return true;
    }
    let initials = |tokens: &[String]| {
        tokens
            .iter()
            .filter_map(|t| t.chars().next())
            .collect::<String>()
    };
    // „TNS" ↔ „The New Saints" mintájú monogram.
    if ta.len() == 1 && ta[0].len() >= 3 && initials(&tb) == ta[0] {
        return true;
    }
    if tb.len() == 1 && tb[0].len() >= 3 && initials(&ta) == tb[0] {
        return true;
    }
    false
}
