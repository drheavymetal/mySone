use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(1100);

/// All three MBIDs we can resolve from a single name search. Any of them
/// may be `None` if the search couldn't disambiguate or MB has no entry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MbResolved {
    pub recording_mbid: Option<String>,
    pub release_group_mbid: Option<String>,
    pub artist_mbid: Option<String>,
}

pub struct MusicBrainzLookup {
    client: std::sync::Mutex<reqwest::Client>,
    /// Legacy cache from the ISRC lookup path: `isrc -> recording_mbid`.
    cache: Mutex<HashMap<String, Option<String>>>,
    /// Name-based cache: `lower(title)|lower(artist) -> MbResolved`.
    /// Lets every play resolve to all three MBIDs even when ISRC is
    /// missing.
    name_cache: Mutex<HashMap<String, Option<MbResolved>>>,
    cache_path: PathBuf,
    name_cache_path: PathBuf,
    last_request: Mutex<Instant>,
    dirty: AtomicBool,
    name_dirty: AtomicBool,
}

impl MusicBrainzLookup {
    pub fn new(config_dir: &std::path::Path, http_client: reqwest::Client) -> Self {
        let cache_path = config_dir.join("mbid_cache.json");
        let name_cache_path = config_dir.join("mbid_name_cache.json");
        let cache = Self::load_cache(&cache_path);
        let name_cache = Self::load_name_cache(&name_cache_path);

        Self {
            client: std::sync::Mutex::new(http_client),
            cache: Mutex::new(cache),
            name_cache: Mutex::new(name_cache),
            cache_path,
            name_cache_path,
            last_request: Mutex::new(Instant::now() - MIN_REQUEST_INTERVAL),
            dirty: AtomicBool::new(false),
            name_dirty: AtomicBool::new(false),
        }
    }

    /// Replace the internal HTTP client (e.g. when proxy settings change).
    pub fn set_http_client(&self, client: reqwest::Client) {
        *self.client.lock().unwrap() = client;
    }

    /// Look up a recording MBID from an ISRC code.
    /// Uses title + artist to filter ambiguous results.
    /// Returns None on cache miss with no network result, or on error.
    pub async fn lookup_isrc(
        &self,
        isrc: &str,
        track_name: &str,
        artist_name: &str,
    ) -> Option<String> {
        // Check cache first
        {
            let cache = self.cache.lock().await;
            if let Some(cached) = cache.get(isrc) {
                return cached.clone();
            }
        }

        // Rate limit
        {
            let mut last = self.last_request.lock().await;
            let elapsed = last.elapsed();
            if elapsed < MIN_REQUEST_INTERVAL {
                tokio::time::sleep(MIN_REQUEST_INTERVAL - elapsed).await;
            }
            *last = Instant::now();
        }

        let result = self.fetch_mbid(isrc, track_name, artist_name).await;

        match result {
            Ok(mbid) => {
                let mut cache = self.cache.lock().await;
                cache.insert(isrc.to_string(), mbid.clone());
                self.dirty.store(true, Ordering::Relaxed);
                mbid
            }
            Err(e) => {
                // Don't cache network errors — allow retry next time
                log::debug!("MusicBrainz ISRC lookup failed for {isrc}: {e}");
                None
            }
        }
    }

    /// Resolve `(recording_mbid, release_group_mbid, artist_mbid)` for
    /// any track via a Lucene-quoted name search against MusicBrainz.
    /// Cached on disk in `mbid_name_cache.json` so a track plays the
    /// network cost once across the app's lifetime.
    ///
    /// Heuristic for picking the "best" recording when several match:
    ///  1. Title (case-insensitive) AND artist match → pick the one
    ///     whose first release sits in the most "official" release-group
    ///     (album type, then single, then anything else).
    ///  2. Title matches → first such recording.
    ///  3. Single recording in the page → take it.
    ///  4. Otherwise give up and cache `None` so we don't re-query.
    pub async fn lookup_by_name(
        &self,
        title: &str,
        artist: &str,
    ) -> Option<MbResolved> {
        let key = format!("{}|{}", title.to_lowercase(), artist.to_lowercase());
        {
            let cache = self.name_cache.lock().await;
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        // Rate limit shared with the ISRC path.
        {
            let mut last = self.last_request.lock().await;
            let elapsed = last.elapsed();
            if elapsed < MIN_REQUEST_INTERVAL {
                tokio::time::sleep(MIN_REQUEST_INTERVAL - elapsed).await;
            }
            *last = Instant::now();
        }

        let result = self.fetch_by_name(title, artist).await;
        match result {
            Ok(resolved) => {
                let mut cache = self.name_cache.lock().await;
                cache.insert(key, resolved.clone());
                self.name_dirty.store(true, Ordering::Relaxed);
                resolved
            }
            Err(e) => {
                log::debug!("[mb-name] lookup failed for {title} / {artist}: {e}");
                None
            }
        }
    }

    /// Stash an MBID resolution for a `(title, artist)` pair without
    /// going through the network. Used by the CAA cover fallback so
    /// once we've resolved a release-group, future plays of any track
    /// from that album benefit from the same cache entry without
    /// re-searching.
    pub async fn set_resolved_for_album(
        &self,
        album: &str,
        artist: &str,
        resolved: MbResolved,
    ) {
        // We key the name cache by `title|artist`; for an album-level
        // hint we synthesize a key tagged with `__album__` so it can't
        // collide with a real recording lookup.
        let key = format!(
            "__album__{}|{}",
            album.to_lowercase(),
            artist.to_lowercase()
        );
        let mut cache = self.name_cache.lock().await;
        cache.insert(key, Some(resolved));
        self.name_dirty.store(true, Ordering::Relaxed);
    }

    /// Mirror of `set_resolved_for_album` — read back the album-level
    /// cache entry if any. Useful when CAA was already consulted for
    /// this album so the UI doesn't refetch.
    pub async fn get_resolved_for_album(
        &self,
        album: &str,
        artist: &str,
    ) -> Option<MbResolved> {
        let key = format!(
            "__album__{}|{}",
            album.to_lowercase(),
            artist.to_lowercase()
        );
        self.name_cache.lock().await.get(&key)?.clone()
    }

    /// Persist both caches to disk if dirty. Call periodically or on shutdown.
    pub async fn persist(&self) {
        if self.dirty.swap(false, Ordering::Relaxed) {
            let cache = self.cache.lock().await;
            Self::atomic_write(&self.cache_path, &*cache, "ISRC MBID cache");
        }
        if self.name_dirty.swap(false, Ordering::Relaxed) {
            let cache = self.name_cache.lock().await;
            Self::atomic_write(&self.name_cache_path, &*cache, "name MBID cache");
        }
    }

    fn atomic_write<T: Serialize>(path: &PathBuf, data: &T, label: &str) {
        let json = match serde_json::to_vec_pretty(data) {
            Ok(j) => j,
            Err(e) => {
                log::warn!("Failed to serialize {label}: {e}");
                return;
            }
        };
        let tmp = path.with_extension("tmp");
        if let Err(e) = std::fs::write(&tmp, &json) {
            log::warn!("Failed to write {label} tmp: {e}");
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            log::warn!("Failed to rename {label}: {e}");
        }
    }

    // -----------------------------------------------------------------------
    // Private
    // -----------------------------------------------------------------------

    fn load_cache(path: &PathBuf) -> HashMap<String, Option<String>> {
        match std::fs::read(path) {
            Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    fn load_name_cache(path: &PathBuf) -> HashMap<String, Option<MbResolved>> {
        match std::fs::read(path) {
            Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    /// Quote a value to slot into a Lucene field clause. Escapes
    /// internal double-quotes and backslashes per MusicBrainz' search
    /// docs; everything else stays as-is so accents and CJK survive.
    fn lucene_quote(s: &str) -> String {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }

    async fn fetch_by_name(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<MbResolved>, String> {
        let query = format!(
            "recording:{} AND artist:{}",
            Self::lucene_quote(title),
            Self::lucene_quote(artist),
        );
        let user_agent = format!("SONE/{APP_VERSION} (https://github.com/lullabyX/sone)");
        let client = self.client.lock().unwrap().clone();
        let resp = client
            .get(format!("{MB_API_BASE}/recording/"))
            .query(&[
                ("query", query.as_str()),
                ("fmt", "json"),
                ("limit", "5"),
            ])
            .header(reqwest::header::USER_AGENT, &user_agent)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("parse failed: {e}"))?;

        let recordings = body
            .get("recordings")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        if recordings.is_empty() {
            return Ok(None);
        }

        let title_lower = title.to_lowercase();
        let artist_lower = artist.to_lowercase();

        // Score recordings: title match + artist match + has releases.
        let scored: Vec<(i32, &serde_json::Value)> = recordings
            .iter()
            .map(|r| (Self::score_recording(r, &title_lower, &artist_lower), r))
            .collect();

        let best = scored.iter().max_by_key(|(s, _)| *s).map(|(_, r)| r);
        let Some(rec) = best else { return Ok(None) };

        let recording_mbid = rec
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Pick the first artist-credit entry's artist id.
        let artist_mbid = rec
            .get("artist-credit")
            .and_then(|ac| ac.as_array())
            .and_then(|arr| arr.iter().find_map(|c| {
                c.get("artist")
                    .and_then(|a| a.get("id"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            }));

        // Prefer an "Album" release-group over a single. Fall back to
        // the first release-group seen.
        let release_group_mbid = rec
            .get("releases")
            .and_then(|r| r.as_array())
            .and_then(|releases| {
                let mut album_rg: Option<String> = None;
                let mut any_rg: Option<String> = None;
                for release in releases {
                    let rg = release.get("release-group");
                    let id = rg
                        .and_then(|g| g.get("id"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if id.is_none() {
                        continue;
                    }
                    let primary = rg
                        .and_then(|g| g.get("primary-type"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    if primary.eq_ignore_ascii_case("album") && album_rg.is_none() {
                        album_rg = id.clone();
                    }
                    if any_rg.is_none() {
                        any_rg = id;
                    }
                }
                album_rg.or(any_rg)
            });

        Ok(Some(MbResolved {
            recording_mbid,
            release_group_mbid,
            artist_mbid,
        }))
    }

    fn score_recording(
        rec: &serde_json::Value,
        title_lower: &str,
        artist_lower: &str,
    ) -> i32 {
        let mut score = 0;
        if rec
            .get("title")
            .and_then(|t| t.as_str())
            .map(|t| t.to_lowercase() == title_lower)
            .unwrap_or(false)
        {
            score += 100;
        }
        let artist_hit = rec
            .get("artist-credit")
            .and_then(|ac| ac.as_array())
            .map(|credits| {
                credits.iter().any(|c| {
                    c.get("name")
                        .or_else(|| c.get("artist").and_then(|a| a.get("name")))
                        .and_then(|n| n.as_str())
                        .map(|n| n.to_lowercase() == artist_lower)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if artist_hit {
            score += 50;
        }
        if rec
            .get("releases")
            .and_then(|r| r.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false)
        {
            score += 5;
        }
        if let Some(s) = rec.get("score").and_then(|v| v.as_i64()) {
            score += (s / 5) as i32; // MB's own confidence score, weighted lightly.
        }
        score
    }

    async fn fetch_mbid(
        &self,
        isrc: &str,
        track_name: &str,
        artist_name: &str,
    ) -> Result<Option<String>, String> {
        let url = format!("{MB_API_BASE}/isrc/{isrc}?fmt=json");

        let user_agent = format!("SONE/{APP_VERSION} (https://github.com/lullabyX/sone)");
        let client = self.client.lock().unwrap().clone();
        let resp = client
            .get(&url)
            .header(reqwest::header::USER_AGENT, &user_agent)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            // ISRC not found in MusicBrainz — cache as None
            return Ok(None);
        }
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("parse failed: {e}"))?;

        let recordings = body
            .get("recordings")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        if recordings.is_empty() {
            return Ok(None);
        }

        // Filter by case-insensitive title match
        let track_lower = track_name.to_lowercase();
        let artist_lower = artist_name.to_lowercase();

        let title_matched: Vec<&serde_json::Value> = recordings
            .iter()
            .filter(|r| {
                r.get("title")
                    .and_then(|t| t.as_str())
                    .map(|t| t.to_lowercase() == track_lower)
                    .unwrap_or(false)
            })
            .collect();

        // Try title + artist match first
        let best = title_matched.iter().find(|r| {
            r.get("artist-credit")
                .and_then(|ac| ac.as_array())
                .map(|credits| {
                    credits.iter().any(|c| {
                        c.get("name")
                            .or_else(|| c.get("artist").and_then(|a| a.get("name")))
                            .and_then(|n| n.as_str())
                            .map(|n| n.to_lowercase() == artist_lower)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });

        if let Some(recording) = best {
            return Ok(recording
                .get("id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string()));
        }

        // Fall back to first title match
        if let Some(recording) = title_matched.first() {
            return Ok(recording
                .get("id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string()));
        }

        // Last resort: take first only if there's exactly one recording
        if recordings.len() == 1 {
            return Ok(recordings[0]
                .get("id")
                .and_then(|id| id.as_str())
                .map(|s| s.to_string()));
        }

        // Multiple ambiguous results — return None
        Ok(None)
    }
}
