//! Last.fm public-data commands (no session, just the embedded API key).
//!
//! These three endpoints power features that don't need the user's
//! account: collaborative-filter "similar tracks" and community tag
//! clouds for tracks and artists. Useful even before the user has
//! created a Last.fm account, since the API key is enough for
//! read-only calls.

use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::SoneError;

const LASTFM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn user_agent() -> String {
    format!("SONE/{APP_VERSION} (https://github.com/lullabyX/sone)")
}

fn api_key() -> Option<String> {
    if crate::embedded_lastfm::has_stream_keys() {
        Some(crate::embedded_lastfm::stream_key_a())
    } else {
        None
    }
}

// --------------------------------------------------------------------
// Similar tracks
// --------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LfmSimilarTrack {
    pub name: String,
    pub artist: String,
    /// Last.fm match score in [0,1] — higher = more similar.
    pub match_score: f32,
    /// Recording MBID, when LFM has it linked. Useful for downstream
    /// resolving against TIDAL or MB.
    pub mbid: Option<String>,
    pub url: Option<String>,
    /// Last.fm playcount across all users, when present. Lets the UI
    /// hide deep cuts or surface popular picks.
    pub playcount: Option<u64>,
}

/// Collaborative-filter similar tracks for `(track, artist)`. Returns
/// up to `limit` tracks ranked by Last.fm's match score. No auth.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_lastfm_similar_tracks(
    _state: State<'_, AppState>,
    track: String,
    artist: String,
    limit: u32,
) -> Result<Vec<LfmSimilarTrack>, SoneError> {
    let key = api_key()
        .ok_or_else(|| SoneError::NotConfigured("Last.fm API key missing".into()))?;
    let limit = limit.clamp(1, 100);
    log::debug!("[lfm-similar] {track:?} / {artist:?} (limit={limit})");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| SoneError::Network(format!("client: {e}")))?;
    let resp = client
        .get(LASTFM_API_URL)
        .query(&[
            ("method", "track.getSimilar"),
            ("track", track.as_str()),
            ("artist", artist.as_str()),
            ("api_key", key.as_str()),
            ("format", "json"),
            ("autocorrect", "1"),
            ("limit", &limit.to_string()),
        ])
        .header(reqwest::header::USER_AGENT, user_agent())
        .send()
        .await?;
    if !resp.status().is_success() {
        return Ok(Vec::new());
    }
    let body: serde_json::Value = resp.json().await.map_err(SoneError::from)?;

    let arr = body
        .get("similartracks")
        .and_then(|v| v.get("track"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(arr.len());
    for raw in arr {
        let name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from);
        let artist_name = raw
            .get("artist")
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let Some(name) = name else { continue };
        let Some(artist_name) = artist_name else { continue };
        // Last.fm returns match as a stringly-typed number.
        let match_score = raw
            .get("match")
            .and_then(|v| match v {
                serde_json::Value::String(s) => s.parse::<f32>().ok(),
                serde_json::Value::Number(n) => n.as_f64().map(|x| x as f32),
                _ => None,
            })
            .unwrap_or(0.0);
        let mbid = raw
            .get("mbid")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let url = raw
            .get("url")
            .and_then(|v| v.as_str())
            .map(String::from);
        let playcount = raw
            .get("playcount")
            .and_then(|v| match v {
                serde_json::Value::String(s) => s.parse::<u64>().ok(),
                serde_json::Value::Number(n) => n.as_u64(),
                _ => None,
            });
        out.push(LfmSimilarTrack {
            name,
            artist: artist_name,
            match_score,
            mbid,
            url,
            playcount,
        });
    }
    Ok(out)
}

// --------------------------------------------------------------------
// Tags
// --------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct LfmTag {
    pub name: String,
    pub count: u32,
    pub url: Option<String>,
}

/// Top community tags for a track. Tags are user-applied, weighted by
/// how many users used each. Different in flavour from MB's curated
/// tags — this side is more mood / era / "vibe".
#[tauri::command(rename_all = "camelCase")]
pub async fn get_lastfm_track_tags(
    _state: State<'_, AppState>,
    track: String,
    artist: String,
) -> Result<Vec<LfmTag>, SoneError> {
    let key = api_key()
        .ok_or_else(|| SoneError::NotConfigured("Last.fm API key missing".into()))?;
    let body = lastfm_get(
        &[
            ("method", "track.getTopTags"),
            ("track", track.as_str()),
            ("artist", artist.as_str()),
            ("api_key", key.as_str()),
            ("format", "json"),
            ("autocorrect", "1"),
        ],
    )
    .await?;
    let tags = parse_tags(body.get("toptags").and_then(|v| v.get("tag")));
    Ok(tags)
}

/// Top community tags for an artist.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_lastfm_artist_tags(
    _state: State<'_, AppState>,
    artist: String,
) -> Result<Vec<LfmTag>, SoneError> {
    let key = api_key()
        .ok_or_else(|| SoneError::NotConfigured("Last.fm API key missing".into()))?;
    let body = lastfm_get(
        &[
            ("method", "artist.getTopTags"),
            ("artist", artist.as_str()),
            ("api_key", key.as_str()),
            ("format", "json"),
            ("autocorrect", "1"),
        ],
    )
    .await?;
    let tags = parse_tags(body.get("toptags").and_then(|v| v.get("tag")));
    Ok(tags)
}

// --------------------------------------------------------------------
// User stats (top tracks/artists/albums)
// --------------------------------------------------------------------
//
// Wraps `user.getTop*` so the Stats UI can pivot to the connected
// Last.fm user's view of their library. No session needed — the
// embedded API key + username is enough for read-only top-X.

fn lfm_period(window: &str) -> &'static str {
    match window {
        "week" => "7day",
        "month" => "1month",
        "year" => "12month",
        _ => "overall",
    }
}

async fn require_lfm_user(state: &State<'_, AppState>) -> Result<String, SoneError> {
    state
        .scrobble_manager
        .lastfm_username()
        .await
        .ok_or_else(|| SoneError::Scrobble("lastfm: not connected".into()))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_lastfm_user_top_tracks(
    state: State<'_, AppState>,
    window: String,
    limit: u32,
) -> Result<Vec<crate::stats::TopTrack>, SoneError> {
    let key = api_key()
        .ok_or_else(|| SoneError::NotConfigured("Last.fm API key missing".into()))?;
    let user = require_lfm_user(&state).await?;
    let body = lastfm_get(&[
        ("method", "user.getTopTracks"),
        ("user", user.as_str()),
        ("period", lfm_period(&window)),
        ("limit", &limit.to_string()),
        ("api_key", key.as_str()),
        ("format", "json"),
    ])
    .await?;
    let tracks = body
        .get("toptracks")
        .and_then(|t| t.get("track"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    let out = tracks
        .into_iter()
        .filter_map(|t| {
            let title = t.get("name").and_then(|v| v.as_str())?.to_string();
            let artist = t
                .get("artist")
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())?
                .to_string();
            let plays = t
                .get("playcount")
                .and_then(|v| match v {
                    serde_json::Value::String(s) => s.parse::<u64>().ok(),
                    serde_json::Value::Number(n) => n.as_u64(),
                    _ => None,
                })
                .unwrap_or(0);
            let duration = t
                .get("duration")
                .and_then(|v| match v {
                    serde_json::Value::String(s) => s.parse::<u64>().ok(),
                    serde_json::Value::Number(n) => n.as_u64(),
                    _ => None,
                })
                .unwrap_or(0);
            Some(crate::stats::TopTrack {
                track_id: None,
                title,
                artist,
                album: None,
                plays,
                listened_secs: duration.saturating_mul(plays),
            })
        })
        .collect();
    Ok(out)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_lastfm_user_top_artists(
    state: State<'_, AppState>,
    window: String,
    limit: u32,
) -> Result<Vec<crate::stats::TopArtist>, SoneError> {
    let key = api_key()
        .ok_or_else(|| SoneError::NotConfigured("Last.fm API key missing".into()))?;
    let user = require_lfm_user(&state).await?;
    let body = lastfm_get(&[
        ("method", "user.getTopArtists"),
        ("user", user.as_str()),
        ("period", lfm_period(&window)),
        ("limit", &limit.to_string()),
        ("api_key", key.as_str()),
        ("format", "json"),
    ])
    .await?;
    let artists = body
        .get("topartists")
        .and_then(|t| t.get("artist"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    let out = artists
        .into_iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|v| v.as_str())?.to_string();
            let plays = a
                .get("playcount")
                .and_then(|v| match v {
                    serde_json::Value::String(s) => s.parse::<u64>().ok(),
                    serde_json::Value::Number(n) => n.as_u64(),
                    _ => None,
                })
                .unwrap_or(0);
            Some(crate::stats::TopArtist {
                artist: name,
                plays,
                listened_secs: 0,
                distinct_tracks: 0,
            })
        })
        .collect();
    Ok(out)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_lastfm_user_top_albums(
    state: State<'_, AppState>,
    window: String,
    limit: u32,
) -> Result<Vec<crate::stats::TopAlbum>, SoneError> {
    let key = api_key()
        .ok_or_else(|| SoneError::NotConfigured("Last.fm API key missing".into()))?;
    let user = require_lfm_user(&state).await?;
    let body = lastfm_get(&[
        ("method", "user.getTopAlbums"),
        ("user", user.as_str()),
        ("period", lfm_period(&window)),
        ("limit", &limit.to_string()),
        ("api_key", key.as_str()),
        ("format", "json"),
    ])
    .await?;
    let albums = body
        .get("topalbums")
        .and_then(|t| t.get("album"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    let out = albums
        .into_iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|v| v.as_str())?.to_string();
            let artist = a
                .get("artist")
                .and_then(|x| x.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let plays = a
                .get("playcount")
                .and_then(|v| match v {
                    serde_json::Value::String(s) => s.parse::<u64>().ok(),
                    serde_json::Value::Number(n) => n.as_u64(),
                    _ => None,
                })
                .unwrap_or(0);
            Some(crate::stats::TopAlbum {
                album: name,
                artist,
                plays,
                listened_secs: 0,
            })
        })
        .collect();
    Ok(out)
}

async fn lastfm_get(params: &[(&str, &str)]) -> Result<serde_json::Value, SoneError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| SoneError::Network(format!("client: {e}")))?;
    let resp = client
        .get(LASTFM_API_URL)
        .query(params)
        .header(reqwest::header::USER_AGENT, user_agent())
        .send()
        .await?;
    if !resp.status().is_success() {
        return Ok(serde_json::Value::Null);
    }
    resp.json::<serde_json::Value>()
        .await
        .map_err(SoneError::from)
}

fn parse_tags(value: Option<&serde_json::Value>) -> Vec<LfmTag> {
    let arr = value.and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut out: Vec<LfmTag> = arr
        .into_iter()
        .filter_map(|raw| {
            let name = raw
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from)?;
            let count = raw
                .get("count")
                .and_then(|v| match v {
                    serde_json::Value::String(s) => s.parse::<u32>().ok(),
                    serde_json::Value::Number(n) => n.as_u64().map(|x| x as u32),
                    _ => None,
                })
                .unwrap_or(0);
            let url = raw
                .get("url")
                .and_then(|v| v.as_str())
                .map(String::from);
            Some(LfmTag { name, count, url })
        })
        .collect();
    // Last.fm returns tags ordered by count, but be defensive.
    out.sort_by(|a, b| b.count.cmp(&a.count));
    out.truncate(8);
    out
}
