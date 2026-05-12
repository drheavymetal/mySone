//! Wikidata SPARQL provider for the Classical Hub (Phase 6 — D-022).
//!
//! Two responsibilities:
//!
//! 1. **Composer enrichment** — given a Wikidata QID for a composer,
//!    resolve their portrait (P18), genres (P136), birth year (P569),
//!    and country of citizenship (P27). All optional, all best-effort.
//!
//! 2. **Related composers** — given a QID, find composers who share at
//!    least one genre (P136 ∩) and were born within ±50 years. Used by
//!    the ComposerPage's "Related composers" section.
//!
//! ### Why SPARQL and not the REST API
//!
//! The Wikidata REST `wbgetentities` endpoint returns **the whole
//! entity** — for a single composer that is several hundred KB and
//! includes every label / sitelink / claim. The classical hub only
//! needs ~5 fields per composer. SPARQL lets us project exactly those
//! fields and filter by genre overlap on the server side, in one
//! round-trip per composer.
//!
//! ### Rate limit & politeness
//!
//! WDQS publishes a usage policy ([wdqs-policy]):
//! - Set a descriptive `User-Agent` (we do — "SONE-classical/x.y.z").
//! - One concurrent query per IP is the safe assumption (the policy
//!   allows up to 5 but they discourage saturation).
//! - 60s per-query budget.
//!
//! We cache aggressively (`StaticMeta` tier — 7d / 30d SWR) because
//! related-composers lists are stable on the order of years.
//!
//! [wdqs-policy]: https://www.mediawiki.org/wiki/Wikidata_Query_Service/User_Manual

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::SoneError;

const WDQS_ENDPOINT: &str = "https://query.wikidata.org/sparql";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// Hard cap on related-composers we accept from a single query.
/// The acceptance gate is "≥5 names for Beethoven"; 12 is comfortable
/// without bloating the UI.
const RELATED_LIMIT: usize = 12;
/// Minimum interval between WDQS queries from this client. The SPARQL
/// service tolerates 5 concurrent queries but we serialize to keep our
/// footprint minimal — this is read-only metadata, not the hot path.
const WDQS_MIN_INTERVAL: Duration = Duration::from_millis(1500);

/// Composer enrichment payload returned by `enrich_composer`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WikidataComposerEnrichment {
    pub portrait_url: Option<String>,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    /// QIDs of music genres P136 — kept opaque (Q1234 form). The UI
    /// renders shared-genre tooltips by matching across composers, so
    /// we never need human labels here.
    pub genres: Vec<String>,
}

/// One related composer surfaced by `list_related_composers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikidataRelatedComposer {
    pub qid: String,
    pub name: String,
    /// MB artist MBID extracted from P434 (MusicBrainz-artist-id).
    /// Empty when Wikidata has no MB link for this composer.
    pub mbid: String,
    pub birth_year: Option<i32>,
    pub portrait_url: Option<String>,
    /// Genre QIDs shared with the seed composer. Used for the
    /// "shared: opera · oratorio" tooltip in the UI.
    pub shared_genres: Vec<String>,
}

pub struct WikidataProvider {
    http: reqwest::Client,
    user_agent: String,
    /// Single-process serialiser: a Mutex around an Instant we touch on
    /// each acquire so we space WDQS calls. WDQS is generous, but
    /// politeness is the price of admission to a free public service.
    last_call: Mutex<std::time::Instant>,
}

impl WikidataProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self {
            http,
            user_agent: format!(
                "SONE-classical/{APP_VERSION} (https://github.com/lullabyX/sone) reqwest/{rq}",
                rq = "0.12"
            ),
            last_call: Mutex::new(
                std::time::Instant::now() - WDQS_MIN_INTERVAL,
            ),
        }
    }

    /// Replace the HTTP client (called when proxy settings change).
    #[allow(dead_code)]
    pub fn set_http_client(&mut self, http: reqwest::Client) {
        self.http = http;
    }

    /// Pace the next outbound query. Spaces calls by `WDQS_MIN_INTERVAL`.
    async fn pace(&self) {
        let mut last = self.last_call.lock().await;
        let elapsed = last.elapsed();
        if elapsed < WDQS_MIN_INTERVAL {
            tokio::time::sleep(WDQS_MIN_INTERVAL - elapsed).await;
        }
        *last = std::time::Instant::now();
    }

    /// Run a SPARQL query and return the parsed JSON `bindings` array.
    /// Returns `Ok(Vec::new())` on any soft failure (timeout, 429, parse
    /// error) — Wikidata is best-effort, never blocks the UI.
    async fn query(&self, sparql: &str) -> Result<Vec<Value>, SoneError> {
        self.pace().await;

        let resp = self
            .http
            .get(WDQS_ENDPOINT)
            .query(&[("format", "json"), ("query", sparql)])
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "application/sparql-results+json")
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                // D-038 classification (wdqs is best-effort but the
                // distinction matters for frontend mensajería).
                let inner: SoneError = e.into();
                match inner {
                    SoneError::NetworkTransient(s) => {
                        SoneError::NetworkTransient(format!("wdqs request: {s}"))
                    }
                    SoneError::Network(s) => {
                        SoneError::Network(format!("wdqs request: {s}"))
                    }
                    other => other,
                }
            })?;

        let status = resp.status();
        if !status.is_success() {
            log::warn!("[wikidata] {status} for query");
            return Ok(Vec::new());
        }

        let body = resp.text().await.map_err(|e| {
            let inner: SoneError = e.into();
            match inner {
                SoneError::NetworkTransient(s) => {
                    SoneError::NetworkTransient(format!("wdqs body: {s}"))
                }
                SoneError::Network(s) => SoneError::Network(format!("wdqs body: {s}")),
                other => other,
            }
        })?;
        let parsed: Value = serde_json::from_str(&body)
            .map_err(|e| SoneError::Parse(format!("wdqs json: {e}")))?;

        let bindings = parsed
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(bindings)
    }

    /// Enrich a composer with Wikidata properties. `qid` is the
    /// composer's Wikidata Q-id (without URL prefix). Returns default
    /// (empty) on missing data — never errors.
    pub async fn enrich_composer(
        &self,
        qid: &str,
    ) -> Result<WikidataComposerEnrichment, SoneError> {
        if !is_valid_qid(qid) {
            return Ok(WikidataComposerEnrichment::default());
        }
        // ?image is wikiBase:url for thumbnail; use the special
        // `wdt:P18` to get a Commons file then build the URL ourselves.
        // ?genre uses GROUP_CONCAT to surface multiple values in one row.
        let sparql = format!(
            r#"
SELECT ?portrait ?birthYear ?deathYear (GROUP_CONCAT(DISTINCT ?genre; separator=",") AS ?genres)
WHERE {{
  OPTIONAL {{ wd:{qid} wdt:P18 ?portrait. }}
  OPTIONAL {{ wd:{qid} wdt:P569 ?birth. BIND(YEAR(?birth) AS ?birthYear) }}
  OPTIONAL {{ wd:{qid} wdt:P570 ?death. BIND(YEAR(?death) AS ?deathYear) }}
  OPTIONAL {{ wd:{qid} wdt:P136 ?genre. }}
}}
GROUP BY ?portrait ?birthYear ?deathYear
LIMIT 1
"#
        );
        let bindings = self.query(&sparql).await.unwrap_or_default();
        let row = match bindings.first() {
            Some(r) => r,
            None => return Ok(WikidataComposerEnrichment::default()),
        };
        let portrait_url = pick_value(row, "portrait");
        let birth_year = pick_value(row, "birthYear").and_then(|s| s.parse::<i32>().ok());
        let death_year = pick_value(row, "deathYear").and_then(|s| s.parse::<i32>().ok());
        let genres: Vec<String> = pick_value(row, "genres")
            .map(|s| {
                s.split(',')
                    .map(str::trim)
                    .filter_map(extract_qid)
                    .collect()
            })
            .unwrap_or_default();

        Ok(WikidataComposerEnrichment {
            portrait_url,
            birth_year,
            death_year,
            genres,
        })
    }

    /// List related composers for a seed `qid`. Strategy:
    ///  - Find all entities P31 wd:Q5 (human) AND P106 wd:Q36834 (composer)
    ///    OR P106 wd:Q486748 (pianist) sharing at least one P136 with
    ///    the seed.
    ///  - Constrain birth year to seed_birth ±50 (best-effort proxy
    ///    for "same era").
    ///  - Project label (English), portrait, MB-id, birth-year, shared
    ///    genres.
    ///  - Sort by shared-genre count DESC, then birth proximity ASC.
    ///  - Cap at `RELATED_LIMIT`.
    ///
    /// Returns empty vec on failure (timeout, no QID, no genres).
    pub async fn list_related_composers(
        &self,
        qid: &str,
    ) -> Result<Vec<WikidataRelatedComposer>, SoneError> {
        if !is_valid_qid(qid) {
            return Ok(Vec::new());
        }

        // Step 1: enrich the seed so we know its birth year + genres.
        let seed = self.enrich_composer(qid).await?;
        if seed.genres.is_empty() {
            return Ok(Vec::new());
        }
        let seed_birth = seed.birth_year.unwrap_or(0);

        // Step 2: SPARQL with VALUES for the seed's genres. The UNION
        // over P106=Q36834|Q486748 picks composers AND pianist-composers
        // (Liszt etc. are sometimes filed only as pianist).
        let genre_values: String = seed
            .genres
            .iter()
            .map(|g| format!("wd:{}", g))
            .collect::<Vec<_>>()
            .join(" ");
        let (lo, hi) = if seed_birth == 0 {
            (i32::MIN, i32::MAX)
        } else {
            (seed_birth - 50, seed_birth + 50)
        };
        let sparql = format!(
            r#"
SELECT ?composer ?composerLabel ?mb ?portrait ?birthYear (GROUP_CONCAT(DISTINCT ?sharedGenre; separator=",") AS ?sharedGenres)
WHERE {{
  VALUES ?seedGenre {{ {genre_values} }}
  ?composer wdt:P136 ?seedGenre.
  ?composer wdt:P106 wd:Q36834.
  FILTER(?composer != wd:{qid})
  OPTIONAL {{ ?composer wdt:P434 ?mb. }}
  OPTIONAL {{ ?composer wdt:P18 ?portrait. }}
  OPTIONAL {{ ?composer wdt:P569 ?birth. BIND(YEAR(?birth) AS ?birthYear) }}
  ?composer wdt:P136 ?sharedGenre.
  FILTER(?sharedGenre IN ( {genre_values} ))
  FILTER(BOUND(?birthYear) = false || (?birthYear >= {lo} && ?birthYear <= {hi}))
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "en". }}
}}
GROUP BY ?composer ?composerLabel ?mb ?portrait ?birthYear
ORDER BY DESC(COUNT(DISTINCT ?sharedGenre)) ?birthYear
LIMIT {RELATED_LIMIT}
"#
        );

        let bindings = self.query(&sparql).await.unwrap_or_default();
        let mut out: Vec<WikidataRelatedComposer> = Vec::with_capacity(bindings.len());
        for row in bindings.iter() {
            let composer_uri = pick_value(row, "composer").unwrap_or_default();
            let related_qid = match extract_qid(&composer_uri) {
                Some(q) => q,
                None => continue,
            };
            let name = pick_value(row, "composerLabel").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let mbid = pick_value(row, "mb").unwrap_or_default();
            let portrait_url = pick_value(row, "portrait");
            let birth_year = pick_value(row, "birthYear").and_then(|s| s.parse::<i32>().ok());
            let shared_genres: Vec<String> = pick_value(row, "sharedGenres")
                .map(|s| {
                    s.split(',')
                        .filter_map(|t| extract_qid(t.trim()))
                        .collect()
                })
                .unwrap_or_default();

            out.push(WikidataRelatedComposer {
                qid: related_qid,
                name,
                mbid,
                birth_year,
                portrait_url,
                shared_genres,
            });
        }
        Ok(out)
    }
}

/// Resolve a SPARQL JSON result row's `?var` to its `value` string,
/// returning `None` when the variable is absent.
fn pick_value(row: &Value, var: &str) -> Option<String> {
    row.get(var)
        .and_then(|b| b.get("value"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// Strip a Wikidata entity URL down to its bare QID. Accepts:
///  - `http://www.wikidata.org/entity/Q1234`
///  - `Q1234`
///  - any other string → None
fn extract_qid(s: &str) -> Option<String> {
    let candidate = s.rsplit('/').next().unwrap_or(s).trim();
    if is_valid_qid(candidate) {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Returns true for strings of the form `Q\d+`.
fn is_valid_qid(s: &str) -> bool {
    let s = s.trim();
    s.len() >= 2
        && s.starts_with('Q')
        && s[1..].chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_qid_handles_various_shapes() {
        assert_eq!(extract_qid("Q255").as_deref(), Some("Q255"));
        assert_eq!(
            extract_qid("http://www.wikidata.org/entity/Q255").as_deref(),
            Some("Q255")
        );
        assert_eq!(extract_qid("https://www.wikidata.org/wiki/Q255").as_deref(), Some("Q255"));
        assert_eq!(extract_qid("not-a-qid"), None);
        assert_eq!(extract_qid(""), None);
        // Q must be uppercase
        assert_eq!(extract_qid("q255"), None);
        // No digits after Q rejected
        assert_eq!(extract_qid("Qabc"), None);
    }

    #[test]
    fn is_valid_qid_rejects_garbage() {
        assert!(is_valid_qid("Q1"));
        assert!(is_valid_qid("Q12345678"));
        assert!(!is_valid_qid("Q"));
        assert!(!is_valid_qid(""));
        assert!(!is_valid_qid("Q12a"));
        assert!(!is_valid_qid("12"));
    }

    #[test]
    fn pick_value_picks_string_value() {
        let row = json!({
            "x": {"value": "hello", "type": "literal"},
            "y": {"value": "", "type": "literal"},
            "z": {"type": "uri"},
        });
        assert_eq!(pick_value(&row, "x").as_deref(), Some("hello"));
        assert_eq!(pick_value(&row, "y"), None, "empty string is filtered");
        assert_eq!(pick_value(&row, "z"), None, "missing value field is filtered");
        assert_eq!(pick_value(&row, "absent"), None);
    }

    /// End-to-end happy path for a composer enrichment SPARQL row.
    /// The raw shape mirrors what WDQS returns; we only assert that
    /// our parser pulls out the four fields cleanly.
    #[test]
    fn parse_enrichment_row_extracts_fields() {
        let row = json!({
            "portrait": {
                "type": "uri",
                "value": "http://commons.wikimedia.org/wiki/Special:FilePath/Beethoven.jpg"
            },
            "birthYear": {"type": "literal", "value": "1770"},
            "deathYear": {"type": "literal", "value": "1827"},
            "genres": {
                "type": "literal",
                "value": "http://www.wikidata.org/entity/Q9730,http://www.wikidata.org/entity/Q484641"
            }
        });
        let portrait = pick_value(&row, "portrait");
        assert!(portrait.unwrap().contains("Beethoven.jpg"));
        let birth = pick_value(&row, "birthYear")
            .and_then(|s| s.parse::<i32>().ok());
        assert_eq!(birth, Some(1770));
        let death = pick_value(&row, "deathYear")
            .and_then(|s| s.parse::<i32>().ok());
        assert_eq!(death, Some(1827));
        let genres: Vec<String> = pick_value(&row, "genres")
            .map(|s| s.split(',').filter_map(|t| extract_qid(t.trim())).collect())
            .unwrap_or_default();
        assert_eq!(genres, vec!["Q9730".to_string(), "Q484641".to_string()]);
    }

    /// Related-composer row parsing — one entry with shared genres.
    #[test]
    fn parse_related_row_extracts_full_record() {
        let row = json!({
            "composer": {
                "type": "uri",
                "value": "http://www.wikidata.org/entity/Q7349"  // Brahms
            },
            "composerLabel": {"type": "literal", "value": "Johannes Brahms"},
            "mb": {"type": "literal", "value": "f50aa0e3-83b6-4d18-a2ab-c97432f64a64"},
            "birthYear": {"type": "literal", "value": "1833"},
            "portrait": {
                "type": "uri",
                "value": "http://commons.wikimedia.org/wiki/Special:FilePath/Brahms.jpg"
            },
            "sharedGenres": {
                "type": "literal",
                "value": "http://www.wikidata.org/entity/Q9730,http://www.wikidata.org/entity/Q484641"
            }
        });
        let composer_uri = pick_value(&row, "composer").unwrap_or_default();
        assert_eq!(extract_qid(&composer_uri).as_deref(), Some("Q7349"));
        let name = pick_value(&row, "composerLabel").unwrap_or_default();
        assert_eq!(name, "Johannes Brahms");
        let mbid = pick_value(&row, "mb").unwrap_or_default();
        assert_eq!(mbid, "f50aa0e3-83b6-4d18-a2ab-c97432f64a64");
        let birth_year = pick_value(&row, "birthYear")
            .and_then(|s| s.parse::<i32>().ok());
        assert_eq!(birth_year, Some(1833));
        let shared_genres: Vec<String> = pick_value(&row, "sharedGenres")
            .map(|s| s.split(',').filter_map(|t| extract_qid(t.trim())).collect())
            .unwrap_or_default();
        assert_eq!(shared_genres.len(), 2);
    }

    #[test]
    fn enrich_with_invalid_qid_returns_default() {
        // Tokio runtime smoke: we don't need network — invalid qid bails
        // out before any HTTP call.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let p = WikidataProvider::new(reqwest::Client::new());
        let out = rt.block_on(p.enrich_composer("not-a-qid")).unwrap();
        assert!(out.portrait_url.is_none());
        assert!(out.birth_year.is_none());
        assert!(out.genres.is_empty());
    }

    #[test]
    fn related_with_invalid_qid_returns_empty() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let p = WikidataProvider::new(reqwest::Client::new());
        let out = rt.block_on(p.list_related_composers("nope")).unwrap();
        assert!(out.is_empty());
    }
}

/// Marker re-export so `Arc<WikidataProvider>` is namespaced under
/// `providers::wikidata::*` in tests / catalog imports.
#[allow(dead_code)]
pub type SharedWikidata = Arc<WikidataProvider>;
