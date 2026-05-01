use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::SoneError;

use super::{ScrobbleProvider, ScrobbleResult, ScrobbleTrack};

const API_BASE: &str = "https://api.listenbrainz.org";

// ---------------------------------------------------------------------------
// Token data
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenData {
    pub token: String,
    pub username: String,
}

/// One listen returned by GET /1/user/{user_name}/listens, distilled
/// down to what the local stats DB needs.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenItem {
    pub listened_at: i64,
    pub track_name: String,
    pub artist_name: String,
    pub release_name: Option<String>,
    pub duration_secs: Option<u32>,
    pub isrc: Option<String>,
    pub recording_mbid: Option<String>,
    pub release_mbid: Option<String>,
}

impl ListenItem {
    fn from_value(v: &Value) -> Option<Self> {
        let listened_at = v.get("listened_at")?.as_i64()?;
        let meta = v.get("track_metadata")?;
        let track_name = meta.get("track_name")?.as_str()?.to_string();
        let artist_name = meta.get("artist_name")?.as_str()?.to_string();
        if track_name.trim().is_empty() || artist_name.trim().is_empty() {
            return None;
        }
        let release_name = meta
            .get("release_name")
            .and_then(|s| s.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(String::from);

        let additional = meta.get("additional_info");
        let duration_secs = additional
            .and_then(|a| a.get("duration_ms").and_then(|d| d.as_u64()))
            .map(|ms| (ms / 1000) as u32)
            .or_else(|| {
                additional
                    .and_then(|a| a.get("duration").and_then(|d| d.as_u64()))
                    .map(|s| s as u32)
            });
        let isrc = additional
            .and_then(|a| a.get("isrc").and_then(|i| i.as_str()))
            .map(String::from);

        let mapping = meta.get("mbid_mapping");
        let recording_mbid = mapping
            .and_then(|m| m.get("recording_mbid").and_then(|s| s.as_str()))
            .or_else(|| {
                additional.and_then(|a| a.get("recording_mbid").and_then(|s| s.as_str()))
            })
            .map(String::from);
        let release_mbid = mapping
            .and_then(|m| m.get("release_mbid").and_then(|s| s.as_str()))
            .or_else(|| additional.and_then(|a| a.get("release_mbid").and_then(|s| s.as_str())))
            .map(String::from);

        Some(ListenItem {
            listened_at,
            track_name,
            artist_name,
            release_name,
            duration_secs,
            isrc,
            recording_mbid,
            release_mbid,
        })
    }
}

/// One page of listens from the public ListenBrainz API.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListensPage {
    pub count: u64,
    pub latest_ts: i64,
    pub oldest_ts: i64,
    pub listens: Vec<ListenItem>,
}

// ---------------------------------------------------------------------------
// ListenBrainzProvider
// ---------------------------------------------------------------------------

pub struct ListenBrainzProvider {
    token: RwLock<Option<TokenData>>,
    client: std::sync::Mutex<reqwest::Client>,
}

impl ListenBrainzProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            token: RwLock::new(None),
            client: std::sync::Mutex::new(client),
        }
    }

    pub async fn set_token(&self, token: String, username: String) {
        let mut data = self.token.write().await;
        *data = Some(TokenData { token, username });
    }

    /// Fetch a page of listens for a user from the public ListenBrainz API.
    /// Public profiles don't require auth, but we send the stored token if
    /// available so listens behind a private profile still resolve.
    ///
    /// Pagination is by timestamp: each call returns up to `count` listens
    /// older than `max_ts` (or older than now if not provided). The caller
    /// uses the `oldest_listen_ts` field of the response to set `max_ts`
    /// for the next page.
    pub async fn fetch_listens(
        client: &reqwest::Client,
        token: Option<&str>,
        username: &str,
        max_ts: Option<i64>,
        min_ts: Option<i64>,
        count: u32,
    ) -> Result<ListensPage, SoneError> {
        let mut req = client
            .get(format!("{API_BASE}/1/user/{username}/listens"))
            .timeout(Duration::from_secs(15));
        let mut query: Vec<(&str, String)> = vec![("count", count.to_string())];
        if let Some(ts) = max_ts {
            query.push(("max_ts", ts.to_string()));
        }
        if let Some(ts) = min_ts {
            query.push(("min_ts", ts.to_string()));
        }
        req = req.query(&query);
        if let Some(t) = token {
            req = req.header("Authorization", format!("Token {t}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| SoneError::Scrobble(format!("listens request failed: {e}")))?;
        let status = resp.status();
        if status.as_u16() == 401 {
            return Err(SoneError::Scrobble("listens: unauthorized".into()));
        }
        if status.as_u16() == 429 {
            return Err(SoneError::Scrobble("listens: rate limited".into()));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SoneError::Scrobble(format!("listens HTTP {status}: {body}")));
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SoneError::Scrobble(format!("listens parse failed: {e}")))?;

        let payload = body
            .get("payload")
            .ok_or_else(|| SoneError::Scrobble("listens: missing payload".into()))?;
        let listens_arr = payload
            .get("listens")
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();

        let mut items = Vec::with_capacity(listens_arr.len());
        for raw in listens_arr {
            if let Some(item) = ListenItem::from_value(&raw) {
                items.push(item);
            }
        }
        Ok(ListensPage {
            count: payload.get("count").and_then(|c| c.as_u64()).unwrap_or(0),
            latest_ts: payload
                .get("latest_listen_ts")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            oldest_ts: payload
                .get("oldest_listen_ts")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            listens: items,
        })
    }

    /// Validate a ListenBrainz user token. Returns the username on success.
    pub async fn validate_token(client: &reqwest::Client, token: &str) -> Result<String, SoneError> {
        let resp = client
            .get(format!("{API_BASE}/1/validate-token"))
            .header("Authorization", format!("Token {token}"))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SoneError::Scrobble(format!("validate-token request failed: {e}")))?;

        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SoneError::Scrobble(format!("validate-token parse failed: {e}")))?;

        if !status.is_success() || body.get("valid").and_then(|v| v.as_bool()) != Some(true) {
            let msg = body
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("invalid token");
            return Err(SoneError::Scrobble(format!("validate-token failed: {msg}")));
        }

        let username = body
            .get("user_name")
            .and_then(|u| u.as_str())
            .ok_or_else(|| SoneError::Scrobble("validate-token: missing user_name".into()))?;

        Ok(username.to_string())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn get_token(&self) -> Result<String, SoneError> {
        let data = self.token.read().await;
        data.as_ref()
            .map(|d| d.token.clone())
            .ok_or_else(|| SoneError::Scrobble("listenbrainz: not authenticated".into()))
    }

    fn build_track_metadata(track: &ScrobbleTrack) -> Value {
        const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

        let mut additional_info = json!({
            "media_player": "SONE",
            "media_player_version": APP_VERSION,
            "submission_client": "SONE",
            "submission_client_version": APP_VERSION,
            "music_service": "tidal.com",
        });

        if track.duration_secs > 0 {
            additional_info["duration"] = json!(track.duration_secs);
        }
        if let Some(track_number) = track.track_number {
            additional_info["tracknumber"] = json!(track_number.to_string());
        }
        if let Some(ref isrc) = track.isrc {
            additional_info["isrc"] = json!(isrc);
        }
        if let Some(ref mbid) = track.recording_mbid {
            additional_info["recording_mbid"] = json!(mbid);
        }
        if let Some(track_id) = track.track_id {
            additional_info["origin_url"] =
                json!(format!("https://listen.tidal.com/track/{track_id}"));
        }

        let mut metadata = json!({
            "artist_name": track.artist,
            "track_name": track.track,
            "additional_info": additional_info,
        });

        if let Some(ref album) = track.album {
            metadata["release_name"] = json!(album);
        }

        metadata
    }

    async fn submit(
        &self,
        listen_type: &str,
        payload: Vec<Value>,
    ) -> Result<reqwest::Response, SoneError> {
        let token = self.get_token().await?;

        let body = json!({
            "listen_type": listen_type,
            "payload": payload,
        });

        let client = self.client.lock().unwrap().clone();
        let resp = client
            .post(format!("{API_BASE}/1/submit-listens"))
            .header("Authorization", format!("Token {token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| SoneError::Scrobble(format!("submit-listens request failed: {e}")))?;

        Ok(resp)
    }
}

// ---------------------------------------------------------------------------
// ScrobbleProvider trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ScrobbleProvider for ListenBrainzProvider {
    fn name(&self) -> &str {
        "listenbrainz"
    }

    fn is_authenticated(&self) -> bool {
        self.token.try_read().map(|t| t.is_some()).unwrap_or(false)
    }

    fn max_batch_size(&self) -> usize {
        1000
    }

    fn set_http_client(&self, client: reqwest::Client) {
        *self.client.lock().unwrap() = client;
    }

    async fn username(&self) -> Option<String> {
        let data = self.token.read().await;
        data.as_ref().map(|d| d.username.clone())
    }

    async fn now_playing(&self, track: &ScrobbleTrack) -> ScrobbleResult {
        let metadata = Self::build_track_metadata(track);
        let payload = vec![json!({
            "track_metadata": metadata,
        })];

        let resp = match self.submit("playing_now", payload).await {
            Ok(r) => r,
            Err(_) => return ScrobbleResult::Retryable("request failed".into()),
        };

        let status = resp.status();
        if status.as_u16() == 401 {
            return ScrobbleResult::AuthError("unauthorized".into());
        }
        if !status.is_success() {
            log::warn!("listenbrainz: now_playing returned {status}");
        }

        ScrobbleResult::Ok
    }

    async fn scrobble(&self, tracks: &[ScrobbleTrack]) -> ScrobbleResult {
        let listen_type = if tracks.len() == 1 {
            "single"
        } else {
            "import"
        };

        let payload: Vec<Value> = tracks
            .iter()
            .map(|track| {
                let metadata = Self::build_track_metadata(track);
                json!({
                    "listened_at": track.timestamp,
                    "track_metadata": metadata,
                })
            })
            .collect();

        let resp = match self.submit(listen_type, payload).await {
            Ok(r) => r,
            Err(_) => return ScrobbleResult::Retryable("request failed".into()),
        };

        let status = resp.status();

        if status.as_u16() == 401 {
            return ScrobbleResult::AuthError("unauthorized".into());
        }

        if status.as_u16() == 429 {
            // Read X-RateLimit-Reset-In header for retry info
            let reset_in = resp
                .headers()
                .get("X-RateLimit-Reset-In")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown");
            return ScrobbleResult::Retryable(format!("rate limited, reset in {reset_in}s"));
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return ScrobbleResult::Retryable(format!("HTTP {status}: {body}"));
        }

        ScrobbleResult::Ok
    }
}
