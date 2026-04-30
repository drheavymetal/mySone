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
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
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

struct Inner {
    active_token: Option<String>,
    listener_count: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<Inner>>,
    broadcaster: ShareBroadcast,
}

pub struct ShareLink {
    inner: Arc<Mutex<Inner>>,
    audio_player: AudioPlayer,
}

impl ShareLink {
    pub fn new(audio_player: AudioPlayer) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            active_token: None,
            listener_count: Arc::new(AtomicUsize::new(0)),
        }));
        let broadcaster = audio_player.share_broadcaster();
        let state = AppState {
            inner: Arc::clone(&inner),
            broadcaster,
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
        }
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
        .route("/r/:token/stream.opus", get(audio_stream))
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
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/ogg"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));

    (headers, Body::from_stream(stream)).into_response()
}

/// Streams Opus/Ogg pages from the broadcast channel until the listener
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
        r#"<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>SONE Listening Room</title>
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
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif; }}
  .wrap {{ display: flex; flex-direction: column; align-items: center; justify-content: center;
          min-height: 100%; padding: 24px; gap: 28px; }}
  .card {{ background: var(--card); border-radius: 18px; padding: 28px 24px; max-width: 420px; width: 100%;
          box-shadow: 0 20px 60px rgba(0,0,0,0.5); }}
  .title {{ font-size: 13px; letter-spacing: 0.18em; text-transform: uppercase; color: var(--muted); margin: 0 0 6px; }}
  .room {{ font-size: 22px; font-weight: 600; margin: 0 0 18px; }}
  audio {{ width: 100%; margin-top: 4px; }}
  .hint {{ font-size: 12px; color: var(--muted); margin-top: 14px; line-height: 1.55; }}
  .pill {{ display: inline-block; padding: 2px 10px; border-radius: 999px; background: rgba(200,181,255,0.15);
          color: var(--accent); font-size: 11px; letter-spacing: 0.08em; text-transform: uppercase; margin-bottom: 14px; }}
  .footer {{ font-size: 11px; color: var(--muted); }}
</style>
</head>
<body>
<div class="wrap">
  <div class="card">
    <div class="pill">Listening room · live</div>
    <p class="title">SONE</p>
    <p class="room">{token}</p>
    <audio controls autoplay preload="none" src="/r/{token}/stream.opus"></audio>
    <p class="hint">El audio empieza en cuanto pulses ▶ (algunos navegadores bloquean autoplay).
    Latencia ≈ 1–3&nbsp;s. Calidad: Opus 256&nbsp;kbps.</p>
  </div>
  <div class="footer">streaming desde mySone — solo audio</div>
</div>
</body>
</html>"#
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
