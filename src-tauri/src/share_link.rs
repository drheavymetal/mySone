//! Listening Share Link — one-button browser-playable HTTP audio stream.
//!
//! Pedro clicks "Share" in the PlayerBar; SONE generates a UUID token and
//! returns a public URL `http(s)://music.drheavymetal.com:33333/r/{token}`.
//! Anyone (including Pedro on his phone) opens that URL to hear what he's
//! playing in their browser. NAT traversal is delegated to Pedro's network
//! setup (DNS + port-forward); SONE only needs to serve HTTP on `LOCAL_PORT`.
//!
//! Architecture:
//! - audio.rs taps the existing decode pipeline → tee → leaky queue → valve →
//!   audioconvert → resample → caps@48k/2ch → opusenc → oggmux → appsink.
//!   The appsink callback pushes Ogg/Opus pages into a `broadcast::Sender`.
//! - This module owns a long-lived axum HTTP server on port 33333. Each
//!   listener subscribes to the broadcast and the server streams chunks
//!   over chunked HTTP. Token validation rejects requests outside the
//!   active sharing window.
//! - The valve in audio.rs is closed by default — the encoder consumes no
//!   CPU until `start_sharing` is called.

use crate::audio::{AudioPlayer, ShareBroadcast};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::convert::Infallible as InfallibleErr;
use tauri::{Emitter, Manager};
use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use serde::Serialize;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio_stream::wrappers::BroadcastStream;

pub const LOCAL_PORT: u16 = 33333;
pub const PUBLIC_DOMAIN: &str = "music.drheavymetal.com";

#[derive(Clone, Debug, Serialize, Default)]
pub struct ShareStatus {
    pub active: bool,
    pub token: Option<String>,
    pub url: Option<String>,
    pub listener_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover_url: String,
    pub duration_secs: f32,
    pub position_secs: f32,
    /// TIDAL quality tier of the current stream — one of HI_RES_LOSSLESS,
    /// HI_RES, LOSSLESS, HIGH; empty string when unknown.
    #[serde(default)]
    pub quality: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub cover_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShareNowState {
    pub now: Option<NowPlaying>,
    pub queue: Vec<QueueEntry>,
    pub is_playing: bool,
}

struct Inner {
    active_token: Option<String>,
    listener_count: Arc<AtomicUsize>,
    now_state: ShareNowState,
}

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<Inner>>,
    broadcaster: ShareBroadcast,
    app_handle: tauri::AppHandle,
    /// Broadcast tick whenever `now_state` changes — SSE handlers wake up
    /// and resend the latest snapshot to their connected clients.
    state_tx: tokio::sync::broadcast::Sender<()>,
}

pub struct ShareLink {
    inner: Arc<Mutex<Inner>>,
    audio_player: AudioPlayer,
    state_tx: tokio::sync::broadcast::Sender<()>,
}

impl ShareLink {
    pub fn new(audio_player: AudioPlayer, app_handle: tauri::AppHandle) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            active_token: None,
            listener_count: Arc::new(AtomicUsize::new(0)),
            now_state: ShareNowState::default(),
        }));
        let broadcaster = audio_player.share_broadcaster();
        let (state_tx, _) = tokio::sync::broadcast::channel::<()>(16);
        let state = AppState {
            inner: Arc::clone(&inner),
            broadcaster,
            app_handle,
            state_tx: state_tx.clone(),
        };

        std::thread::Builder::new()
            .name("share-link-http".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("share-link tokio rt");
                rt.block_on(serve(state));
            })
            .expect("spawn share-link http thread");

        Self {
            inner,
            audio_player,
            state_tx,
        }
    }

    pub fn set_now_state(&self, state: ShareNowState) -> Result<(), String> {
        {
            let mut g = self
                .inner
                .lock()
                .map_err(|e| format!("share state poisoned: {e}"))?;
            g.now_state = state;
        }
        // Wake SSE handlers; ignore Err (no subscribers is fine).
        let _ = self.state_tx.send(());
        Ok(())
    }

    pub fn now_state(&self) -> ShareNowState {
        self.inner
            .lock()
            .map(|g| g.now_state.clone())
            .unwrap_or_default()
    }

    pub fn start_sharing(&self) -> Result<ShareStatus, String> {
        let token = uuid::Uuid::new_v4().simple().to_string();
        {
            let mut g = self
                .inner
                .lock()
                .map_err(|e| format!("share state poisoned: {e}"))?;
            g.active_token = Some(token.clone());
        }
        // Open the valve so opusenc starts producing data.
        self.audio_player.set_share_active(true)?;
        log::info!(
            "[share] sharing started → http://{}:{}/r/{}",
            PUBLIC_DOMAIN, LOCAL_PORT, token
        );
        Ok(self.status())
    }

    pub fn stop_sharing(&self) -> Result<ShareStatus, String> {
        {
            let mut g = self
                .inner
                .lock()
                .map_err(|e| format!("share state poisoned: {e}"))?;
            g.active_token = None;
        }
        self.audio_player.set_share_active(false)?;
        log::info!("[share] sharing stopped");
        Ok(self.status())
    }

    pub fn status(&self) -> ShareStatus {
        let g = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return ShareStatus::default(),
        };
        let token = g.active_token.clone();
        let listener_count = g.listener_count.load(Ordering::Relaxed);
        let url = token
            .as_ref()
            .map(|t| format!("http://{}:{}/r/{}", PUBLIC_DOMAIN, LOCAL_PORT, t));
        ShareStatus {
            active: token.is_some(),
            token,
            url,
            listener_count,
        }
    }
}

async fn serve(state: AppState) {
    let app = Router::new()
        .route("/r/:token", get(landing_page))
        .route("/r/:token/stream.mp3", get(audio_stream))
        .route("/r/:token/state", get(now_state))
        .route("/r/:token/events", get(events_sse))
        .route("/r/:token/cmd", post(cmd_handler))
        .route("/r/:token/search", get(search_handler))
        .route("/health", get(|| async { "ok" }))
        .with_state(state);

    let addr = format!("0.0.0.0:{LOCAL_PORT}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("[share] failed to bind {addr}: {e}");
            return;
        }
    };
    log::info!("[share] http server listening on {addr}");
    if let Err(e) = axum::serve(listener, app).await {
        log::error!("[share] http server exited: {e}");
    }
}

fn token_active(state: &AppState, token: &str) -> bool {
    state
        .inner
        .lock()
        .ok()
        .and_then(|g| g.active_token.clone())
        .map(|t| t == token)
        .unwrap_or(false)
}

async fn landing_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    if !token_active(&state, &token) {
        return (StatusCode::GONE, Html(NOT_FOUND_HTML.to_string())).into_response();
    }
    Html(landing_html(&token)).into_response()
}

async fn audio_stream(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    if !token_active(&state, &token) {
        return (StatusCode::GONE, "share link no longer active").into_response();
    }

    let listener_counter = state
        .inner
        .lock()
        .map(|g| Arc::clone(&g.listener_count))
        .unwrap_or_else(|_| Arc::new(AtomicUsize::new(0)));
    listener_counter.fetch_add(1, Ordering::Relaxed);

    let receiver = state.broadcaster.subscribe();
    let stream = OpusStream {
        inner: BroadcastStream::new(receiver),
        counter: listener_counter,
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/mpeg"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));

    (headers, Body::from_stream(stream)).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandPayload {
    action: String,
    #[serde(default)]
    track_id: Option<u64>,
}

async fn now_state(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    if !token_active(&state, &token) {
        return (StatusCode::GONE, "share link no longer active").into_response();
    }
    let snap = state
        .inner
        .lock()
        .map(|g| g.now_state.clone())
        .unwrap_or_default();
    Json(snap).into_response()
}

/// Server-Sent Events stream — pushes the current ShareNowState
/// immediately and again on every change. Replaces the 2 s polling
/// loop, dropping the perceived UI latency from 0-2 s to ~0.
async fn events_sse(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    if !token_active(&state, &token) {
        return (StatusCode::GONE, "share link no longer active").into_response();
    }

    let inner_for_stream = Arc::clone(&state.inner);
    let rx = state.state_tx.subscribe();

    let initial_snap = state
        .inner
        .lock()
        .map(|g| g.now_state.clone())
        .unwrap_or_default();

    use futures_util::stream::{self, StreamExt};

    // First event = current snapshot. Subsequent events = re-snapshot on
    // every state_tx tick. Lagged broadcasts silently skipped; closed
    // channel ends the stream.
    let initial = stream::once(async move {
        let json = serde_json::to_string(&initial_snap).unwrap_or_else(|_| "{}".into());
        Ok::<Event, InfallibleErr>(Event::default().data(json))
    });
    let updates = stream::unfold(
        (rx, inner_for_stream),
        |(mut rx, inner)| async move {
            loop {
                match rx.recv().await {
                    Ok(_) => {
                        let snap = inner
                            .lock()
                            .map(|g| g.now_state.clone())
                            .unwrap_or_default();
                        let json =
                            serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into());
                        return Some((
                            Ok::<Event, InfallibleErr>(Event::default().data(json)),
                            (rx, inner),
                        ));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );

    Sse::new(initial.chain(updates))
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Accepts {action: "play"|"pause"|"toggle"|"next"|"prev"|"playTrack",
/// trackId?: u64} and forwards as a Tauri event to the frontend, which
/// runs the actual playback action via its existing usePlaybackActions
/// hook. Keeping the queue + transport logic on the React side avoids
/// duplicating it in Rust.
async fn cmd_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(payload): Json<CommandPayload>,
) -> Response {
    if !token_active(&state, &token) {
        return (StatusCode::GONE, "share link no longer active").into_response();
    }
    let action = payload.action.as_str();
    if !matches!(
        action,
        "play" | "pause" | "toggle" | "next" | "prev" | "playTrack" | "addToQueue"
    ) {
        return (StatusCode::BAD_REQUEST, "unknown action").into_response();
    }
    let payload_out = serde_json::json!({
        "action": action,
        "trackId": payload.track_id,
    });
    if let Err(e) = state.app_handle.emit("share-cmd", payload_out) {
        log::warn!("[share] emit share-cmd failed: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "emit failed").into_response();
    }
    log::info!("[share] cmd {action} (track={:?})", payload.track_id);
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchTrackHit {
    track_id: u64,
    title: String,
    artist: String,
    cover_url: String,
}

#[derive(Serialize, Default)]
struct SearchResp {
    tracks: Vec<SearchTrackHit>,
}

/// Build the standard TIDAL image URL the frontend uses elsewhere.
/// `cover` is the dashed UUID returned by TIDAL; size is one of the
/// supported sizes (160, 320, 640, 1280, etc.).
fn tidal_cover_url(cover: &str, size: u32) -> String {
    format!(
        "https://resources.tidal.com/images/{}/{size}x{size}.jpg",
        cover.replace('-', "/"),
        size = size,
    )
}

async fn search_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(params): Query<SearchQuery>,
) -> Response {
    if !token_active(&state, &token) {
        return (StatusCode::GONE, "share link no longer active").into_response();
    }
    let q = params.q.trim();
    if q.is_empty() {
        return Json(SearchResp::default()).into_response();
    }
    let limit = params.limit.unwrap_or(15).clamp(1, 30);

    let app_state = state.app_handle.state::<crate::AppState>();
    let mut client = app_state.tidal_client.lock().await;
    match client.search(q, limit).await {
        Ok(results) => {
            let tracks: Vec<SearchTrackHit> = results
                .tracks
                .into_iter()
                .take(limit as usize)
                .map(|t| {
                    let artist = t
                        .artists
                        .as_ref()
                        .filter(|a| !a.is_empty())
                        .map(|a| {
                            a.iter()
                                .map(|x| x.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .or_else(|| t.artist.as_ref().map(|a| a.name.clone()))
                        .unwrap_or_default();
                    let cover_url = t
                        .album
                        .as_ref()
                        .and_then(|a| a.cover.as_ref())
                        .map(|c| tidal_cover_url(c, 160))
                        .unwrap_or_default();
                    SearchTrackHit {
                        track_id: t.id,
                        title: t.title,
                        artist,
                        cover_url,
                    }
                })
                .collect();
            Json(SearchResp { tracks }).into_response()
        }
        Err(e) => {
            log::warn!("[share] search failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response()
        }
    }
}

/// Streams MP3 frames from the broadcast channel until the listener
/// disconnects. Drops the listener counter on Drop so the count is accurate
/// even on abnormal disconnect. Lagged frames (slow client) are silently
/// skipped — better a glitch than a broken connection.
struct OpusStream {
    inner: BroadcastStream<Bytes>,
    counter: Arc<AtomicUsize>,
}

impl Drop for OpusStream {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Stream for OpusStream {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(bytes))) => return Poll::Ready(Some(Ok(bytes))),
                Poll::Ready(Some(Err(_))) => continue,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

fn landing_html(token: &str) -> String {
    format!(
        r##"<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="theme-color" content="#0c0c10">
<title>SONE — En directo</title>
<style>
  :root {{
    color-scheme: dark;
    --bg: #0c0c10;
    --fg: #e6e6ea;
    --muted: #8a8a92;
    --accent: #c8b5ff;
    --card: #16161c;
  }}
  * {{ box-sizing: border-box; }}
  html, body {{ margin: 0; height: 100%; background: var(--bg); color: var(--fg);
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
                -webkit-tap-highlight-color: transparent; }}
  body {{ min-height: 100vh; padding-bottom: 40px; }}

  .wrap {{ max-width: 520px; margin: 0 auto; padding: 22px 18px 60px; }}

  .pill {{ display: inline-flex; align-items: center; gap: 6px;
          padding: 4px 12px; border-radius: 999px;
          background: rgba(200,181,255,0.12); color: var(--accent);
          font-size: 11px; letter-spacing: 0.16em; text-transform: uppercase; }}
  .pill .dot {{ width: 6px; height: 6px; border-radius: 999px; background: var(--accent);
               animation: dot-pulse 1.6s ease-in-out infinite; }}
  @keyframes dot-pulse {{
    0%, 100% {{ opacity: 1; transform: scale(1); }}
    50%      {{ opacity: 0.45; transform: scale(0.8); }}
  }}

  .header {{ display: flex; justify-content: space-between; align-items: center; gap: 12px; }}
  .room   {{ font-size: 11px; color: var(--muted); font-family: ui-monospace, Menlo, monospace;
            letter-spacing: 0.04em; }}

  .stage {{ margin-top: 22px; display: flex; flex-direction: column; align-items: center;
           text-align: center; gap: 10px; }}
  .cover {{
    position: relative; width: min(72vw, 320px); aspect-ratio: 1 / 1;
    border-radius: 16px; overflow: hidden;
    background: linear-gradient(135deg, #1c1c24, #11111a);
    box-shadow: 0 30px 60px -10px rgba(0,0,0,0.6);
    transition: transform 0.4s ease;
  }}
  .cover.is-playing {{ animation: cover-breathe 6.5s ease-in-out infinite; }}
  @keyframes cover-breathe {{
    0%, 100% {{ transform: scale(1); }}
    50%      {{ transform: scale(1.025); }}
  }}
  .cover img {{ width: 100%; height: 100%; object-fit: cover; display: block;
               opacity: 0; transition: opacity 0.4s ease; }}
  .cover.has-art img {{ opacity: 1; }}
  .cover .placeholder {{ position: absolute; inset: 0; display: flex; align-items: center;
                        justify-content: center; color: rgba(255,255,255,0.18); font-size: 40px; }}

  .meta-title  {{ font-size: 19px; font-weight: 600; line-height: 1.25; margin: 8px 0 0; }}
  .meta-artist {{ font-size: 14px; color: var(--muted); margin: 2px 0 0; }}
  .meta-album  {{ font-size: 12px; color: rgba(138,138,146,0.7); margin: 0; font-style: italic; }}
  .quality-badge {{
    display: none; margin-top: 6px;
    padding: 2px 8px; border-radius: 999px;
    background: rgba(200,181,255,0.12); color: var(--accent);
    font-size: 9.5px; letter-spacing: 0.18em; text-transform: uppercase;
    font-weight: 500;
  }}
  .quality-badge.show {{ display: inline-block; }}

  .transport {{ display: flex; align-items: center; justify-content: center; gap: 22px;
               margin-top: 18px; }}
  .tbtn {{
    appearance: none; -webkit-appearance: none; border: 0; background: transparent;
    color: var(--fg); cursor: pointer; padding: 10px;
    border-radius: 50%; transition: background 0.15s ease, transform 0.1s ease;
  }}
  .tbtn:active {{ transform: scale(0.92); }}
  .tbtn:hover  {{ background: rgba(255,255,255,0.06); }}
  .tbtn svg {{ display: block; }}
  .play-pill {{
    width: 64px; height: 64px; display: flex; align-items: center; justify-content: center;
    background: radial-gradient(circle, rgba(200,181,255,0.5), rgba(200,181,255,0.15) 70%);
    border-radius: 50%;
    box-shadow: 0 10px 30px rgba(200,181,255,0.18);
  }}
  .play-pill:hover {{ background: radial-gradient(circle, rgba(200,181,255,0.7), rgba(200,181,255,0.2) 70%); }}

  .listen-bar {{
    margin: 22px auto 0; max-width: 360px;
    background: var(--card); border-radius: 14px; padding: 14px 16px;
    display: flex; align-items: center; justify-content: space-between; gap: 14px;
  }}
  .listen-bar .label {{ font-size: 12px; color: var(--muted); line-height: 1.4; }}
  .listen-bar .label b {{ color: var(--fg); font-weight: 500; }}
  .listen-btn {{
    appearance: none; -webkit-appearance: none; border: 0;
    padding: 8px 14px; border-radius: 999px; cursor: pointer;
    background: var(--accent); color: #1d162e; font-weight: 600; font-size: 12px;
    letter-spacing: 0.08em; text-transform: uppercase;
    transition: filter 0.15s ease;
  }}
  .listen-btn.muted {{ background: rgba(255,255,255,0.1); color: var(--fg); }}
  .listen-btn:active {{ filter: brightness(0.9); }}

  .add-section {{ margin-top: 28px; }}
  .add-section h2 {{ font-size: 11px; letter-spacing: 0.22em; text-transform: uppercase;
                    color: var(--muted); margin: 0 0 10px; }}
  .search-row {{ display: flex; gap: 8px; }}
  .search-input {{
    flex: 1; min-width: 0;
    background: var(--card); border: 1px solid rgba(255,255,255,0.06);
    border-radius: 10px; padding: 10px 12px; font-size: 14px;
    color: var(--fg); outline: none;
    transition: border-color 0.15s ease;
  }}
  .search-input:focus {{ border-color: rgba(200,181,255,0.4); }}
  .search-results {{ margin-top: 8px; background: var(--card); border-radius: 10px;
                    overflow: hidden; }}
  .search-results .hit {{ display: flex; gap: 10px; align-items: center;
                         padding: 8px 10px; cursor: pointer;
                         border-top: 1px solid rgba(255,255,255,0.05);
                         transition: background 0.12s ease; }}
  .search-results .hit:first-child {{ border-top: 0; }}
  .search-results .hit:hover {{ background: rgba(200,181,255,0.06); }}
  .search-results .hit img {{ width: 36px; height: 36px; border-radius: 6px; object-fit: cover;
                             background: rgba(255,255,255,0.05); flex-shrink: 0; }}
  .search-results .hit .meta {{ min-width: 0; flex: 1; }}
  .search-results .hit .t {{ font-size: 13px; line-height: 1.25; white-space: nowrap;
                            overflow: hidden; text-overflow: ellipsis; }}
  .search-results .hit .a {{ font-size: 11px; color: var(--muted); white-space: nowrap;
                            overflow: hidden; text-overflow: ellipsis; }}
  .search-results .added {{ font-size: 11px; color: var(--accent); padding: 0 8px; }}
  .search-status {{ font-size: 11px; color: var(--muted); padding: 8px 10px; }}

  .queue {{ margin-top: 28px; }}
  .queue h2 {{ font-size: 11px; letter-spacing: 0.22em; text-transform: uppercase;
              color: var(--muted); margin: 0 0 10px; }}
  .queue ul {{ list-style: none; padding: 0; margin: 0;
              border-radius: 12px; overflow: hidden; background: var(--card); }}
  .queue li {{ display: flex; gap: 10px; align-items: center;
              padding: 8px 10px; border-top: 1px solid rgba(255,255,255,0.05);
              cursor: pointer; transition: background 0.12s ease; }}
  .queue li:first-child {{ border-top: 0; }}
  .queue li:hover {{ background: rgba(255,255,255,0.04); }}
  .queue li img {{ width: 38px; height: 38px; border-radius: 6px; object-fit: cover;
                  background: rgba(255,255,255,0.05); flex-shrink: 0; }}
  .queue li .qmeta {{ min-width: 0; flex: 1; }}
  .queue li .qt {{ font-size: 13px; line-height: 1.25; white-space: nowrap; overflow: hidden;
                  text-overflow: ellipsis; }}
  .queue li .qa {{ font-size: 11px; color: var(--muted); white-space: nowrap; overflow: hidden;
                  text-overflow: ellipsis; }}
  .queue .empty {{ padding: 14px; text-align: center; color: var(--muted); font-size: 12px;
                  background: var(--card); border-radius: 12px; }}

  .err  {{ color: #ff6e6e; font-size: 12px; text-align: center; margin-top: 10px; }}
  .footer {{ margin-top: 30px; text-align: center;
            font-size: 10px; letter-spacing: 0.22em; text-transform: uppercase;
            color: rgba(255,255,255,0.18); }}

  audio {{ display: none; }}
</style>
</head>
<body>
<div class="wrap">
  <div class="header">
    <span class="pill"><span class="dot"></span> En directo</span>
    <span class="room">{token_short}</span>
  </div>

  <div class="stage">
    <div id="cover" class="cover">
      <span class="placeholder">♪</span>
      <img id="cover-img" alt="" />
    </div>
    <p id="title"  class="meta-title">Cargando…</p>
    <p id="artist" class="meta-artist"></p>
    <p id="album"  class="meta-album"></p>
    <span id="quality" class="quality-badge"></span>

    <div class="transport">
      <button class="tbtn" data-act="prev" aria-label="Anterior">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
          <path d="M6 6h2v12H6zM10 12L20 6v12z"/>
        </svg>
      </button>
      <button class="tbtn play-pill" data-act="toggle" aria-label="Reproducir/Pausa">
        <svg id="play-icon" width="28" height="28" viewBox="0 0 24 24" fill="currentColor">
          <path d="M8 5v14l11-7z"/>
        </svg>
      </button>
      <button class="tbtn" data-act="next" aria-label="Siguiente">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor">
          <path d="M16 6h2v12h-2zM4 18l10-6L4 6z"/>
        </svg>
      </button>
    </div>
  </div>

  <div class="listen-bar">
    <div class="label">
      <b>Escuchar</b><br>activa el audio en este dispositivo
    </div>
    <button id="listen-btn" class="listen-btn">PLAY</button>
  </div>

  <section class="add-section">
    <h2>Añadir a la cola</h2>
    <div class="search-row">
      <input id="search-input" class="search-input" type="search"
             placeholder="Busca artista, pista o álbum…" autocomplete="off"
             enterkeyhint="search" inputmode="search" />
    </div>
    <div id="search-results" class="search-results" style="display:none"></div>
  </section>

  <section class="queue">
    <h2>A continuación</h2>
    <ul id="queue-list"></ul>
    <div id="queue-empty" class="empty" style="display:none">Sin pistas en cola</div>
  </section>

  <div id="err" class="err" style="display:none"></div>
  <div class="footer">stream · sone</div>

  <audio id="audio" preload="none" playsinline></audio>
</div>

<script>
(function() {{
  var TOKEN = "{token}";
  var STREAM = '/r/' + TOKEN + '/stream.mp3';
  var STATE_URL = '/r/' + TOKEN + '/state';
  var CMD_URL = '/r/' + TOKEN + '/cmd';

  var audio   = document.getElementById('audio');
  var listenBtn = document.getElementById('listen-btn');
  var coverEl = document.getElementById('cover');
  var coverImg = document.getElementById('cover-img');
  var titleEl = document.getElementById('title');
  var artistEl = document.getElementById('artist');
  var albumEl = document.getElementById('album');
  var qualityEl = document.getElementById('quality');
  var playIconEl = document.getElementById('play-icon');
  var queueListEl = document.getElementById('queue-list');
  var queueEmpty = document.getElementById('queue-empty');
  var errEl = document.getElementById('err');

  function prettyQuality(q) {{
    switch (q) {{
      case 'HI_RES_LOSSLESS': return 'Hi-Res Lossless';
      case 'HI_RES':          return 'MQA Hi-Res';
      case 'LOSSLESS':        return 'Lossless';
      case 'HIGH':            return 'High (AAC)';
      default:                return q || '';
    }}
  }}

  var listening = false;
  var lastCover = null;
  var hostPlaying = false;

  function setListening(on) {{
    listening = on;
    listenBtn.textContent = on ? 'MUTE' : 'PLAY';
    listenBtn.classList.toggle('muted', on);
  }}

  function startListen() {{
    audio.src = STREAM + '?t=' + Date.now();
    var p = audio.play();
    if (p && p.then) {{
      p.then(function() {{ setListening(true); errEl.style.display = 'none'; }})
       .catch(function(e) {{
         errEl.style.display = 'block';
         errEl.textContent = 'No se pudo reproducir: ' + (e && e.message ? e.message : e);
       }});
    }}
  }}
  function stopListen() {{
    audio.pause();
    audio.removeAttribute('src');
    audio.load();
    setListening(false);
  }}

  listenBtn.addEventListener('click', function() {{
    if (listening) stopListen(); else startListen();
  }});
  audio.addEventListener('playing', function() {{ setListening(true); }});
  audio.addEventListener('pause',   function() {{ setListening(false); }});
  audio.addEventListener('error',   function() {{
    errEl.style.display = 'block';
    errEl.textContent = 'Stream interrumpido. Toca PLAY para reintentar.';
    setListening(false);
  }});

  // Transport buttons → POST /cmd
  document.querySelectorAll('.transport .tbtn').forEach(function(b) {{
    b.addEventListener('click', function() {{
      sendCmd(b.dataset.act);
    }});
  }});
  function sendCmd(action, trackId) {{
    var body = {{action: action}};
    if (trackId != null) body.trackId = trackId;
    fetch(CMD_URL, {{
      method: 'POST',
      headers: {{'Content-Type': 'application/json'}},
      body: JSON.stringify(body),
    }}).catch(function() {{}});
  }}

  // ── State polling ──
  var lastQueueSig = '';
  function renderState(state) {{
    if (!state || !state.now) {{
      titleEl.textContent = 'Nada sonando';
      artistEl.textContent = '';
      albumEl.textContent = '';
      qualityEl.classList.remove('show');
      qualityEl.textContent = '';
      coverEl.classList.remove('has-art', 'is-playing');
      coverImg.removeAttribute('src');
      lastCover = null;
      hostPlaying = false;
      setPlayIcon(false);
    }} else {{
      var n = state.now;
      titleEl.textContent  = n.title || '—';
      artistEl.textContent = n.artist || '';
      albumEl.textContent  = n.album || '';
      var q = prettyQuality(n.quality);
      if (q) {{
        qualityEl.textContent = q;
        qualityEl.classList.add('show');
      }} else {{
        qualityEl.classList.remove('show');
      }}
      if (n.coverUrl && n.coverUrl !== lastCover) {{
        coverImg.src = n.coverUrl;
        lastCover = n.coverUrl;
        coverEl.classList.add('has-art');
      }} else if (!n.coverUrl) {{
        coverEl.classList.remove('has-art');
        coverImg.removeAttribute('src');
        lastCover = null;
      }}
      hostPlaying = !!state.isPlaying;
      coverEl.classList.toggle('is-playing', hostPlaying);
      setPlayIcon(hostPlaying);
    }}

    var q = (state && state.queue) || [];
    var sig = q.map(function(t) {{ return t.trackId; }}).join('|');
    if (sig !== lastQueueSig) {{
      lastQueueSig = sig;
      queueListEl.innerHTML = '';
      if (q.length === 0) {{
        queueEmpty.style.display = 'block';
      }} else {{
        queueEmpty.style.display = 'none';
        q.forEach(function(t) {{
          var li = document.createElement('li');
          li.dataset.trackId = t.trackId;
          var img = document.createElement('img');
          if (t.coverUrl) img.src = t.coverUrl;
          var meta = document.createElement('div');
          meta.className = 'qmeta';
          var qt = document.createElement('div');
          qt.className = 'qt';
          qt.textContent = t.title || '—';
          var qa = document.createElement('div');
          qa.className = 'qa';
          qa.textContent = t.artist || '';
          meta.appendChild(qt);
          meta.appendChild(qa);
          li.appendChild(img);
          li.appendChild(meta);
          li.addEventListener('click', function() {{
            sendCmd('playTrack', t.trackId);
          }});
          queueListEl.appendChild(li);
        }});
      }}
    }}
  }}

  function setPlayIcon(playing) {{
    // playing → show pause icon; paused → play icon
    playIconEl.innerHTML = playing
      ? '<path d="M6 5h4v14H6zM14 5h4v14h-4z"/>'
      : '<path d="M8 5v14l11-7z"/>';
  }}

  // ── State stream (SSE) ──
  // Falls back to polling if EventSource isn't available or the SSE
  // connection drops repeatedly.
  var pollInterval = null;
  function startPolling() {{
    if (pollInterval) return;
    function poll() {{
      fetch(STATE_URL, {{cache: 'no-store'}})
        .then(function(r) {{ return r.ok ? r.json() : null; }})
        .then(function(s) {{ if (s) renderState(s); }})
        .catch(function() {{}});
    }}
    poll();
    pollInterval = setInterval(poll, 2000);
  }}
  function stopPolling() {{
    if (pollInterval) {{ clearInterval(pollInterval); pollInterval = null; }}
  }}

  if (typeof EventSource === 'function') {{
    var es;
    var sseFails = 0;
    function openSse() {{
      es = new EventSource('/r/' + TOKEN + '/events');
      es.onmessage = function(ev) {{
        try {{ renderState(JSON.parse(ev.data)); sseFails = 0; }}
        catch (e) {{}}
      }};
      es.onerror = function() {{
        // EventSource auto-reconnects; if it keeps failing, fall back to poll.
        sseFails++;
        if (sseFails > 4) {{ es.close(); startPolling(); }}
      }};
    }}
    openSse();
  }} else {{
    startPolling();
  }}

  // ── Search + add-to-queue ──
  var searchInput = document.getElementById('search-input');
  var searchResults = document.getElementById('search-results');
  var searchTimer = null;
  var searchSeq = 0;

  function runSearch(q) {{
    var seq = ++searchSeq;
    if (!q) {{
      searchResults.style.display = 'none';
      searchResults.innerHTML = '';
      return;
    }}
    searchResults.style.display = 'block';
    searchResults.innerHTML = '<div class="search-status">Buscando…</div>';
    fetch('/r/' + TOKEN + '/search?q=' + encodeURIComponent(q) + '&limit=12',
          {{cache: 'no-store'}})
      .then(function(r) {{ return r.ok ? r.json() : null; }})
      .then(function(data) {{
        if (seq !== searchSeq) return;
        if (!data || !data.tracks || data.tracks.length === 0) {{
          searchResults.innerHTML = '<div class="search-status">Sin resultados</div>';
          return;
        }}
        searchResults.innerHTML = '';
        data.tracks.forEach(function(t) {{
          var row = document.createElement('div');
          row.className = 'hit';
          var img = document.createElement('img');
          if (t.coverUrl) img.src = t.coverUrl;
          var meta = document.createElement('div');
          meta.className = 'meta';
          var tt = document.createElement('div');
          tt.className = 't';
          tt.textContent = t.title || '—';
          var ta = document.createElement('div');
          ta.className = 'a';
          ta.textContent = t.artist || '';
          meta.appendChild(tt); meta.appendChild(ta);
          var added = document.createElement('span');
          added.className = 'added';
          added.style.display = 'none';
          added.textContent = 'añadido';
          row.appendChild(img); row.appendChild(meta); row.appendChild(added);
          row.addEventListener('click', function() {{
            sendCmd('addToQueue', t.trackId);
            added.style.display = 'inline';
            row.style.pointerEvents = 'none';
            row.style.opacity = '0.55';
          }});
          searchResults.appendChild(row);
        }});
      }})
      .catch(function() {{
        if (seq !== searchSeq) return;
        searchResults.innerHTML = '<div class="search-status">Error de búsqueda</div>';
      }});
  }}

  searchInput.addEventListener('input', function() {{
    if (searchTimer) clearTimeout(searchTimer);
    var q = searchInput.value.trim();
    searchTimer = setTimeout(function() {{ runSearch(q); }}, 280);
  }});
  searchInput.addEventListener('focus', function() {{
    if (searchInput.value.trim()) runSearch(searchInput.value.trim());
  }});

  // Best-effort silent autoplay on load (mobile usually blocks; user taps PLAY).
  window.addEventListener('load', function() {{
    audio.src = STREAM + '?t=' + Date.now();
    var p = audio.play();
    if (p && p.catch) p.catch(function() {{ /* expected */ }});
  }});
}})();
</script>
</body>
</html>"##,
        token = token,
        token_short = &token[..token.len().min(12)],
    )
}

const NOT_FOUND_HTML: &str = r#"<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<title>Link not active</title>
<style>
  body {{ background: #0c0c10; color: #8a8a92; font-family: system-ui, sans-serif;
         display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }}
  div {{ text-align: center; }}
  h1 {{ font-size: 22px; color: #e6e6ea; margin: 0 0 8px; }}
  p  {{ font-size: 13px; }}
</style>
</head>
<body>
<div>
  <h1>Link no activo</h1>
  <p>Esta sala ya no está emitiendo.</p>
</div>
</body>
</html>"#;
