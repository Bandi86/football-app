pub const ODDS_TOLERANCE_DEFAULT: f64 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustClass {
    High,
    Medium,
    DisplayOnly,
}

impl TrustClass {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::DisplayOnly => "display_only",
        }
    }
    pub fn from_db_str(s: &str) -> Result<Self, String> {
        match s {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "display_only" => Ok(Self::DisplayOnly),
            other => Err(format!("ismeretlen trust osztály: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceTrust {
    pub source_code: String,
    pub domain: String,
    pub trust_class: TrustClass,
    pub provider: Option<String>,
}

/// A kanonikus 7 odds-mező értékei. A hiányzó piac/kulcs `None` (a hiányzó
/// adat NULL elve: a forrás nem ad BTTS-t → nincs érték, nem hiba).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OddsValues {
    pub home: Option<f64>,
    pub draw: Option<f64>,
    pub away: Option<f64>,
    pub btts_yes: Option<f64>,
    pub btts_no: Option<f64>,
    pub over25: Option<f64>,
    pub under25: Option<f64>,
}

impl OddsValues {
    /// Kulcsonkénti egyezés: mindkét oldalon hiányzó kulcs kimarad az
    /// összehasonlításból; ha csak az egyik oldalon van érték, az eltérés
    /// (nem lehet „verified” két eltérő piackészletű forrás).
    pub fn matches_within(&self, other: &OddsValues, tolerance: f64) -> bool {
        let pairs = [
            (self.home, other.home),
            (self.draw, other.draw),
            (self.away, other.away),
            (self.btts_yes, other.btts_yes),
            (self.btts_no, other.btts_no),
            (self.over25, other.over25),
            (self.under25, other.under25),
        ];
        // A spec „±tolerance-on belül” matematikai szándékát követjük: az f64
        // ábrázolási zaja a tizedes határokon átcsúszhat (pl. abs(1.85-1.86) =
        // 0.010000000000000009 > 0.01), ezért mikro-epszilonnal engedünk a határon.
        pairs.iter().all(|(a, b)| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => (a - b).abs() <= tolerance + 1e-9,
            _ => false,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SourceOdds {
    pub source_code: String,
    pub provider: Option<String>,
    pub odds: OddsValues,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OddsTrustState {
    Verified,
    Provisional,
    NoOdds,
}

impl OddsTrustState {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Provisional | Self::NoOdds => "provisional",
        }
    }
}

pub fn group_by_provider<'a, T>(
    items: &'a [T],
    provider_of: impl Fn(&T) -> Option<&str>,
) -> Vec<Vec<&'a T>> {
    let mut groups: Vec<Vec<&'a T>> = Vec::new();
    let mut named: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for item in items {
        match provider_of(item) {
            None => groups.push(vec![item]),
            Some(p) => match named.get(p) {
                Some(&idx) => groups[idx].push(item),
                None => {
                    named.insert(p, groups.len());
                    groups.push(vec![item]);
                }
            },
        }
    }
    groups
}
