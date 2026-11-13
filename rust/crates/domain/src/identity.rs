use uuid::Uuid;

pub const FUZZY_THRESHOLD: f64 = 0.6;

pub fn normalize_name(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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

pub fn context_matches(a: &IdentityContext, b: &IdentityContext) -> bool {
    field_matches(&a.country_code, &b.country_code)
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
