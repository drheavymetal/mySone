//! Tidal provider for the Classical Hub catalog. Two operations:
//!
//!   * `lookup_by_isrc` — calls Tidal's `/v1/tracks?isrc=XXX` endpoint
//!     directly (the standard `TidalClient` doesn't expose this; the spike
//!     proved the call shape). Returns the first track that exact-matches
//!     the ISRC, with its quality tags.
//!
//!   * `search_canonical` — runs `tidal.search(query, N)` and returns the
//!     raw track list for the matching layer to score. The provider
//!     itself does not score; that's `matching::Matcher`'s job.
//!
//! This provider does NOT mutate the shared `TidalClient` state — it
//! borrows a read-only handle to its tokens and HTTP client. We hold a
//! `tokio::sync::Mutex<TidalClient>` like the rest of the app.
//!
//! Audio routing remains COMPLETELY untouched: this layer is read-only
//! catalog lookup. The bit-perfect contract (route_volume_change, writer
//! guard) is unaffected.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use super::super::types::{CatalogueNumber, Recording, Work};
use super::ClassicalProvider;
use crate::tidal_api::{TidalClient, TidalSearchResults, TidalTrack};
use crate::SoneError;

const TIDAL_API_V1: &str = "https://api.tidal.com/v1";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

/// Outcome of an ISRC → Tidal track lookup.
#[derive(Debug, Clone)]
pub struct TidalIsrcHit {
    pub track_id: u64,
    pub album_id: Option<u64>,
    pub quality_tags: Vec<String>,
    pub audio_modes: Vec<String>,
    pub duration_secs: u32,
    pub cover: Option<String>,
}

/// Phase 4 (D-017): metadata-only result of probing
/// `/tracks/{id}/playbackinfopostpaywall`. Does NOT include the manifest
/// (we deliberately skip decoding it — manifests carry signed URLs that
/// expire and must never be cached). Used by the Hub to refine the
/// `HIRES_LOSSLESS` tier into "24/96", "24/192", etc.
///
/// SAFETY NOTE: this struct never reaches the audio path. It is consumed
/// solely by `CatalogService` to populate `Recording::sample_rate_hz` /
/// `bit_depth` / `quality_score`. The bit-perfect contract is unaffected.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackQualityMeta {
    /// Tier label as Tidal returned it: "HIRES_LOSSLESS" | "LOSSLESS" |
    /// "MQA" | "HIGH" | "LOW". Empty when missing.
    pub tier: String,
    /// Sample rate in Hz, e.g. 44100, 48000, 96000, 192000. None when
    /// Tidal omitted the field for that track.
    #[serde(default)]
    pub sample_rate_hz: Option<u32>,
    /// Bit depth, e.g. 16 or 24.
    #[serde(default)]
    pub bit_depth: Option<u8>,
}

pub struct TidalProvider {
    client: Arc<Mutex<TidalClient>>,
}

impl TidalProvider {
    pub fn new(client: Arc<Mutex<TidalClient>>) -> Self {
        Self { client }
    }

    /// Resolve an ISRC to a Tidal track via the v1 `/tracks?isrc=...`
    /// endpoint. Returns `Ok(None)` when the ISRC is not present in
    /// Tidal's region catalogue. Errors propagate for actual network /
    /// auth issues so callers can decide whether to fall back to text
    /// search.
    pub async fn lookup_by_isrc(
        &self,
        isrc: &str,
    ) -> Result<Option<TidalIsrcHit>, SoneError> {
        let (access_token, country, http) = {
            let guard = self.client.lock().await;
            let tokens = guard
                .tokens
                .as_ref()
                .ok_or(SoneError::NotAuthenticated)?
                .access_token
                .clone();
            (tokens, guard.country_code.clone(), guard.raw_client().clone())
        };

        let url = format!("{TIDAL_API_V1}/tracks");
        let resp = http
            .get(&url)
            .header("Authorization", format!("Bearer {access_token}"))
            .query(&[("isrc", isrc), ("countryCode", country.as_str())])
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                // D-038 — classify reqwest errors so transient failures
                // (connect, TLS EOF, timeout) propagate as transient.
                let inner: SoneError = e.into();
                match inner {
                    SoneError::NetworkTransient(s) => {
                        SoneError::NetworkTransient(format!("tidal isrc {isrc}: {s}"))
                    }
                    SoneError::Network(s) => {
                        SoneError::Network(format!("tidal isrc {isrc}: {s}"))
                    }
                    other => other,
                }
            })?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(SoneError::from_http_status(
                status.as_u16(),
                format!("tidal isrc {isrc}: HTTP {status}: {body}"),
            ));
        }

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| SoneError::Parse(format!("tidal isrc body: {e}")))?;

        // Two response shapes possible: top-level array, or {items: [...]}.
        let arr_owned: Vec<Value>;
        let arr: &Vec<Value> = if let Some(a) = json.as_array() {
            a
        } else if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
            items
        } else {
            arr_owned = Vec::new();
            &arr_owned
        };

        if arr.is_empty() {
            return Ok(None);
        }

        // Prefer exact ISRC match. Otherwise accept the first decoded track
        // (Tidal's filter normalises case/punctuation differently from MB).
        let mut fallback: Option<TidalTrack> = None;
        for item in arr.iter() {
            match serde_json::from_value::<TidalTrack>(item.clone()) {
                Ok(mut t) => {
                    t.backfill_artist();
                    if t.isrc.as_deref() == Some(isrc) {
                        return Ok(Some(track_to_hit(&t)));
                    }
                    if fallback.is_none() {
                        fallback = Some(t);
                    }
                }
                Err(e) => {
                    log::debug!("tidal isrc decode skip: {e}");
                }
            }
        }

        Ok(fallback.map(|t| track_to_hit(&t)))
    }

    /// Run a Tidal text search. Wrapper around `TidalClient::search`
    /// that holds the lock for the minimum time necessary.
    pub async fn search_canonical(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<TidalSearchResults, SoneError> {
        let mut guard = self.client.lock().await;
        guard.search(query, limit).await
    }

    /// Phase 4 (D-017): fetch the per-track quality metadata from Tidal
    /// via `playbackinfopostpaywall`. Returns ONLY the top-level
    /// `audio_quality` / `bit_depth` / `sample_rate` fields — the
    /// manifest is intentionally discarded so this method is safe to
    /// cache without leaking expiring URLs.
    ///
    /// IMPORTANT: this method never mutates `TidalClient` and never
    /// emits an audio engine call. It is a parallel read-only path that
    /// shares only tokens + http client with the rest of the app.
    ///
    /// Quality probe semantics: we ask Tidal for `HI_RES_LOSSLESS`
    /// (its highest tier name) so the response reports the maximum
    /// quality the user's subscription can stream — Tidal will degrade
    /// the response automatically when the track itself is lower-tier.
    /// That matches what `signal_path` would observe at stream-time.
    pub async fn fetch_track_quality_meta(
        &self,
        track_id: u64,
    ) -> Result<Option<TrackQualityMeta>, SoneError> {
        let (access_token, country, http) = {
            let guard = self.client.lock().await;
            let tokens = guard
                .tokens
                .as_ref()
                .ok_or(SoneError::NotAuthenticated)?
                .access_token
                .clone();
            (tokens, guard.country_code.clone(), guard.raw_client().clone())
        };

        let url = format!("{TIDAL_API_V1}/tracks/{track_id}/playbackinfopostpaywall");
        let resp = http
            .get(&url)
            .header("Authorization", format!("Bearer {access_token}"))
            .query(&[
                ("countryCode", country.as_str()),
                ("audioquality", "HI_RES_LOSSLESS"),
                ("playbackmode", "STREAM"),
                ("assetpresentation", "FULL"),
            ])
            .timeout(HTTP_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                // D-038 — same transient-vs-permanent classification.
                let inner: SoneError = e.into();
                match inner {
                    SoneError::NetworkTransient(s) => {
                        SoneError::NetworkTransient(format!("tidal pbinfo {track_id}: {s}"))
                    }
                    SoneError::Network(s) => {
                        SoneError::Network(format!("tidal pbinfo {track_id}: {s}"))
                    }
                    other => other,
                }
            })?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(SoneError::from_http_status(
                status.as_u16(),
                format!("tidal pbinfo {track_id}: HTTP {status}: {body}"),
            ));
        }

        // We pluck the three top-level fields by hand to keep the parse
        // robust against future schema additions and to avoid pulling in
        // the manifest blob.
        let json: Value = serde_json::from_str(&body)
            .map_err(|e| SoneError::Parse(format!("tidal pbinfo body: {e}")))?;

        let tier = json
            .get("audioQuality")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let sample_rate_hz = json
            .get("sampleRate")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        let bit_depth = json
            .get("bitDepth")
            .and_then(|v| v.as_u64())
            .and_then(|n| u8::try_from(n).ok());

        if tier.is_empty() && sample_rate_hz.is_none() && bit_depth.is_none() {
            return Ok(None);
        }
        Ok(Some(TrackQualityMeta {
            tier,
            sample_rate_hz,
            bit_depth,
        }))
    }
}

#[async_trait]
impl ClassicalProvider for TidalProvider {
    fn name(&self) -> &'static str {
        "tidal"
    }

    /// No-op: enrichment runs through the matcher. The Catalog service
    /// invokes `lookup_by_isrc` / `search_canonical` directly.
    async fn enrich_recording(&self, _r: &mut Recording) -> Result<(), SoneError> {
        Ok(())
    }

    /// No-op for Work: Tidal doesn't have work entities.
    async fn enrich_work(&self, _w: &mut Work) -> Result<(), SoneError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn track_to_hit(t: &TidalTrack) -> TidalIsrcHit {
    let quality_tags: Vec<String> = t
        .media_metadata
        .as_ref()
        .map(|m| m.tags.clone())
        .unwrap_or_default();
    let audio_modes: Vec<String> = t.audio_modes.clone().unwrap_or_default();
    let cover = t.album.as_ref().and_then(|a| a.cover.clone());
    let album_id = t.album.as_ref().map(|a| a.id);
    TidalIsrcHit {
        track_id: t.id,
        album_id,
        quality_tags,
        audio_modes,
        duration_secs: t.duration,
        cover,
    }
}

/// Build a canonical query for a recording. Used by the matcher and
/// available standalone for the spike → catalog port. Output shape:
/// `"{composer} {stripped_title} [{catalogue}] [{primary_artist}] [{year}]"`.
///
/// D-041 (Phase 8.9): `catalogue` was added so the work-level fallback
/// disambiguates titles whose tail contains a generic numeric token
/// (e.g. `"3 Gesänge von Goethe, Op. 83"` would otherwise let Tidal's
/// FTS engine match Beethoven's Symphony No. 3). When `catalogue` is
/// `Some`, its `display` form (e.g. "Op. 83", "BWV 244", "K. 466") is
/// appended verbatim — this is a high-signal token that Tidal weights
/// heavily.
pub fn build_canonical_query(
    composer_name: Option<&str>,
    work_title: &str,
    catalogue: Option<&CatalogueNumber>,
    primary_artist: Option<&str>,
    year: Option<i32>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = composer_name {
        // Prefer last name to keep the query short and discriminative.
        let last = c
            .split_whitespace()
            .last()
            .unwrap_or(c)
            .to_string();
        parts.push(last);
    }
    parts.push(strip_catalogue_suffix(work_title));
    if let Some(cat) = catalogue {
        // The catalogue display ("Op. 83", "BWV 244") is the key
        // discriminator. Append it verbatim — Tidal FTS treats numeric
        // tokens as high-weight when paired with a system marker.
        let trimmed = cat.display.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
    if let Some(a) = primary_artist {
        parts.push(a.to_string());
    }
    if let Some(y) = year {
        parts.push(y.to_string());
    }
    parts.join(" ")
}

/// Trim the catalogue/key tail of a title to keep the search query
/// generic — e.g. `"Symphony No. 9 in D minor, Op. 125 \"Choral\""` →
/// `"Symphony No. 9"` so Tidal's full-text engine matches more
/// recordings.
fn strip_catalogue_suffix(title: &str) -> String {
    let mut out = title;
    // Cut at first comma — typical separator before "Op. ...", "K. ...".
    if let Some(idx) = out.find(',') {
        out = &out[..idx];
    }
    // Drop any " in <key> minor/major" tail.
    if let Some(idx) = out.to_lowercase().find(" in ") {
        let after = &out[idx + 4..].to_lowercase();
        if after.contains("minor") || after.contains("major") {
            out = &out[..idx];
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_catalogue_tail() {
        assert_eq!(
            strip_catalogue_suffix("Symphony No. 9 in D minor, Op. 125 \"Choral\""),
            "Symphony No. 9"
        );
        assert_eq!(strip_catalogue_suffix("Glassworks"), "Glassworks");
        assert_eq!(strip_catalogue_suffix("Goldberg Variations, BWV 988"), "Goldberg Variations");
    }

    #[test]
    fn builds_query_with_year() {
        let q = build_canonical_query(
            Some("Ludwig van Beethoven"),
            "Symphony No. 9 in D minor, Op. 125",
            None,
            Some("Karajan"),
            Some(1962),
        );
        assert_eq!(q, "Beethoven Symphony No. 9 Karajan 1962");
    }

    #[test]
    fn builds_query_without_artist_or_year() {
        let q = build_canonical_query(
            Some("Philip Glass"),
            "Glassworks",
            None,
            None,
            None,
        );
        assert_eq!(q, "Glass Glassworks");
    }

    // ---- D-041 (A2) — catalogue number propagation ----

    #[test]
    fn builds_query_appends_opus_catalogue() {
        let cat = CatalogueNumber {
            system: "Op".to_string(),
            number: "83".to_string(),
            display: "Op. 83".to_string(),
        };
        let q = build_canonical_query(
            Some("Beethoven"),
            "3 Gesänge von Goethe",
            Some(&cat),
            None,
            None,
        );
        assert!(q.contains("Op. 83"), "missing catalogue marker in: {q}");
        assert_eq!(q, "Beethoven 3 Gesänge von Goethe Op. 83");
    }

    #[test]
    fn builds_query_appends_bwv_catalogue() {
        let cat = CatalogueNumber {
            system: "BWV".to_string(),
            number: "244".to_string(),
            display: "BWV 244".to_string(),
        };
        let q = build_canonical_query(
            Some("Bach"),
            "Matthäus-Passion",
            Some(&cat),
            None,
            None,
        );
        assert!(q.contains("BWV 244"), "missing catalogue marker in: {q}");
    }

    #[test]
    fn builds_query_without_catalogue_is_backward_compat() {
        // No catalogue → matches pre-D-041 behaviour for works that
        // never had a catalogue number (Glassworks, free-form titles).
        let q = build_canonical_query(
            Some("Glass"),
            "Glassworks",
            None,
            None,
            None,
        );
        assert_eq!(q, "Glass Glassworks");
    }

    #[test]
    fn builds_query_full_form_with_catalogue_artist_year() {
        let cat = CatalogueNumber {
            system: "Op".to_string(),
            number: "125".to_string(),
            display: "Op. 125".to_string(),
        };
        let q = build_canonical_query(
            Some("Beethoven"),
            "Symphony No. 9 in D minor, Op. 125",
            Some(&cat),
            Some("Karajan"),
            Some(1962),
        );
        // Catalogue between title and artist; year last.
        assert_eq!(q, "Beethoven Symphony No. 9 Op. 125 Karajan 1962");
    }
}
