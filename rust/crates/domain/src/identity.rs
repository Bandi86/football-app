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

/// Tárolási ország-kód kanonizálás (S41): ISO3 → ISO2, illetve a brit hazai
/// nemzetek egységes alrégió-kódja. A [`normalize_country_code`] az
/// identitás-összehasonlítás lazább kulcsa (GB-* → GB, ENG marad ENG); ez a
/// függvény a `core.country.code` **tárolt** alakját egységesíti, hogy az
/// ingest ne hozzon létre párhuzamos ISO3-sorokat (`ESP` az `ES` mellett).
///
/// Döntések:
/// - `ENG` → `GB-ENG`, `SCO` → `GB-SCT`, `WAL` → `GB-WLS`, `NIR` → `GB-NIR`
///   (a korábbi `GB`-sor mellett a hazai nemzetek külön alrégió-sorként élnek);
///   a már `GB-XXX` alakú kód nagybetűsítve változatlan;
/// - `GB`/`GBR` → `GB`;
/// - `INT` → `XX` (nemzetközi / kontinentális verseny);
/// - ismert háromjegyű kód → a hozzá tartozó ISO2;
/// - ismeretlen kód változatlan, nagybetűsítve.
pub fn canonical_country_code(code: &str) -> String {
    let upper = code.trim().to_ascii_uppercase();
    if upper.is_empty() {
        return String::new();
    }
    // A brit hazai nemzetek alrégió-kódja már kanonikus: csak normalizáljuk.
    if upper.starts_with("GB-") {
        return upper;
    }
    match upper.as_str() {
        "ENG" => return "GB-ENG".to_string(),
        "SCO" => return "GB-SCT".to_string(),
        "WAL" => return "GB-WLS".to_string(),
        "NIR" => return "GB-NIR".to_string(),
        "GB" | "GBR" => return "GB".to_string(),
        "INT" => return "XX".to_string(),
        _ => {}
    }
    if upper.len() == 3
        && let Some(iso2) = iso3_to_iso2(&upper)
    {
        return iso2.to_string();
    }
    upper
}

/// ISO 3166-1 alpha-3 → alpha-2 (a labdarúgásban gyakori kódokkal, illetve a
/// futball-specifikus rövidítésekkel, pl. `GER`, `NED`, `NIG`, `UAE`).
fn iso3_to_iso2(code: &str) -> Option<&'static str> {
    let iso2 = match code {
        // Európa
        "ALB" => "AL",
        "AND" => "AD",
        "ARM" => "AM",
        "AUT" => "AT",
        "AZE" => "AZ",
        "BLR" => "BY",
        "BEL" => "BE",
        "BIH" => "BA",
        "BUL" | "BGR" => "BG",
        "CRO" | "HRV" => "HR",
        "CYP" => "CY",
        "CZE" => "CZ",
        "DEN" | "DNK" => "DK",
        "EST" => "EE",
        "FRO" => "FO",
        "FIN" => "FI",
        "FRA" => "FR",
        "GEO" => "GE",
        "GER" | "DEU" => "DE",
        "GIB" => "GI",
        "GRE" | "GRC" => "GR",
        "HUN" => "HU",
        "ISL" => "IS",
        "ISR" => "IL",
        "ITA" => "IT",
        "KAZ" => "KZ",
        "KOS" => "XK",
        "LVA" => "LV",
        "LIE" => "LI",
        "LTU" => "LT",
        "LUX" => "LU",
        "MLT" => "MT",
        "MDA" => "MD",
        "MNE" => "ME",
        "NED" | "NLD" => "NL",
        "MKD" => "MK",
        "NOR" => "NO",
        "POL" => "PL",
        "POR" | "PRT" => "PT",
        "IRL" => "IE",
        "ROU" | "ROM" => "RO",
        "RUS" => "RU",
        "SMR" => "SM",
        "SRB" => "RS",
        "SVK" => "SK",
        "SVN" => "SI",
        "ESP" => "ES",
        "SWE" => "SE",
        "SUI" | "CHE" => "CH",
        "TUR" => "TR",
        "UKR" => "UA",
        // Dél- és Közép-Amerika, Karib-térség
        "ARG" => "AR",
        "BOL" => "BO",
        "BRA" => "BR",
        "CHI" | "CHL" => "CL",
        "COL" => "CO",
        "CRC" => "CR",
        "CUB" => "CU",
        "CUW" => "CW",
        "ECU" => "EC",
        "SLV" => "SV",
        "GUA" | "GTM" => "GT",
        "GUY" => "GY",
        "HAI" => "HT",
        "HON" => "HN",
        "JAM" => "JM",
        "MEX" => "MX",
        "NCA" => "NI",
        "PAN" => "PA",
        "PAR" => "PY",
        "PER" => "PE",
        "SUR" => "SR",
        "TRI" | "TTO" => "TT",
        "URU" | "URY" => "UY",
        "USA" => "US",
        "VEN" => "VE",
        // Ázsia / Óceánia
        "AFG" => "AF",
        "AUS" => "AU",
        "BHR" => "BH",
        "BAN" | "BGD" => "BD",
        "CAM" | "KHM" => "KH",
        "CHN" => "CN",
        "TPE" => "TW",
        "HKG" => "HK",
        "IND" => "IN",
        "IDN" => "ID",
        "IRN" => "IR",
        "IRQ" => "IQ",
        "JPN" => "JP",
        "JOR" => "JO",
        "KUW" => "KW",
        "KGZ" => "KG",
        "LAO" => "LA",
        "LIB" => "LB",
        "MAS" | "MYS" => "MY",
        "MDV" => "MV",
        "MYA" | "MMR" => "MM",
        "NEP" => "NP",
        "NZL" => "NZ",
        "OMA" => "OM",
        "PAK" => "PK",
        "PHI" | "PHL" => "PH",
        "PRK" => "KP",
        "KOR" => "KR",
        "KSA" | "SAU" => "SA",
        "SGP" => "SG",
        "SRI" => "LK",
        "SYR" => "SY",
        "TJK" => "TJ",
        "THA" => "TH",
        "TLS" => "TL",
        "TKM" => "TM",
        "UAE" => "AE",
        "UZB" => "UZ",
        "VIE" | "VNM" => "VN",
        "YEM" => "YE",
        // Afrika
        "ALG" | "DZA" => "DZ",
        "ANG" => "AO",
        "BEN" => "BJ",
        "BFA" => "BF",
        "BDI" => "BI",
        "BOT" => "BW",
        "CPV" => "CV",
        "CMR" => "CM",
        "CAF" => "CF",
        "CHA" | "TCD" => "TD",
        "COM" => "KM",
        "COD" => "CD",
        "COG" => "CG",
        "CIV" => "CI",
        "DJI" => "DJ",
        "EGY" => "EG",
        "EQG" => "GQ",
        "ERI" => "ER",
        "ETH" => "ET",
        "GAB" => "GA",
        "GAM" => "GM",
        "GHA" => "GH",
        "GUI" => "GN",
        "GNB" => "GW",
        "KEN" => "KE",
        "LSO" => "LS",
        "LBR" => "LR",
        "LBY" => "LY",
        "MAD" | "MDG" => "MG",
        "MWI" => "MW",
        "MLI" => "ML",
        "MTN" => "MR",
        "MRI" | "MUS" => "MU",
        "MAR" => "MA",
        "MOZ" => "MZ",
        "NAM" => "NA",
        "NIG" | "NER" => "NE",
        "NGA" => "NG",
        "RWA" => "RW",
        "STP" => "ST",
        "SEN" => "SN",
        "SEY" => "SC",
        "SLE" => "SL",
        "SOM" => "SO",
        "RSA" | "ZAF" => "ZA",
        "SSD" => "SS",
        "SDN" | "SUD" => "SD",
        "SWZ" => "SZ",
        "TAN" | "TZA" => "TZ",
        "TOG" => "TG",
        "TUN" => "TN",
        "UGA" => "UG",
        "ZAM" => "ZM",
        "ZIM" => "ZW",
        _ => return None,
    };
    Some(iso2)
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

/// A forrás-csapatnévből levezethető jelölők: utánpótlás-korosztály (U17,
/// U19, …) és női ág („W”, „Women”, „női”). A jelölő a forrás nevében él,
/// ezért megbízhatóbb, mint egy hiányos vagy elavult payload-kontextus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NameMarkers {
    pub youth: bool,
    pub female: bool,
}

impl NameMarkers {
    pub fn is_empty(self) -> bool {
        !self.youth && !self.female
    }
}

/// A csapatnév jelölőinek felismerése (S41 identitás-javítás).
///
/// Korosztály: `U17`, `U-17`, `U 17`, `U19` stb. (a `strip_age_markers`
/// szabályával összhangban). Női ág: `W`, `Women`, `Womens`, `női`/`noi`.
/// A `W` jelölő csak önálló tokenként számít, hogy a klubnevekbe ne fusson bele.
pub fn name_markers(name: &str) -> NameMarkers {
    let folded = fold_name(name);
    let tokens: Vec<&str> = folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();

    let mut youth = false;
    for (i, token) in tokens.iter().enumerate() {
        if *token == "u" {
            // „U 17” alak: a következő token a korosztály száma.
            let next_is_age = tokens
                .get(i + 1)
                .is_some_and(|n| n.len() <= 2 && n.chars().all(|c| c.is_ascii_digit()));
            youth |= next_is_age;
        } else {
            // „u17” / „u-17” alak: a token maga az U + korosztály.
            let age = token.len() >= 2
                && token.len() <= 3
                && token.starts_with('u')
                && token[1..].chars().all(|c| c.is_ascii_digit());
            youth |= age;
        }
    }

    let female = tokens
        .iter()
        .any(|t| matches!(*t, "w" | "women" | "womens" | "noi" | "female"));

    NameMarkers { youth, female }
}

/// A kontextus kiegészítése a névből levezethető jelölőkkel (S41).
///
/// A névből levezetett érték **nyer** a kontextussal szemben: a forrás
/// csapatneve hordozza az age_group/gender/branch jelölőt, a payload
/// kontextusa viszont hiányos vagy elavult lehet (ez okozta a senior csapat
/// újrahasznosítását az U17-es kódoknál). Az ellentmondásokat magyar nyelvű
/// indoklással adja vissza a hívó auditjának/naplójának.
///
/// A női jelölő a `branch`-ben is megjelenik (`women`), az utánpótlás
/// (férfi) csapat pedig `youth` branch-et kap — ugyanaz a sorrend, amit a
/// soccerway adapter használ.
pub fn merge_name_markers(name: &str, base: &IdentityContext) -> (IdentityContext, Vec<String>) {
    let markers = name_markers(name);
    let mut ctx = base.clone();
    let mut notes = Vec::new();

    if markers.female {
        if ctx.gender.as_deref() != Some("female") {
            if let Some(old) = ctx.gender.as_deref() {
                notes.push(format!(
                    "gender: {old} → female (a csapatnév női jelölőt tartalmaz)"
                ));
            }
            ctx.gender = Some("female".to_string());
        }
        if ctx.branch.as_deref() != Some("women") {
            if let Some(old) = ctx.branch.as_deref() {
                notes.push(format!(
                    "branch: {old} → women (a csapatnév női jelölőt tartalmaz)"
                ));
            }
            ctx.branch = Some("women".to_string());
        }
    } else if markers.youth && ctx.branch.as_deref() != Some("youth") {
        if let Some(old) = ctx.branch.as_deref() {
            notes.push(format!(
                "branch: {old} → youth (a csapatnév U-korosztály-jelölőt tartalmaz)"
            ));
        }
        ctx.branch = Some("youth".to_string());
    }

    if markers.youth && ctx.age_group.as_deref() != Some("youth") {
        if let Some(old) = ctx.age_group.as_deref() {
            notes.push(format!(
                "age_group: {old} → youth (a csapatnév U-korosztály-jelölőt tartalmaz)"
            ));
        }
        ctx.age_group = Some("youth".to_string());
    }

    (ctx, notes)
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
/// Korosztály-/női-jelölők levágása ("Germany U17" → "Germany", "Bodø / Glimt
/// (női)" → "Bodø / Glimt") — a források eltérően jelölik ugyanazt a csapatot.
/// Csak összehasonlításra használható, kanonikus név képzésére nem.
pub fn strip_age_markers(name: &str) -> String {
    let folded = fold_name(name);
    folded
        .split_whitespace()
        .filter(|token| {
            let t = token.trim_matches(|c: char| !c.is_alphanumeric());
            if t.is_empty() {
                return false;
            }
            let age = t.len() >= 2
                && t.starts_with('u')
                && t[1..].chars().all(|c| c.is_ascii_digit())
                && t.len() <= 3;
            !age && !matches!(t, "noi" | "women" | "womens" | "n")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Két név ugyanaz-e a korosztály-jelölés elhagyásával (S37c szigorú pár).
pub fn names_likely_same_ignoring_age(a: &str, b: &str) -> bool {
    let (sa, sb) = (strip_age_markers(a), strip_age_markers(b));
    !sa.is_empty() && !sb.is_empty() && names_likely_same(&sa, &sb)
}

/// Auto-merge-hez elég szigorú név-egyezés (S37c backfill).
///
/// A lazább [`names_likely_same`] prefix/elgépelés-szabályai nemzeti válogatottaknál
/// tévesen egyezhetnek ("Dominica" / "Dominican Republic"), ezért itt:
/// - a teljes (korosztály-jelölés nélküli) nevek fold-egyezése, vagy
/// - azonos tokenek halmaza/token-részhalmaz (exact token-egyezés), vagy
/// - többtokenes névnél pontosan egy eltérő token, ami legfeljebb 1 szerkesztés,
/// - egytokenes névnél a többtokenes másik fél monogramja ("TNS" = The New Saints).
pub fn names_strictly_same(a: &str, b: &str) -> bool {
    let (sa, sb) = (strip_age_markers(a), strip_age_markers(b));
    if sa.is_empty() || sb.is_empty() {
        return false;
    }
    if sa == sb {
        return true;
    }
    let (ta, tb) = (name_tokens(&sa), name_tokens(&sb));
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    let subset_exact = |x: &[String], y: &[String]| x.iter().all(|t| y.iter().any(|o| o == t));
    if subset_exact(&ta, &tb) || subset_exact(&tb, &ta) {
        return true;
    }
    // Egytokenes oldal: monogram (pl. "tns" = the new saints).
    if ta.len() == 1 || tb.len() == 1 {
        let (short, long) = if ta.len() == 1 {
            (&ta, &tb)
        } else {
            (&tb, &ta)
        };
        if long.len() >= 2 {
            let initials: String = long
                .iter()
                .map(|t| t.chars().next().unwrap_or(' '))
                .collect();
            if short[0].len() >= 2 && short[0] == initials {
                return true;
            }
        }
        return false;
    }
    // Többtokenes: legfeljebb egy eltérő token, 1 szerkesztésnyi távolsággal.
    if ta.len() == tb.len() {
        let diffs: Vec<usize> = (0..ta.len()).filter(|&i| ta[i] != tb[i]).collect();
        if diffs.len() == 1 {
            let i = diffs[0];
            let (x, y) = (&ta[i], &tb[i]);
            if x.len().min(y.len()) >= 6 && token_equal(x, y) {
                return true;
            }
        }
    }
    false
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_country_code_iso3_to_iso2() {
        for (iso3, iso2) in [
            ("ESP", "ES"),
            ("HUN", "HU"),
            ("CHL", "CL"),
            ("CHI", "CL"),
            ("ECU", "EC"),
            ("IND", "IN"),
            ("KEN", "KE"),
            ("MDA", "MD"),
            ("NIG", "NE"),
            ("PAN", "PA"),
            ("PAR", "PY"),
            ("PER", "PE"),
            ("SMR", "SM"),
            ("VEN", "VE"),
            ("BOT", "BW"),
            ("IDN", "ID"),
            ("MAS", "MY"),
            ("VIE", "VN"),
            ("ALB", "AL"),
            ("UAE", "AE"),
            ("FRA", "FR"),
            ("GER", "DE"),
            ("BRA", "BR"),
            ("JPN", "JP"),
            ("USA", "US"),
            ("MAR", "MA"),
            ("RSA", "ZA"),
        ] {
            assert_eq!(canonical_country_code(iso3), iso2, "{iso3}");
        }
    }

    #[test]
    fn canonical_country_code_keeps_iso2_and_home_nations() {
        assert_eq!(canonical_country_code("ES"), "ES");
        assert_eq!(canonical_country_code("gb"), "GB");
        assert_eq!(canonical_country_code("GBR"), "GB");
        assert_eq!(canonical_country_code("ENG"), "GB-ENG");
        assert_eq!(canonical_country_code("sco"), "GB-SCT");
        assert_eq!(canonical_country_code("WAL"), "GB-WLS");
        assert_eq!(canonical_country_code("NIR"), "GB-NIR");
        assert_eq!(canonical_country_code(" GB-ENG "), "GB-ENG");
        assert_eq!(canonical_country_code("gb-wls"), "GB-WLS");
        assert_eq!(canonical_country_code("INT"), "XX");
    }

    #[test]
    fn canonical_country_code_is_unknown_and_idempotent() {
        assert_eq!(canonical_country_code("zz"), "ZZ");
        assert_eq!(canonical_country_code("XY"), "XY");
        assert_eq!(canonical_country_code(""), "");
        assert_eq!(canonical_country_code("   "), "");
        for code in ["ESP", "ENG", "GB-ENG", "INT", "zz", "GBR"] {
            let once = canonical_country_code(code);
            assert_eq!(canonical_country_code(&once), once, "{code}");
        }
    }

    #[test]
    fn normalize_country_code_unchanged_by_storage_canonicalization() {
        // A tárolási kanonizálás nem írhatja át az identitás-összehasonlítás kulcsát.
        assert_eq!(normalize_country_code("ENG"), "ENG");
        assert_eq!(normalize_country_code("GB-ENG"), "GB");
        assert_eq!(normalize_country_code("ESP"), "ES");
        assert_eq!(normalize_country_code("INT"), "INT");
    }

    #[test]
    fn name_markers_detects_youth_and_women() {
        assert_eq!(
            name_markers("Bahrain U17"),
            NameMarkers {
                youth: true,
                female: false
            }
        );
        assert_eq!(
            name_markers("Hungary U-19"),
            NameMarkers {
                youth: true,
                female: false
            }
        );
        assert_eq!(
            name_markers("Iceland U 21"),
            NameMarkers {
                youth: true,
                female: false
            }
        );
        assert_eq!(
            name_markers("Germany Women"),
            NameMarkers {
                youth: false,
                female: true
            }
        );
        assert_eq!(
            name_markers("Bodø / Glimt (női)"),
            NameMarkers {
                youth: false,
                female: true
            }
        );
        assert_eq!(
            name_markers("Spain W"),
            NameMarkers {
                youth: false,
                female: true
            }
        );
        assert_eq!(name_markers("Bahrain"), NameMarkers::default());
        assert_eq!(name_markers("Wrexham"), NameMarkers::default());
        assert!(name_markers("Bahrain").is_empty());
        assert!(!name_markers("Bahrain U17").is_empty());
    }

    #[test]
    fn merge_name_markers_overrides_conflicting_context() {
        let base = IdentityContext {
            country_code: Some("BH".into()),
            gender: Some("male".into()),
            age_group: Some("senior".into()),
            branch: Some("senior".into()),
        };
        let (ctx, notes) = merge_name_markers("Bahrain U17", &base);
        assert_eq!(ctx.age_group.as_deref(), Some("youth"));
        assert_eq!(ctx.branch.as_deref(), Some("youth"));
        assert_eq!(ctx.gender.as_deref(), Some("male"));
        assert_eq!(ctx.country_code.as_deref(), Some("BH"));
        assert_eq!(
            notes.len(),
            2,
            "age_group és branch ellentmondás: {notes:?}"
        );
        assert!(
            notes
                .iter()
                .any(|n| n.contains("age_group: senior → youth"))
        );
        assert!(notes.iter().any(|n| n.contains("branch: senior → youth")));

        let empty = IdentityContext::default();
        let (ctx, notes) = merge_name_markers("Germany Women", &empty);
        assert_eq!(ctx.gender.as_deref(), Some("female"));
        assert_eq!(ctx.branch.as_deref(), Some("women"));
        assert!(ctx.age_group.is_none());
        assert!(
            notes.is_empty(),
            "hiányzó mezőnél nincs ellentmondás: {notes:?}"
        );

        // A válogatott női korosztályos neve: age_group youth, branch women.
        let (ctx, _) = merge_name_markers("Spain U19 W", &empty);
        assert_eq!(ctx.age_group.as_deref(), Some("youth"));
        assert_eq!(ctx.gender.as_deref(), Some("female"));
        assert_eq!(ctx.branch.as_deref(), Some("women"));
    }
}
