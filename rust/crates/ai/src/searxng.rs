//! Helyi SearXNG webkeresés (JSON API). Csak 127.0.0.1-re hív, nincs
//! API-kulcs és nincs külső szolgáltatás; a találatokat a modell
//! `web_search` toolja használja.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::AiError;

/// Felnőtt/spam tartalom jelei a találat domainjében vagy címében.
/// Szó-tagként vizsgáljuk, hogy az `essex` (helynév) ne legyen hamis találat.
const BLOCKED_TOKENS: &[&str] = &[
    "porn",
    "porno",
    "xxx",
    "sex",
    "szex",
    "escort",
    "adult",
    "casino",
    "viagra",
    "dating",
    // Keresési zaj: nem adnak választ sportkérdésre, mégis előre kerülnek.
    "earth",
    "scholar",
    "translate",
];

/// Enyhe relevanciaszűrő: a kérdés érdemi szavai közül legalább egy szerepeljen
/// a találatban. A kereső gyakran ad vissza céginfót/banki oldalt névszerű
/// kérdésekre; ezek nem bizonyítékai egy sportállításnak.
fn is_relevant(query: &str, title: &str, url: &str, content: &str) -> bool {
    let stopwords = [
        "mikor",
        "mennyi",
        "melyik",
        "hogy",
        "milyen",
        "következő",
        "kovetkezo",
        "játsza",
        "jatsza",
        "mérkőzés",
        "merkozes",
        "meccs",
        "next",
        "match",
        "when",
        "what",
        "with",
        "against",
        "the",
        "van",
        "lesz",
    ];
    let words: Vec<String> = tokens(query)
        .into_iter()
        .filter(|word| word.chars().count() >= 4 && !stopwords.contains(&word.as_str()))
        .collect();
    if words.is_empty() {
        return true;
    }
    let haystack = format!("{title} {url} {content}").to_lowercase();
    words.iter().any(|word| {
        let prefix: String = word.chars().take(6).collect();
        haystack.contains(prefix.as_str())
    })
}

/// Csak valódi, http(s) URL legyen forrás (a `http:///go/...` átirányítások
/// gazda nélkül érkeznek a SearXNG-ből).
fn is_usable_url(url: &str) -> bool {
    Url::parse(url).is_ok_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https")
            && parsed.host_str().is_some_and(|host| host.contains('.'))
    })
}

/// A domain és a cím szó-tagjai; a kötőjelek és ékezetes ékezetek eltávolítása
/// után (pl. `magyar-szex.hu` → `magyar`, `szex`, `hu`).
fn tokens(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// A SearXNG néha felnőtt/spam találatokat is visszaad; ezek nem kerülhetnek
/// a modellhez és a válasz forráslistájába sem (forrásbizonyíték minősége).
fn is_blocked(url: &str, title: &str) -> bool {
    let parsed = Url::parse(url).ok();
    let host = parsed
        .as_ref()
        .and_then(|parsed| parsed.host_str().map(str::to_owned))
        .unwrap_or_default();
    let path = parsed
        .map(|parsed| parsed.path().to_owned())
        .unwrap_or_default();
    tokens(&host)
        .into_iter()
        .chain(tokens(&path))
        .chain(tokens(title))
        .any(|token| BLOCKED_TOKENS.contains(&token.as_str()))
}

/// Egy keresési találat, a modellnek átadható minimális formában.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct SearxResponse {
    #[serde(default)]
    results: Vec<SearxItem>,
}

#[derive(Debug, Deserialize)]
struct SearxItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

/// SearXNG kliens egy konfigurált helyi végponttal.
#[derive(Debug, Clone)]
pub struct SearxngClient {
    http: reqwest::Client,
    endpoint: Url,
}

impl SearxngClient {
    pub fn new(endpoint: &str, timeout: Duration) -> Result<Self, AiError> {
        let endpoint = Url::parse(endpoint)
            .map_err(|e| AiError::Config(format!("érvénytelen SearXNG URL ({endpoint}): {e}")))?;
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| AiError::Config(format!("nem sikerült HTTP-klienst építeni: {e}")))?;
        Ok(Self { http, endpoint })
    }

    /// Keresés a helyi SearXNG-n; üres kifejezésre üres találatlista.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, AiError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let mut url = self.endpoint.clone();
        url.set_path("/search");
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("format", "json");
        let url_text = url.to_string();
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|source| AiError::Http {
                url: url_text.clone(),
                source,
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AiError::Status {
                url: url_text,
                status,
            });
        }
        let body = response.text().await.map_err(|source| AiError::Http {
            url: url_text.clone(),
            source,
        })?;
        let parsed: SearxResponse =
            serde_json::from_str(&body).map_err(|source| AiError::Decode {
                context: "SearXNG válasz".to_string(),
                source,
            })?;
        Ok(parsed
            .results
            .into_iter()
            .filter(|item| !item.url.trim().is_empty())
            .filter(|item| is_usable_url(&item.url))
            .filter(|item| !is_blocked(&item.url, &item.title))
            .filter(|item| is_relevant(query, &item.title, &item.url, &item.content))
            .take(limit)
            .map(|item| SearchResult {
                title: item.title.trim().to_string(),
                url: item.url.trim().to_string(),
                content: item.content.trim().to_string(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_adult_spam_from_todays_search_results() {
        assert!(is_blocked(
            "https://magyar-szex.hu/fasz%20szop%C3%A1s/",
            "Fasz szopás szexvideók"
        ));
        assert!(is_blocked(
            "https://porn.szex.hu/cimkek/orias_fasz/",
            "óriás fasz videók"
        ));
        assert!(is_blocked(
            "https://example.com/xxx",
            "A Premier League eredményei"
        ));
    }

    #[test]
    fn keeps_legitimate_football_sources() {
        assert!(!is_blocked(
            "https://www.espn.com/soccer/match/_/gameId/401880728",
            "Doncaster vs Huddersfield - Live Score"
        ));
        assert!(!is_blocked(
            "https://www.skysports.com/football/doncaster-rovers-vs-huddersfield-town/561796",
            "Doncaster Rovers vs Huddersfield Town"
        ));
        // Az "essex" nem tartalmaz önálló "sex" szót.
        assert!(!is_blocked(
            "https://essexfa.example/meccs",
            "Essex FA Cup eredmény"
        ));
        assert!(is_blocked(
            "https://m.google.hu/intl/hu/earth/download/gep/agree.html",
            "Google Earth"
        ));
    }

    #[test]
    fn relevance_filter_drops_unrelated_results() {
        let query = "Mikor játssza a Ferencváros a Paks elleni következő mérkőzését?";
        assert!(is_relevant(
            query,
            "Ferencváros vs Paks Broadcasting Channels",
            "https://serieschedule.com/event/ferencvaros-vs-paks",
            ""
        ));
        assert!(!is_relevant(
            query,
            "Kontron Hungary Kft. - Céginformáció",
            "https://www.ceginformacio.hu/cr9312554995",
            "Céginformáció, SEPA fizetési övezet"
        ));
    }

    #[test]
    fn rejects_urls_without_a_real_host() {
        assert!(!is_usable_url("http:///go/v1/redirect"));
        assert!(!is_usable_url("ftp://example.com/x"));
        assert!(is_usable_url(
            "https://www.flashscore.com/team/ferencvaros/fixtures/"
        ));
    }
}
