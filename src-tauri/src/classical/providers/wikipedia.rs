//! Wikipedia REST summary provider for composer + work descriptions.
//!
//! Calls the public `https://{lang}.wikipedia.org/api/rest_v1/page/summary/{title}`
//! endpoint, which is rate-limited but generous (~200 req/s globally; we
//! stay well under). Each summary includes a short `description` (~80
//! chars), an `extract` (~plain prose, 1-3 paragraphs), and an
//! attribution URL we surface in the UI per CC BY-SA.
//!
//! Two title resolution strategies, both best-effort:
//!   - For composers: use the composer name as-is. Wikipedia's redirect
//!     handling resolves "Beethoven" → "Ludwig van Beethoven" reliably.
//!   - For works: use the work title with composer prefix when needed
//!     (e.g. `"Symphony No. 9 (Beethoven)"`).
//!
//! Falls through silently to `Ok(())` when the page doesn't exist (404)
//! — the catalog still works without descriptions.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::super::types::{Composer, Work};
use super::{ClassicalProvider, MbRateLimiter};
use crate::SoneError;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_LANGS: &[&str] = &["en"]; // Phase 1: en-only. Phase 5 adds locale.

/// Provider that hits Wikipedia REST summaries.
///
/// We share an `MbRateLimiter` only as a soft handbrake when used in the
/// same task chain as MB calls — Wikipedia itself does not need 1 req/s.
pub struct WikipediaProvider {
    http: reqwest::Client,
    user_agent: String,
    /// Shared rate-limiter is used if the caller wants to interleave
    /// Wikipedia and MB calls under the same budget. Phase 1 ignores it
    /// for Wikipedia-only chains.
    #[allow(dead_code)]
    rate: Arc<MbRateLimiter>,
}

impl WikipediaProvider {
    pub fn new(http: reqwest::Client, rate: Arc<MbRateLimiter>) -> Self {
        Self {
            http,
            user_agent: format!(
                "SONE-classical/{APP_VERSION} (https://github.com/lullabyX/sone)"
            ),
            rate,
        }
    }

    pub fn set_http_client(&mut self, http: reqwest::Client) {
        self.http = http;
    }

    pub async fn fetch_summary(
        &self,
        title: &str,
        lang: &str,
    ) -> Result<Option<WikiSummary>, SoneError> {
        // The REST summary endpoint expects a URL-encoded title.
        let encoded = url_path_encode(title);
        let url = format!(
            "https://{lang}.wikipedia.org/api/rest_v1/page/summary/{encoded}"
        );

        let resp = self
            .http
            .get(&url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                // D-038 classification.
                let inner: SoneError = e.into();
                match inner {
                    SoneError::NetworkTransient(s) => {
                        SoneError::NetworkTransient(format!("wiki summary {title}: {s}"))
                    }
                    SoneError::Network(s) => {
                        SoneError::Network(format!("wiki summary {title}: {s}"))
                    }
                    other => other,
                }
            })?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            // Treat 5xx and other 4xx as a soft failure: log + None.
            log::debug!("[wiki] {status} for {url}");
            return Ok(None);
        }

        let body = resp.text().await.map_err(|e| {
            let inner: SoneError = e.into();
            match inner {
                SoneError::NetworkTransient(s) => {
                    SoneError::NetworkTransient(format!("wiki body: {s}"))
                }
                SoneError::Network(s) => SoneError::Network(format!("wiki body: {s}")),
                other => other,
            }
        })?;
        let parsed: RawSummary = serde_json::from_str(&body)
            .map_err(|e| SoneError::Parse(format!("wiki json: {e}")))?;

        // 'disambiguation' or 'no extract' → treat as not found.
        if parsed.extract.as_deref().unwrap_or("").trim().is_empty() {
            return Ok(None);
        }
        if parsed.kind.as_deref() == Some("disambiguation") {
            return Ok(None);
        }

        Ok(Some(WikiSummary {
            short: parsed.description,
            long: parsed.extract,
            source_url: parsed
                .content_urls
                .and_then(|c| c.desktop)
                .and_then(|d| d.page),
            portrait_url: parsed.thumbnail.and_then(|t| t.source),
        }))
    }

    pub async fn fetch_for_composer(
        &self,
        composer: &mut Composer,
    ) -> Result<(), SoneError> {
        // Try full name first, then short name.
        let candidates: Vec<String> = std::iter::once(composer.full_name.clone())
            .chain(std::iter::once(Some(composer.name.clone())))
            .flatten()
            .collect();

        for lang in DEFAULT_LANGS {
            for title in candidates.iter() {
                match self.fetch_summary(title, lang).await {
                    Ok(Some(s)) => {
                        composer.bio_short = s.short;
                        composer.bio_long = s.long;
                        composer.bio_source_url = s.source_url;
                        if composer.portrait_url.is_none() {
                            composer.portrait_url = s.portrait_url;
                        }
                        return Ok(());
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::debug!(
                            "[wiki] composer summary {title} ({lang}): {e}"
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn fetch_for_work(&self, work: &mut Work) -> Result<(), SoneError> {
        // Strategy: try `"<title> (<composer last name>)"` first
        // (Wikipedia's standard disambiguation), then plain title.
        let composer_last = work
            .composer_name
            .as_deref()
            .and_then(|n| n.split_whitespace().last())
            .map(String::from);
        let title_a = match composer_last.as_deref() {
            Some(last) => format!("{} ({last})", work.title),
            None => work.title.clone(),
        };
        let title_b = work.title.clone();

        for lang in DEFAULT_LANGS {
            for title in [&title_a, &title_b] {
                match self.fetch_summary(title, lang).await {
                    Ok(Some(s)) => {
                        work.description = s.long.or(s.short);
                        work.description_source_url = s.source_url;
                        return Ok(());
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::debug!(
                            "[wiki] work summary {title} ({lang}): {e}"
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WikiSummary {
    pub short: Option<String>,
    pub long: Option<String>,
    pub source_url: Option<String>,
    pub portrait_url: Option<String>,
}

#[async_trait]
impl ClassicalProvider for WikipediaProvider {
    fn name(&self) -> &'static str {
        "wikipedia"
    }

    async fn enrich_composer(&self, c: &mut Composer) -> Result<(), SoneError> {
        if c.bio_long.is_some() {
            return Ok(());
        }
        self.fetch_for_composer(c).await
    }

    async fn enrich_work(&self, w: &mut Work) -> Result<(), SoneError> {
        if w.description.is_some() || w.title.is_empty() {
            return Ok(());
        }
        self.fetch_for_work(w).await
    }
}

// ---------------------------------------------------------------------------
// Wikipedia raw shape
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawSummary {
    #[serde(rename = "type")]
    kind: Option<String>,
    description: Option<String>,
    extract: Option<String>,
    thumbnail: Option<RawImage>,
    content_urls: Option<RawContentUrls>,
}

#[derive(Debug, Deserialize)]
struct RawImage {
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawContentUrls {
    desktop: Option<RawDesktopUrls>,
}

#[derive(Debug, Deserialize)]
struct RawDesktopUrls {
    page: Option<String>,
}

// ---------------------------------------------------------------------------
// URL path encoding — Wikipedia accepts `_` in place of spaces and percent-encodes
// other unsafe chars. We want a minimal, dependency-free encoder.
// ---------------------------------------------------------------------------

fn url_path_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c == ' ' {
            out.push('_');
        } else if c.is_ascii_alphanumeric()
            || matches!(
                c,
                '_' | '-' | '.' | '~' | '!' | '*' | '\'' | '(' | ')' | ',' | ':'
            )
        {
            out.push(c);
        } else {
            // Percent-encode multi-byte safely.
            let mut buf = [0u8; 4];
            let bytes = c.encode_utf8(&mut buf).as_bytes();
            for b in bytes {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_simple_title() {
        assert_eq!(url_path_encode("Ludwig van Beethoven"), "Ludwig_van_Beethoven");
    }

    #[test]
    fn encodes_special_chars() {
        let encoded = url_path_encode("J. S. Bach / Goldberg");
        assert!(encoded.contains("%2F"));
        assert!(encoded.contains("J._S._Bach"));
    }
}
