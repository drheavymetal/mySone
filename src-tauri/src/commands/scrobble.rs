use serde::{Deserialize, Serialize};
use tauri::State;

use crate::scrobble::listenbrainz::ListenBrainzProvider;
use crate::scrobble::{ProviderStatus, ScrobbleTrack};
use crate::{AppState, LastfmCredentials, ListenBrainzCredentials, SoneError};

#[derive(Debug, Serialize)]
pub struct AuthStartResponse {
    pub url: String,
    pub token: String,
}

// ---------------------------------------------------------------------------
// Payload types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStartedPayload {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration_secs: u32,
    pub track_number: Option<u32>,
    pub chosen_by_user: bool,
    pub isrc: Option<String>,
    pub track_id: Option<u64>,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "camelCase")]
pub async fn notify_track_started(
    state: State<'_, AppState>,
    payload: TrackStartedPayload,
) -> Result<(), SoneError> {
    let track = ScrobbleTrack {
        artist: payload.artist,
        track: payload.title,
        album: payload.album,
        album_artist: payload.album_artist,
        duration_secs: payload.duration_secs,
        track_number: payload.track_number,
        timestamp: crate::now_secs() as i64,
        chosen_by_user: payload.chosen_by_user,
        isrc: payload.isrc,
        release_group_mbid: None,
        artist_mbid: None,
        track_id: payload.track_id,
        recording_mbid: None,
    };
    state.hooks.on_track_started(&track);
    state.scrobble_manager.on_track_started(track).await;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn notify_track_paused(state: State<'_, AppState>) -> Result<(), SoneError> {
    state.hooks.on_pause();
    state.scrobble_manager.on_pause().await;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn notify_track_resumed(state: State<'_, AppState>) -> Result<(), SoneError> {
    state.hooks.on_resume();
    state.scrobble_manager.on_resume().await;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn notify_track_seeked(state: State<'_, AppState>) -> Result<(), SoneError> {
    state.scrobble_manager.on_seek().await;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn notify_track_stopped(state: State<'_, AppState>) -> Result<(), SoneError> {
    state.hooks.on_stop();
    state.scrobble_manager.on_track_stopped().await;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_scrobble_status(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderStatus>, SoneError> {
    Ok(state.scrobble_manager.provider_statuses().await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_scrobble_queue_size(state: State<'_, AppState>) -> Result<usize, SoneError> {
    Ok(state.scrobble_manager.queue_size().await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn connect_listenbrainz(
    state: State<'_, AppState>,
    token: String,
) -> Result<String, SoneError> {
    let http_client = {
        let client = state.tidal_client.lock().await;
        client.raw_client().clone()
    };
    let username = ListenBrainzProvider::validate_token(&http_client, &token).await?;

    // Create and register the provider
    let provider = ListenBrainzProvider::new(http_client);
    provider.set_token(token.clone(), username.clone()).await;
    state
        .scrobble_manager
        .add_provider(Box::new(provider))
        .await;

    // Save credentials to settings
    if let Some(mut settings) = state.load_settings() {
        settings.scrobble.listenbrainz = Some(ListenBrainzCredentials {
            token,
            username: username.clone(),
        });
        state.save_settings(&settings)?;
    }

    Ok(username)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn disconnect_provider(
    state: State<'_, AppState>,
    provider: String,
) -> Result<(), SoneError> {
    // Clear credentials from settings
    if let Some(mut settings) = state.load_settings() {
        match provider.as_str() {
            "lastfm" => settings.scrobble.lastfm = None,
            "listenbrainz" => settings.scrobble.listenbrainz = None,
            "librefm" => settings.scrobble.librefm = None,
            _ => {
                return Err(SoneError::Scrobble(format!("unknown provider: {provider}")));
            }
        }
        state.save_settings(&settings)?;
    }

    state.scrobble_manager.remove_provider(&provider).await;
    Ok(())
}

/// Fetch a request token and return the auth URL + token for Last.fm desktop auth.
#[tauri::command(rename_all = "camelCase")]
pub async fn connect_lastfm(state: State<'_, AppState>) -> Result<AuthStartResponse, SoneError> {
    if !crate::embedded_lastfm::has_stream_keys() {
        return Err(SoneError::Scrobble("Last.fm not configured".into()));
    }
    let http_client = {
        let client = state.tidal_client.lock().await;
        client.raw_client().clone()
    };
    let provider = crate::scrobble::lastfm::AudioscrobblerProvider::new(
        "lastfm",
        "https://ws.audioscrobbler.com/2.0/",
        "https://www.last.fm/api/auth/",
        crate::embedded_lastfm::stream_key_a(),
        crate::embedded_lastfm::stream_key_b(),
        http_client,
    );
    let token = provider.get_token().await?;
    let url = provider.auth_url_with_token(&token);
    Ok(AuthStartResponse { url, token })
}

/// Fetch a request token and return the auth URL + token for Libre.fm desktop auth.
#[tauri::command(rename_all = "camelCase")]
pub async fn connect_librefm(state: State<'_, AppState>) -> Result<AuthStartResponse, SoneError> {
    if !crate::embedded_librefm::has_stream_keys() {
        return Err(SoneError::Scrobble("Libre.fm not configured".into()));
    }
    let http_client = {
        let client = state.tidal_client.lock().await;
        client.raw_client().clone()
    };
    let provider = crate::scrobble::lastfm::AudioscrobblerProvider::new(
        "librefm",
        crate::scrobble::librefm::LIBREFM_API_URL,
        "https://libre.fm/api/auth/",
        crate::embedded_librefm::stream_key_a(),
        crate::embedded_librefm::stream_key_b(),
        http_client,
    );
    let token = provider.get_token().await?;
    let url = provider.auth_url_with_token(&token);
    Ok(AuthStartResponse { url, token })
}

/// Exchange an auth token for a permanent session key.
/// The frontend calls this after the user authorizes in the browser and
/// provides the token.
#[tauri::command(rename_all = "camelCase")]
pub async fn complete_audioscrobbler_auth(
    state: State<'_, AppState>,
    provider_name: String,
    token: String,
) -> Result<String, SoneError> {
    let (api_key, api_secret, api_url, auth_base_url) = match provider_name.as_str() {
        "lastfm" => {
            if !crate::embedded_lastfm::has_stream_keys() {
                return Err(SoneError::Scrobble("Last.fm not configured".into()));
            }
            (
                crate::embedded_lastfm::stream_key_a(),
                crate::embedded_lastfm::stream_key_b(),
                "https://ws.audioscrobbler.com/2.0/",
                "https://www.last.fm/api/auth/",
            )
        }
        "librefm" => {
            if !crate::embedded_librefm::has_stream_keys() {
                return Err(SoneError::Scrobble("Libre.fm not configured".into()));
            }
            (
                crate::embedded_librefm::stream_key_a(),
                crate::embedded_librefm::stream_key_b(),
                crate::scrobble::librefm::LIBREFM_API_URL,
                "https://libre.fm/api/auth/",
            )
        }
        _ => {
            return Err(SoneError::Scrobble(format!(
                "Unknown provider: {provider_name}"
            )));
        }
    };

    let http_client = {
        let client = state.tidal_client.lock().await;
        client.raw_client().clone()
    };
    let provider = crate::scrobble::lastfm::AudioscrobblerProvider::new(
        if provider_name == "lastfm" {
            "lastfm"
        } else {
            "librefm"
        },
        api_url,
        auth_base_url,
        api_key,
        api_secret,
        http_client,
    );

    let (session_key, username) = provider.get_session(&token).await?;
    provider
        .set_session(session_key.clone(), username.clone())
        .await;

    // Save credentials
    if let Some(mut settings) = state.load_settings() {
        let creds = LastfmCredentials {
            session_key,
            username: username.clone(),
        };
        match provider_name.as_str() {
            "lastfm" => settings.scrobble.lastfm = Some(creds),
            "librefm" => settings.scrobble.librefm = Some(creds),
            _ => {}
        }
        state.save_settings(&settings)?;
    }

    // Register provider with the scrobble manager
    state
        .scrobble_manager
        .add_provider(Box::new(provider))
        .await;

    Ok(username)
}

/// Pull the user's ListenBrainz history into the local stats DB.
/// Pass `sinceUnix` to limit how far back the importer walks; if omitted
/// the walk continues until the page yielded mostly duplicates or the
/// per-call page cap is reached. Streams progress via the
/// `import-listenbrainz-progress` event.
#[tauri::command(rename_all = "camelCase")]
pub async fn import_listenbrainz_history(
    state: State<'_, AppState>,
    since_unix: Option<i64>,
) -> Result<crate::scrobble::ImportResult, SoneError> {
    log::info!("[import_listenbrainz_history]: since_unix={:?}", since_unix);
    state
        .scrobble_manager
        .import_listenbrainz_history(since_unix)
        .await
}

// --------------------------------------------------------------------
// ListenBrainz remote stats
// --------------------------------------------------------------------
//
// Wraps the public `/1/stats/user/{user_name}/top-*` endpoints so the
// Stats UI can pivot from local plays to whatever ListenBrainz has on
// record for the connected user. Public-profile-only — for private
// profiles the API returns 404. The toggle in the UI surfaces that as
// a friendly "no remote stats" empty state.

/// Map our window enum onto LB's `range` parameter.
fn lb_range(window: &str) -> &'static str {
    match window {
        "week" => "week",
        "month" => "month",
        "year" => "year",
        _ => "all_time",
    }
}

async fn lb_get(
    state: &State<'_, AppState>,
    path_after_user: &str,
    range: &str,
    extra_query: &[(&str, String)],
) -> Result<serde_json::Value, SoneError> {
    let username = state
        .scrobble_manager
        .listenbrainz_username()
        .await
        .ok_or_else(|| SoneError::Scrobble("listenbrainz: not connected".into()))?;
    let url = format!(
        "https://api.listenbrainz.org/1/stats/user/{username}/{path_after_user}"
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| SoneError::Network(format!("client: {e}")))?;
    let mut query: Vec<(&str, String)> = vec![("range", range.to_string())];
    for (k, v) in extra_query {
        query.push((k, v.clone()));
    }
    let resp = client.get(&url).query(&query).send().await?;
    let status = resp.status();
    if status.as_u16() == 204 || status.as_u16() == 404 {
        // 204 = "no data yet for this range"; 404 = private profile.
        return Ok(serde_json::Value::Null);
    }
    if !status.is_success() {
        return Err(SoneError::Network(format!("listenbrainz HTTP {status}")));
    }
    Ok(resp.json::<serde_json::Value>().await?)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_listenbrainz_top_tracks(
    state: State<'_, AppState>,
    window: String,
    limit: u32,
) -> Result<Vec<crate::stats::TopTrack>, SoneError> {
    let body = lb_get(
        &state,
        "top-recordings",
        lb_range(&window),
        &[("count", limit.to_string())],
    )
    .await?;
    let recordings = body
        .get("payload")
        .and_then(|p| p.get("recordings"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let out = recordings
        .into_iter()
        .filter_map(|r| {
            let title = r.get("track_name").and_then(|v| v.as_str())?.to_string();
            let artist = r
                .get("artist_name")
                .and_then(|v| v.as_str())?
                .to_string();
            let album = r
                .get("release_name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            let plays = r
                .get("listen_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(crate::stats::TopTrack {
                track_id: None,
                title,
                artist,
                album,
                plays,
                listened_secs: 0,
            })
        })
        .collect();
    Ok(out)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_listenbrainz_top_artists(
    state: State<'_, AppState>,
    window: String,
    limit: u32,
) -> Result<Vec<crate::stats::TopArtist>, SoneError> {
    let body = lb_get(
        &state,
        "top-artists",
        lb_range(&window),
        &[("count", limit.to_string())],
    )
    .await?;
    let artists = body
        .get("payload")
        .and_then(|p| p.get("artists"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let out = artists
        .into_iter()
        .filter_map(|r| {
            let artist = r
                .get("artist_name")
                .and_then(|v| v.as_str())?
                .to_string();
            let plays = r
                .get("listen_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(crate::stats::TopArtist {
                artist,
                plays,
                listened_secs: 0,
                distinct_tracks: 0,
            })
        })
        .collect();
    Ok(out)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_listenbrainz_top_albums(
    state: State<'_, AppState>,
    window: String,
    limit: u32,
) -> Result<Vec<crate::stats::TopAlbum>, SoneError> {
    let body = lb_get(
        &state,
        "release-groups",
        lb_range(&window),
        &[("count", limit.to_string())],
    )
    .await?;
    // LB also has /top-releases for editions; release-groups merges
    // reissues/remasters which is what we want for "Top Albums".
    let groups = body
        .get("payload")
        .and_then(|p| p.get("release_groups"))
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let out = groups
        .into_iter()
        .filter_map(|r| {
            let album = r
                .get("release_group_name")
                .and_then(|v| v.as_str())?
                .to_string();
            let artist = r
                .get("artist_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let plays = r
                .get("listen_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(crate::stats::TopAlbum {
                album,
                artist,
                plays,
                listened_secs: 0,
            })
        })
        .collect();
    Ok(out)
}
