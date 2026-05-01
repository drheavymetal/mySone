use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::SoneError;

use super::{ScrobbleProvider, ScrobbleResult, ScrobbleTrack};

// ---------------------------------------------------------------------------
// Recent-tracks (history import)
// ---------------------------------------------------------------------------

/// One scrobble returned by `user.getRecentTracks`, distilled to the
/// fields the local stats DB needs.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTrackItem {
    pub listened_at: i64,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: Option<String>,
    pub recording_mbid: Option<String>,
}

/// One page of `user.getRecentTracks`. `total_pages` lets the importer
/// stop once it walks past the last available page.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTracksPage {
    pub page: u32,
    pub total_pages: u32,
    pub tracks: Vec<RecentTrackItem>,
}

/// Fetch one page of a user's recent scrobbles. No session needed —
/// the embedded API key plus a public username is enough. The
/// `nowplaying` entry (the track currently being scrobbled, no `date`
/// field) is filtered out so it doesn't poison the import with fake
/// listened_at=0 rows.
pub async fn fetch_recent_tracks(
    client: &reqwest::Client,
    api_key: &str,
    username: &str,
    from_ts: Option<i64>,
    page: u32,
    limit: u32,
) -> Result<RecentTracksPage, SoneError> {
    let limit = limit.clamp(1, 200);
    let mut query: Vec<(&str, String)> = vec![
        ("method", "user.getRecentTracks".into()),
        ("user", username.into()),
        ("api_key", api_key.into()),
        ("format", "json".into()),
        ("limit", limit.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if let Some(ts) = from_ts {
        if ts > 0 {
            query.push(("from", ts.to_string()));
        }
    }

    let resp = client
        .get("https://ws.audioscrobbler.com/2.0/")
        .query(&query)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| SoneError::Scrobble(format!("getRecentTracks request failed: {e}")))?;
    let status = resp.status();
    if status.as_u16() == 429 {
        return Err(SoneError::Scrobble("getRecentTracks: rate limited".into()));
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(SoneError::Scrobble(format!(
            "getRecentTracks HTTP {status}: {body}"
        )));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SoneError::Scrobble(format!("getRecentTracks parse failed: {e}")))?;

    if let Some(code) = body.get("error").and_then(|c| c.as_u64()) {
        let msg = body.get("message").and_then(|m| m.as_str()).unwrap_or("");
        return Err(SoneError::Scrobble(format!(
            "getRecentTracks error {code}: {msg}"
        )));
    }

    let recent = body
        .get("recenttracks")
        .ok_or_else(|| SoneError::Scrobble("getRecentTracks: missing recenttracks".into()))?;

    let attr = recent.get("@attr");
    let total_pages = attr
        .and_then(|a| a.get("totalPages"))
        .and_then(|v| match v {
            Value::String(s) => s.parse::<u32>().ok(),
            Value::Number(n) => n.as_u64().map(|x| x as u32),
            _ => None,
        })
        .unwrap_or(0);
    let cur_page = attr
        .and_then(|a| a.get("page"))
        .and_then(|v| match v {
            Value::String(s) => s.parse::<u32>().ok(),
            Value::Number(n) => n.as_u64().map(|x| x as u32),
            _ => None,
        })
        .unwrap_or(page);

    // `track` is `[]` when empty, an array when there are several, but
    // sometimes a single-object when there's exactly one (cheap LFM JSON
    // quirk). Coerce to a Vec uniformly.
    let raw_tracks: Vec<Value> = match recent.get("track") {
        Some(Value::Array(a)) => a.clone(),
        Some(v @ Value::Object(_)) => vec![v.clone()],
        _ => Vec::new(),
    };

    let mut tracks = Vec::with_capacity(raw_tracks.len());
    for raw in raw_tracks {
        // Skip the now-playing entry (no `date` -> `@attr.nowplaying`).
        if raw
            .get("@attr")
            .and_then(|a| a.get("nowplaying"))
            .is_some()
        {
            continue;
        }
        let listened_at = raw
            .get("date")
            .and_then(|d| d.get("uts"))
            .and_then(|v| match v {
                Value::String(s) => s.parse::<i64>().ok(),
                Value::Number(n) => n.as_i64(),
                _ => None,
            });
        let Some(listened_at) = listened_at else {
            continue;
        };
        let track_name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let artist_name = raw
            .get("artist")
            .and_then(|a| a.get("#text").or_else(|| a.get("name")))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if track_name.is_empty() || artist_name.is_empty() {
            continue;
        }
        let album_name = raw
            .get("album")
            .and_then(|a| a.get("#text").or_else(|| a.get("name")))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let recording_mbid = raw
            .get("mbid")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        tracks.push(RecentTrackItem {
            listened_at,
            track_name,
            artist_name,
            album_name,
            recording_mbid,
        });
    }

    Ok(RecentTracksPage {
        page: cur_page,
        total_pages,
        tracks,
    })
}

// ---------------------------------------------------------------------------
// Session data
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionData {
    pub session_key: String,
    pub username: String,
}

// ---------------------------------------------------------------------------
// AudioscrobblerProvider
// ---------------------------------------------------------------------------

pub struct AudioscrobblerProvider {
    name: &'static str,
    api_url: &'static str,
    auth_base_url: &'static str,
    api_key: String,
    api_secret: String,
    session: RwLock<Option<SessionData>>,
    client: std::sync::Mutex<reqwest::Client>,
}

impl AudioscrobblerProvider {
    pub fn new(
        name: &'static str,
        api_url: &'static str,
        auth_base_url: &'static str,
        api_key: String,
        api_secret: String,
        client: reqwest::Client,
    ) -> Self {
        Self {
            name,
            api_url,
            auth_base_url,
            api_key,
            api_secret,
            session: RwLock::new(None),
            client: std::sync::Mutex::new(client),
        }
    }

    pub async fn set_session(&self, session_key: String, username: String) {
        let mut session = self.session.write().await;
        *session = Some(SessionData {
            session_key,
            username,
        });
    }

    /// Fetch an unauthorized request token from the API (desktop auth step 2).
    pub async fn get_token(&self) -> Result<String, SoneError> {
        let mut params = BTreeMap::new();
        params.insert("method", "auth.getToken".to_string());
        params.insert("api_key", self.api_key.clone());

        let sig = self.sign(&params);
        params.insert("api_sig", sig);
        params.insert("format", "json".to_string());

        let client = self.client.lock().unwrap().clone();
        let resp = client
            .get(self.api_url)
            .query(&params)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| SoneError::Scrobble(format!("auth.getToken request failed: {e}")))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SoneError::Scrobble(format!("auth.getToken parse failed: {e}")))?;

        if let Some(err_code) = Self::parse_error_code(&body) {
            return Err(SoneError::Scrobble(format!(
                "auth.getToken error {err_code}: {}",
                body.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error")
            )));
        }

        body.get("token")
            .and_then(|t| t.as_str())
            .map(|t| t.to_string())
            .ok_or_else(|| SoneError::Scrobble("auth.getToken: missing token".into()))
    }

    /// Generate the browser auth URL for the user to grant access (desktop auth step 3).
    /// The token must be obtained from `get_token()` first.
    pub fn auth_url_with_token(&self, token: &str) -> String {
        format!(
            "{}?api_key={}&token={}",
            self.auth_base_url, self.api_key, token
        )
    }

    /// Exchange an auth token for a permanent session key.
    /// Returns (session_key, username).
    pub async fn get_session(&self, token: &str) -> Result<(String, String), SoneError> {
        let mut params = BTreeMap::new();
        params.insert("method", "auth.getSession".to_string());
        params.insert("api_key", self.api_key.clone());
        params.insert("token", token.to_string());

        let sig = self.sign(&params);
        params.insert("api_sig", sig);
        params.insert("format", "json".to_string());

        let client = self.client.lock().unwrap().clone();
        let resp = client
            .post(self.api_url)
            .form(&params)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| SoneError::Scrobble(format!("auth.getSession request failed: {e}")))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SoneError::Scrobble(format!("auth.getSession parse failed: {e}")))?;

        if let Some(err_code) = Self::parse_error_code(&body) {
            return Err(SoneError::Scrobble(format!(
                "auth.getSession error {err_code}: {}",
                body.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error")
            )));
        }

        let session = body
            .get("session")
            .ok_or_else(|| SoneError::Scrobble("auth.getSession: missing session".into()))?;
        let key = session
            .get("key")
            .and_then(|k| k.as_str())
            .ok_or_else(|| SoneError::Scrobble("auth.getSession: missing key".into()))?;
        let name = session
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| SoneError::Scrobble("auth.getSession: missing name".into()))?;

        Ok((key.to_string(), name.to_string()))
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// MD5 API signature.
    /// Algorithm: sort params alphabetically by key, concatenate key1value1key2value2...,
    /// append api_secret, compute MD5 hex digest.
    /// The "format" and "callback" params are excluded from the signature.
    fn sign(&self, params: &BTreeMap<&str, String>) -> String {
        let mut sig_input = String::new();
        for (k, v) in params {
            if *k == "format" || *k == "callback" {
                continue;
            }
            sig_input.push_str(k);
            sig_input.push_str(v);
        }
        sig_input.push_str(&self.api_secret);
        format!("{:x}", md5::compute(sig_input.as_bytes()))
    }

    /// Same as `sign` but for indexed params (batch scrobbles like artist[0], track[0]).
    /// Params are already sorted as (String, String) tuples.
    fn sign_indexed(&self, params: &[(String, String)]) -> String {
        // Sort by key alphabetically
        let mut sorted: Vec<(&str, &str)> = params
            .iter()
            .filter(|(k, _)| k != "format" && k != "callback")
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        sorted.sort_by_key(|(k, _)| *k);

        let mut sig_input = String::new();
        for (k, v) in sorted {
            sig_input.push_str(k);
            sig_input.push_str(v);
        }
        sig_input.push_str(&self.api_secret);
        format!("{:x}", md5::compute(sig_input.as_bytes()))
    }

    /// Get session key or return an error.
    async fn session_key(&self) -> Result<String, SoneError> {
        let session = self.session.read().await;
        session
            .as_ref()
            .map(|s| s.session_key.clone())
            .ok_or_else(|| SoneError::Scrobble(format!("{}: not authenticated", self.name)))
    }

    /// Parse an error code from a Last.fm API JSON response.
    fn parse_error_code(body: &serde_json::Value) -> Option<u32> {
        body.get("error").and_then(|e| e.as_u64()).map(|e| e as u32)
    }
}

// ---------------------------------------------------------------------------
// ScrobbleProvider trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ScrobbleProvider for AudioscrobblerProvider {
    fn name(&self) -> &str {
        self.name
    }

    fn is_authenticated(&self) -> bool {
        self.session
            .try_read()
            .map(|s| s.is_some())
            .unwrap_or(false)
    }

    fn max_batch_size(&self) -> usize {
        50
    }

    fn set_http_client(&self, client: reqwest::Client) {
        *self.client.lock().unwrap() = client;
    }

    async fn username(&self) -> Option<String> {
        let session = self.session.read().await;
        session.as_ref().map(|s| s.username.clone())
    }

    async fn now_playing(&self, track: &ScrobbleTrack) -> ScrobbleResult {
        let sk = match self.session_key().await {
            Ok(sk) => sk,
            Err(_) => return ScrobbleResult::AuthError("not authenticated".into()),
        };

        let mut params = BTreeMap::new();
        params.insert("method", "track.updateNowPlaying".to_string());
        params.insert("artist", track.artist.clone());
        params.insert("track", track.track.clone());
        params.insert("api_key", self.api_key.clone());
        params.insert("sk", sk);
        params.insert("duration", track.duration_secs.to_string());

        if let Some(ref album) = track.album {
            params.insert("album", album.clone());
        }
        if let Some(ref album_artist) = track.album_artist {
            params.insert("albumArtist", album_artist.clone());
        }
        if let Some(track_number) = track.track_number {
            params.insert("trackNumber", track_number.to_string());
        }

        let sig = self.sign(&params);
        params.insert("api_sig", sig);
        params.insert("format", "json".to_string());

        let client = self.client.lock().unwrap().clone();
        let result = client
            .post(self.api_url)
            .form(&params)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(resp) => {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(code) = Self::parse_error_code(&body) {
                        let msg = body
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        log::warn!("{}: now_playing error {code}: {msg}", self.name);
                        // Auth errors must be surfaced so the provider gets disconnected
                        if matches!(code, 9 | 10 | 26) {
                            return ScrobbleResult::AuthError(msg);
                        }
                        // All other now_playing failures are non-critical
                        return ScrobbleResult::Ok;
                    }
                }
                ScrobbleResult::Ok
            }
            Err(e) => {
                log::warn!("{}: now_playing failed: {e}", self.name);
                // Network failures for now_playing are non-critical
                ScrobbleResult::Ok
            }
        }
    }

    async fn scrobble(&self, tracks: &[ScrobbleTrack]) -> ScrobbleResult {
        let sk = match self.session_key().await {
            Ok(sk) => sk,
            Err(_) => return ScrobbleResult::AuthError("not authenticated".into()),
        };

        // Build indexed params for batch scrobble
        let mut params: Vec<(String, String)> = Vec::new();
        params.push(("method".to_string(), "track.scrobble".to_string()));
        params.push(("api_key".to_string(), self.api_key.clone()));
        params.push(("sk".to_string(), sk));

        for (i, track) in tracks.iter().enumerate() {
            params.push((format!("artist[{i}]"), track.artist.clone()));
            params.push((format!("track[{i}]"), track.track.clone()));
            params.push((format!("timestamp[{i}]"), track.timestamp.to_string()));
            params.push((format!("duration[{i}]"), track.duration_secs.to_string()));

            if let Some(ref album) = track.album {
                params.push((format!("album[{i}]"), album.clone()));
            }
            if let Some(ref album_artist) = track.album_artist {
                params.push((format!("albumArtist[{i}]"), album_artist.clone()));
            }
            if let Some(track_number) = track.track_number {
                params.push((format!("trackNumber[{i}]"), track_number.to_string()));
            }
            if !track.chosen_by_user {
                params.push((format!("chosenByUser[{i}]"), "0".to_string()));
            }
            if let Some(ref mbid) = track.recording_mbid {
                params.push((format!("mbid[{i}]"), mbid.clone()));
            }
        }

        let sig = self.sign_indexed(&params);
        params.push(("api_sig".to_string(), sig));
        params.push(("format".to_string(), "json".to_string()));

        let client = self.client.lock().unwrap().clone();
        let result = client
            .post(self.api_url)
            .form(&params)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(resp) => {
                let body: serde_json::Value = match resp.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        return ScrobbleResult::Retryable(format!("response parse error: {e}"));
                    }
                };

                if let Some(code) = Self::parse_error_code(&body) {
                    let msg = body
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error")
                        .to_string();

                    return match code {
                        // 9 = Invalid session, 10 = Invalid API key, 26 = Key suspended
                        9 | 10 | 26 => ScrobbleResult::AuthError(msg),
                        // 8 = Temporary error, 11 = Service offline,
                        // 16 = Temporarily unavailable, 29 = Rate limit
                        8 | 11 | 16 | 29 => ScrobbleResult::Retryable(msg),
                        // All other errors are permanent failures — do not retry
                        _ => {
                            log::error!("{}: permanent scrobble error {code}: {msg}", self.name);
                            ScrobbleResult::Ok
                        }
                    };
                }

                ScrobbleResult::Ok
            }
            Err(e) => ScrobbleResult::Retryable(format!("request failed: {e}")),
        }
    }
}
