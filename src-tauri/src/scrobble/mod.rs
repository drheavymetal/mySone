pub mod lastfm;
pub mod librefm;
pub mod listenbrainz;
pub mod musicbrainz;
pub mod queue;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::{Mutex, RwLock};

use crate::crypto::Crypto;
use crate::stats::{PlayRecord, StatsDb};
use crate::SoneError;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScrobbleTrack {
    pub artist: String,
    pub track: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub album_artist: Option<String>,
    pub duration_secs: u32,
    #[serde(default)]
    pub track_number: Option<u32>,
    pub timestamp: i64,
    pub chosen_by_user: bool,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub track_id: Option<u64>,
    #[serde(default)]
    pub recording_mbid: Option<String>,
    #[serde(default)]
    pub release_group_mbid: Option<String>,
    #[serde(default)]
    pub artist_mbid: Option<String>,
    /// MusicBrainz Work MBID (parent work for classical recordings).
    /// Resolved post-track-start by the catalog service when a recording
    /// MBID becomes known. NULL for non-classical plays.
    #[serde(default)]
    pub work_mbid: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ProviderStatus {
    pub name: String,
    pub connected: bool,
    pub username: Option<String>,
}

/// Outcome of a ListenBrainz history import run.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: u64,
    pub skipped: u64,
    pub pages: u32,
    pub username: String,
}

pub enum ScrobbleResult {
    Ok,
    AuthError(String),
    Retryable(String),
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait ScrobbleProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_authenticated(&self) -> bool;
    fn max_batch_size(&self) -> usize;
    fn set_http_client(&self, client: reqwest::Client);
    async fn username(&self) -> Option<String>;
    async fn now_playing(&self, track: &ScrobbleTrack) -> ScrobbleResult;
    async fn scrobble(&self, tracks: &[ScrobbleTrack]) -> ScrobbleResult;
}

// ---------------------------------------------------------------------------
// Track playback state (private)
// ---------------------------------------------------------------------------

struct TrackPlayback {
    track: ScrobbleTrack,
    accumulated_secs: f64,
    last_resumed_at: Option<Instant>,
    scrobbled: bool,
}

impl TrackPlayback {
    fn new(track: ScrobbleTrack) -> Self {
        Self {
            track,
            accumulated_secs: 0.0,
            last_resumed_at: Some(Instant::now()),
            scrobbled: false,
        }
    }

    /// Total seconds of actual playback so far.
    fn elapsed(&self) -> f64 {
        let live = self
            .last_resumed_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        self.accumulated_secs + live
    }

    fn pause(&mut self) {
        if let Some(resumed) = self.last_resumed_at.take() {
            self.accumulated_secs += resumed.elapsed().as_secs_f64();
        }
    }

    fn resume(&mut self) {
        if self.last_resumed_at.is_none() {
            self.last_resumed_at = Some(Instant::now());
        }
    }

    /// After a seek, reset the live timer but keep accumulated time.
    /// If paused (last_resumed_at is None), do nothing — stay paused.
    fn on_seek(&mut self) {
        if let Some(resumed) = self.last_resumed_at.take() {
            self.accumulated_secs += resumed.elapsed().as_secs_f64();
            self.last_resumed_at = Some(Instant::now());
        }
    }

    /// Meets the scrobble threshold:
    /// - track is longer than 30 seconds
    /// - listened to at least 50% of the track OR at least 4 minutes
    fn meets_threshold(&self) -> bool {
        if self.track.duration_secs <= 30 {
            return false;
        }
        let listened = self.elapsed();
        let half = self.track.duration_secs as f64 / 2.0;
        listened >= half || listened >= 240.0
    }
}

// ---------------------------------------------------------------------------
// ScrobbleManager
// ---------------------------------------------------------------------------

/// Resolve a parent Work MBID from a Recording MBID. Implemented by the
/// classical `CatalogService`. Kept as a trait here so the scrobble
/// manager doesn't depend on the classical module directly — keeps the
/// dependency arrow pointing one way (lib → scrobble; lib → classical;
/// scrobble does not import classical).
#[async_trait]
pub trait WorkMbidResolver: Send + Sync {
    async fn resolve_work_for_recording(
        &self,
        recording_mbid: &str,
    ) -> Option<String>;
}

pub struct ScrobbleManager {
    providers: RwLock<Vec<Box<dyn ScrobbleProvider>>>,
    queue: queue::ScrobbleQueue,
    current_track: Arc<Mutex<Option<TrackPlayback>>>,
    app_handle: tauri::AppHandle,
    mb_lookup: Arc<musicbrainz::MusicBrainzLookup>,
    stats: Arc<StatsDb>,
    /// Optional resolver for parent-Work MBID. Set after construction
    /// (the classical catalog is built later in `AppState::new`). Kept
    /// behind a `RwLock` so mutating it doesn't require `&mut self`.
    work_resolver: RwLock<Option<Arc<dyn WorkMbidResolver>>>,
}

impl ScrobbleManager {
    pub fn new(
        app_handle: tauri::AppHandle,
        crypto: Arc<Crypto>,
        config_dir: &Path,
        http_client: reqwest::Client,
        stats: Arc<StatsDb>,
    ) -> Self {
        let queue_path = config_dir.join("scrobble_queue.bin");
        Self {
            providers: RwLock::new(Vec::new()),
            queue: queue::ScrobbleQueue::new(&queue_path, crypto),
            current_track: Arc::new(Mutex::new(None)),
            app_handle,
            mb_lookup: Arc::new(musicbrainz::MusicBrainzLookup::new(config_dir, http_client)),
            stats,
            work_resolver: RwLock::new(None),
        }
    }

    /// Install the work-mbid resolver. Best-effort, may be called once
    /// or never; if never set, on_track_started simply skips the
    /// classical work resolution.
    pub async fn set_work_resolver(&self, resolver: Arc<dyn WorkMbidResolver>) {
        let mut w = self.work_resolver.write().await;
        *w = Some(resolver);
    }

    /// Snapshot of the parent-Work MBID for the currently playing
    /// track, when the background MBID resolver has caught up.
    /// Returns `None` while the lookup is still in flight or when the
    /// track is not classical.
    pub async fn current_work_mbid(&self) -> Option<String> {
        let current = self.current_track.lock().await;
        current
            .as_ref()
            .and_then(|p| p.track.work_mbid.clone())
    }

    /// Snapshot of the recording MBID once it's been resolved. Useful
    /// for the player to show "View work" UI as soon as MB confirms
    /// the recording, even before the work-rel call lands.
    pub async fn current_recording_mbid(&self) -> Option<String> {
        let current = self.current_track.lock().await;
        current
            .as_ref()
            .and_then(|p| p.track.recording_mbid.clone())
    }

    /// Persist a finished playback to the local stats DB. Called whenever
    /// a track transitions out of `current_track` (replaced, stopped, or
    /// app shutdown). Quiet on errors — stats are best-effort.
    fn record_to_stats(&self, p: &TrackPlayback) {
        let listened = p.elapsed();
        if listened < 1.0 {
            return;
        }
        let now = crate::now_secs() as i64;
        let record = PlayRecord {
            started_at: p.track.timestamp,
            finished_at: now,
            track_id: p.track.track_id,
            title: &p.track.track,
            artist: &p.track.artist,
            album: p.track.album.as_deref(),
            album_artist: p.track.album_artist.as_deref(),
            duration_secs: p.track.duration_secs,
            listened_secs: listened as u32,
            completed: p.meets_threshold(),
            isrc: p.track.isrc.as_deref(),
            chosen_by_user: p.track.chosen_by_user,
            source: "local",
            recording_mbid: p.track.recording_mbid.as_deref(),
            release_group_mbid: p.track.release_group_mbid.as_deref(),
            artist_mbid: p.track.artist_mbid.as_deref(),
            work_mbid: p.track.work_mbid.as_deref(),
        };
        if let Err(e) = self.stats.record_play(&record) {
            log::warn!("[stats] record_play failed: {e}");
        }
    }

    /// Update the HTTP client used by all active scrobble providers and the
    /// MusicBrainz lookup. Called when proxy settings change.
    pub async fn update_http_client(&self, client: reqwest::Client) {
        let providers = self.providers.read().await;
        for provider in providers.iter() {
            provider.set_http_client(client.clone());
        }
        drop(providers);
        self.mb_lookup.set_http_client(client);
    }

    pub async fn add_provider(&self, provider: Box<dyn ScrobbleProvider>) {
        let mut providers = self.providers.write().await;
        // Remove existing provider with the same name
        let name = provider.name().to_string();
        providers.retain(|p| p.name() != name);
        providers.push(provider);
    }

    pub async fn remove_provider(&self, name: &str) {
        let mut providers = self.providers.write().await;
        providers.retain(|p| p.name() != name);
    }

    pub async fn provider_statuses(&self) -> Vec<ProviderStatus> {
        let providers = self.providers.read().await;
        let mut statuses = Vec::new();

        let known = ["lastfm", "listenbrainz", "librefm"];
        for &name in &known {
            if let Some(p) = providers.iter().find(|p| p.name() == name) {
                statuses.push(ProviderStatus {
                    name: name.to_string(),
                    connected: p.is_authenticated(),
                    username: p.username().await,
                });
            } else {
                statuses.push(ProviderStatus {
                    name: name.to_string(),
                    connected: false,
                    username: None,
                });
            }
        }
        statuses
    }

    /// Called when a new track begins playing.
    pub async fn on_track_started(&self, track: ScrobbleTrack) {
        // 1. Single lock: extract previous, set new immediately
        let prev_playback = {
            let mut current = self.current_track.lock().await;
            let prev = current.take();
            *current = Some(TrackPlayback::new(track.clone()));
            prev
        };
        // Lock released — new track is live with correct Instant::now()

        // Persist the finished play locally (fires for every track, even skips).
        let prev_track = prev_playback.and_then(|p| {
            self.record_to_stats(&p);
            if !p.scrobbled && p.meets_threshold() {
                Some(p.track)
            } else {
                None
            }
        });

        // 2. Network I/O runs concurrently, AFTER track is set
        tokio::join!(
            async {
                if let Some(prev) = prev_track {
                    self.dispatch_scrobble(prev).await;
                }
            },
            self.fire_now_playing(&track),
        );

        // 3. Spawn fire-and-forget MBID lookup. Two sources, merged:
        //    - ISRC (when track has one): authoritative for recording_mbid.
        //    - Name search: gives us release_group_mbid + artist_mbid the
        //      ISRC endpoint doesn't return, plus a recording_mbid
        //      fallback when ISRC is missing or unmatched.
        //    We guard the write by track_id (if present) or by the
        //    `(title, artist)` pair so a stale lookup doesn't bleed into
        //    a freshly-started track.
        let track_name = track.track.clone();
        let artist_name = track.artist.clone();
        let isrc = track.isrc.clone();
        let expected_id = track.track_id;
        let mb = Arc::clone(&self.mb_lookup);
        let ct = Arc::clone(&self.current_track);
        // Snapshot the resolver under the read lock so the spawned task
        // doesn't have to borrow `self`.
        let work_resolver = self.work_resolver.read().await.clone();
        // Cloned for the event emission inside the spawned task. The
        // ScrobbleManager owns its own app_handle; cloning is cheap.
        let event_handle = self.app_handle.clone();
        tokio::spawn(async move {
            // Run ISRC and name lookups in parallel — they share a
            // rate-limit mutex internally, so ordering is enforced there.
            let isrc_fut = async {
                if let Some(ref code) = isrc {
                    mb.lookup_isrc(code, &track_name, &artist_name).await
                } else {
                    None
                }
            };
            let name_fut = mb.lookup_by_name(&track_name, &artist_name);
            let (recording_from_isrc, name_resolved) = tokio::join!(isrc_fut, name_fut);

            let recording_mbid = recording_from_isrc
                .or_else(|| name_resolved.as_ref().and_then(|r| r.recording_mbid.clone()));
            let release_group_mbid =
                name_resolved.as_ref().and_then(|r| r.release_group_mbid.clone());
            let artist_mbid = name_resolved.as_ref().and_then(|r| r.artist_mbid.clone());

            if recording_mbid.is_none()
                && release_group_mbid.is_none()
                && artist_mbid.is_none()
            {
                return;
            }

            // Apply MBIDs to the live track first.
            {
                let mut current = ct.lock().await;
                if let Some(ref mut playback) = *current {
                    let same_track = match expected_id {
                        Some(id) => playback.track.track_id == Some(id),
                        None => {
                            playback.track.track == track_name
                                && playback.track.artist == artist_name
                        }
                    };
                    if same_track {
                        if let Some(ref v) = recording_mbid {
                            playback.track.recording_mbid = Some(v.clone());
                        }
                        if let Some(v) = release_group_mbid {
                            playback.track.release_group_mbid = Some(v);
                        }
                        if let Some(v) = artist_mbid {
                            playback.track.artist_mbid = Some(v);
                        }
                    }
                }
            }

            // Phase 1 (Classical Hub): resolve parent Work MBID, off the
            // critical path. Best-effort; failure does not affect the
            // play in any way.
            //
            // Phase 3 (B3.3): once the work_mbid lands on the live track,
            // emit `classical:work-resolved` so the frontend can show the
            // "View work" affordance + work header without polling. The
            // emission happens AFTER the playback state is updated, so
            // listeners reading `current_work_mbid` see a consistent value
            // when they react to the event.
            if let (Some(rec_mbid), Some(resolver)) =
                (recording_mbid.as_ref(), work_resolver.as_ref())
            {
                if let Some(work_mbid) =
                    resolver.resolve_work_for_recording(rec_mbid).await
                {
                    let mut applied = false;
                    {
                        let mut current = ct.lock().await;
                        if let Some(ref mut playback) = *current {
                            let same_track = match expected_id {
                                Some(id) => playback.track.track_id == Some(id),
                                None => {
                                    playback.track.track == track_name
                                        && playback.track.artist == artist_name
                                }
                            };
                            if same_track {
                                playback.track.work_mbid = Some(work_mbid.clone());
                                applied = true;
                            }
                        }
                    }
                    if applied {
                        let _ = event_handle.emit(
                            "classical:work-resolved",
                            serde_json::json!({
                                "trackId": expected_id,
                                "recordingMbid": rec_mbid,
                                "workMbid": work_mbid,
                            }),
                        );
                    }
                }
            }
        });
    }

    pub async fn on_pause(&self) {
        let mut current = self.current_track.lock().await;
        if let Some(playback) = current.as_mut() {
            playback.pause();
        }
    }

    pub async fn on_resume(&self) {
        let mut current = self.current_track.lock().await;
        if let Some(playback) = current.as_mut() {
            playback.resume();
        }
    }

    pub async fn on_seek(&self) {
        let mut current = self.current_track.lock().await;
        if let Some(playback) = current.as_mut() {
            playback.on_seek();
        }
    }

    /// Called when the audio stream ends naturally (EOS event).
    /// Peeks at the current track and scrobbles if threshold is met, but
    /// NEVER removes it from `current_track`. This prevents a stale EOS
    /// event from destroying a newly-started track's tracking state.
    pub async fn try_scrobble_finished(&self) {
        let track_to_scrobble = {
            let mut current = self.current_track.lock().await;
            if let Some(ref mut playback) = *current {
                if !playback.scrobbled && playback.meets_threshold() {
                    playback.scrobbled = true;
                    Some(playback.track.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(track) = track_to_scrobble {
            self.dispatch_scrobble(track).await;
        }
    }

    /// Called on explicit stop (user action). Scrobbles if threshold is met
    /// and unconditionally clears the current track.
    pub async fn on_track_stopped(&self) {
        let prev_playback = {
            let mut current = self.current_track.lock().await;
            current.take()
        };
        let track_to_scrobble = prev_playback.and_then(|mut p| {
            self.record_to_stats(&p);
            if !p.scrobbled && p.meets_threshold() {
                p.scrobbled = true;
                Some(p.track)
            } else {
                None
            }
        });
        if let Some(track) = track_to_scrobble {
            self.dispatch_scrobble(track).await;
        }
    }

    /// Shutdown: scrobble current if threshold met, persist queue.
    pub async fn flush(&self) {
        // Try to scrobble current track with a 2s timeout
        let prev_playback = {
            let mut current = self.current_track.lock().await;
            current.take()
        };
        let track_to_scrobble = prev_playback.and_then(|mut playback| {
            self.record_to_stats(&playback);
            if !playback.scrobbled && playback.meets_threshold() {
                playback.scrobbled = true;
                Some(playback.track.clone())
            } else {
                None
            }
        });

        if let Some(track) = track_to_scrobble {
            let _ =
                tokio::time::timeout(Duration::from_secs(2), self.dispatch_scrobble(track)).await;
        }

        self.queue.flush().await;
        self.mb_lookup.persist().await;
    }

    /// Send a scrobbled track to all connected providers.
    /// Queue failures for retry. Emit auth errors to the frontend.
    async fn dispatch_scrobble(&self, track: ScrobbleTrack) {
        // Collect authenticated provider names under the lock, then drop it
        // so we never await provider calls while the lock is held.
        let names: Vec<String> = {
            let providers = self.providers.read().await;
            providers
                .iter()
                .filter(|p| p.is_authenticated())
                .map(|p| p.name().to_string())
                .collect()
        };

        for name in names {
            // Acquire, call, and drop the lock per-provider.
            // The scrobble() call returns a boxed future (async_trait);
            // we must await it while still borrowing the guard, but a read
            // lock is cheap and only blocks writers briefly.
            let providers = self.providers.read().await;
            let Some(provider) = providers.iter().find(|p| p.name() == name) else {
                continue;
            };
            let result = provider.scrobble(std::slice::from_ref(&track)).await;
            drop(providers);

            match result {
                ScrobbleResult::Ok => {
                    log::debug!("Scrobbled to {name}: {} - {}", track.artist, track.track);
                }
                ScrobbleResult::AuthError(msg) => {
                    log::warn!("Scrobble auth error for {name}: {msg}");
                    let _ = self.app_handle.emit("scrobble-auth-error", &name);
                }
                ScrobbleResult::Retryable(msg) => {
                    log::warn!("Scrobble failed for {name} (will retry): {msg}");
                    self.queue.push(&name, track.clone()).await;
                }
            }
        }
    }

    /// Drain the retry queue: send queued scrobbles to their providers.
    /// Called once on startup after providers are registered.
    pub async fn drain_queue(&self) {
        // Clean up entries for disconnected providers / expired entries first
        let connected: Vec<String> = {
            let providers = self.providers.read().await;
            providers
                .iter()
                .filter(|p| p.is_authenticated())
                .map(|p| p.name().to_string())
                .collect()
        };
        self.queue.cleanup(&connected).await;

        let total = self.queue.len().await;
        if total == 0 {
            return;
        }
        log::info!("Draining scrobble retry queue ({total} entries)");

        for provider_name in &connected {
            let pending = self.queue.take_for_provider(provider_name).await;
            if pending.is_empty() {
                continue;
            }
            log::info!(
                "Retrying {} queued scrobbles for {provider_name}",
                pending.len()
            );

            let batch_size = {
                let providers = self.providers.read().await;
                providers
                    .iter()
                    .find(|p| p.name() == provider_name)
                    .map(|p| p.max_batch_size())
                    .unwrap_or(50)
            };

            let mut failed: Vec<(ScrobbleTrack, u32)> = Vec::new();
            let chunks: Vec<&[(ScrobbleTrack, u32)]> = pending.chunks(batch_size).collect();
            let mut chunk_idx = 0;
            while chunk_idx < chunks.len() {
                let chunk = chunks[chunk_idx];
                chunk_idx += 1;
                let tracks: Vec<ScrobbleTrack> = chunk.iter().map(|(t, _)| t.clone()).collect();

                // Acquire lock, find provider, drop lock before network call
                let provider_exists = {
                    let providers = self.providers.read().await;
                    providers.iter().any(|p| p.name() == provider_name)
                };
                if !provider_exists {
                    // Provider removed — requeue this chunk and all remaining
                    failed.extend(chunk.iter().cloned());
                    for remaining in &chunks[chunk_idx..] {
                        failed.extend(remaining.iter().cloned());
                    }
                    break;
                }

                let result = {
                    let providers = self.providers.read().await;
                    let provider = providers
                        .iter()
                        .find(|p| p.name() == provider_name)
                        .unwrap();
                    tokio::time::timeout(Duration::from_secs(15), provider.scrobble(&tracks)).await
                };

                match result {
                    Ok(ScrobbleResult::Ok) => {
                        log::debug!("Retried {} scrobbles to {provider_name}", tracks.len());
                    }
                    _ => {
                        match &result {
                            Ok(ScrobbleResult::AuthError(msg)) => {
                                log::warn!("Auth error draining queue for {provider_name}: {msg}");
                                let _ = self.app_handle.emit("scrobble-auth-error", provider_name);
                            }
                            Ok(ScrobbleResult::Retryable(msg)) => {
                                log::warn!("Retry failed for {provider_name}: {msg}");
                            }
                            Err(_) => {
                                log::warn!("Timeout draining queue for {provider_name}");
                            }
                            _ => {}
                        }
                        // Requeue current chunk + all remaining unprocessed chunks
                        failed.extend(chunk.iter().cloned());
                        for remaining in &chunks[chunk_idx..] {
                            failed.extend(remaining.iter().cloned());
                        }
                        break;
                    }
                }
            }

            if !failed.is_empty() {
                log::info!(
                    "Re-queuing {} failed scrobbles for {provider_name}",
                    failed.len()
                );
                self.queue.requeue(provider_name, failed).await;
            }
        }
    }

    pub async fn queue_size(&self) -> usize {
        self.queue.len().await
    }

    /// Expose the MB lookup helper to other modules (e.g. tauri command
    /// handlers that want to resolve covers or fetch full recording
    /// details without duplicating the cache + rate-limit machinery).
    pub fn mb_lookup(&self) -> Arc<musicbrainz::MusicBrainzLookup> {
        Arc::clone(&self.mb_lookup)
    }

    /// Backfill the local stats DB with a user's ListenBrainz history.
    ///
    /// The user must be connected to ListenBrainz (the username is taken
    /// from the connected provider). Listens are paginated newest-first,
    /// converted to local play records, and inserted via the dedup-aware
    /// `bulk_import_plays`. Progress is emitted to the frontend on the
    /// `import-listenbrainz-progress` channel after every page so the UI
    /// can render a live counter.
    ///
    /// The walk stops on:
    ///  * an empty page (we ran out of history),
    ///  * three consecutive pages where ≥95% of rows were already in the
    ///    local DB (signals a re-import — no new history beyond this),
    ///  * the page's oldest timestamp falling below `since_unix`.
    ///
    /// Public listens require no token; private profiles will fail with
    /// 401 — the caller can flip their LB profile to public and retry.
    pub async fn import_listenbrainz_history(
        &self,
        since_unix: Option<i64>,
    ) -> Result<ImportResult, SoneError> {
        let username: String = {
            let providers = self.providers.read().await;
            let provider = providers
                .iter()
                .find(|p| p.name() == "listenbrainz" && p.is_authenticated())
                .ok_or_else(|| SoneError::Scrobble("listenbrainz: not connected".into()))?;
            provider
                .username()
                .await
                .ok_or_else(|| SoneError::Scrobble("listenbrainz: no username".into()))?
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| SoneError::Scrobble(format!("import client build failed: {e}")))?;

        let min_ts = since_unix.unwrap_or(0);
        let mut max_ts: Option<i64> = None;
        let mut imported: u64 = 0;
        let mut skipped: u64 = 0;
        let mut pages: u32 = 0;
        let mut consecutive_dupes: u32 = 0;
        const PAGE: u32 = 1000;
        const MAX_PAGES: u32 = 100;

        loop {
            if pages >= MAX_PAGES {
                log::warn!("[lb-import] hit page cap ({MAX_PAGES})");
                break;
            }
            let page = listenbrainz::ListenBrainzProvider::fetch_listens(
                &client,
                None,
                &username,
                max_ts,
                Some(min_ts),
                PAGE,
            )
            .await?;
            pages += 1;

            if page.listens.is_empty() {
                break;
            }

            let oldest_in_page = page.oldest_ts;
            let records: Vec<PlayRecord> = page
                .listens
                .iter()
                .map(|l| {
                    let dur = l.duration_secs.unwrap_or(180);
                    PlayRecord {
                        started_at: l.listened_at,
                        finished_at: l.listened_at + dur as i64,
                        track_id: None,
                        title: l.track_name.as_str(),
                        artist: l.artist_name.as_str(),
                        album: l.release_name.as_deref(),
                        album_artist: None,
                        duration_secs: dur,
                        listened_secs: dur,
                        completed: true,
                        isrc: l.isrc.as_deref(),
                        chosen_by_user: true,
                        source: "listenbrainz",
                        recording_mbid: l.recording_mbid.as_deref(),
                        release_group_mbid: None,
                        artist_mbid: None,
                        work_mbid: None,
                    }
                })
                .collect();

            let res = self
                .stats
                .bulk_import_plays(&records)
                .map_err(|e| SoneError::Scrobble(format!("import db error: {e}")))?;
            imported += res.imported;
            skipped += res.skipped;

            let _ = self.app_handle.emit(
                "import-listenbrainz-progress",
                serde_json::json!({
                    "page": pages,
                    "imported": imported,
                    "skipped": skipped,
                    "oldestTs": oldest_in_page,
                }),
            );

            let total = res.imported + res.skipped;
            if total > 0 && res.skipped * 100 / total >= 95 {
                consecutive_dupes += 1;
                if consecutive_dupes >= 3 {
                    log::info!("[lb-import] stopping — three consecutive duplicate pages");
                    break;
                }
            } else {
                consecutive_dupes = 0;
            }

            if oldest_in_page <= min_ts {
                break;
            }
            max_ts = Some(oldest_in_page - 1);
            tokio::time::sleep(Duration::from_millis(350)).await;
        }

        Ok(ImportResult {
            imported,
            skipped,
            pages,
            username,
        })
    }

    /// Backfill the local stats DB with a user's Last.fm history.
    ///
    /// Same shape as `import_listenbrainz_history`: paginates the public
    /// `user.getRecentTracks` endpoint newest-first using the embedded
    /// API key (no session required for public profiles), converts each
    /// scrobble to a `PlayRecord`, and dedups against existing rows
    /// inside `bulk_import_plays`. Progress is emitted on the
    /// `import-lastfm-progress` channel after every page.
    ///
    /// The walk stops on:
    ///  * an empty page,
    ///  * three consecutive pages where ≥95% of rows were duplicates,
    ///  * the page index passing the API's reported `totalPages`,
    ///  * the per-call `MAX_PAGES` cap.
    pub async fn import_lastfm_history(
        &self,
        since_unix: Option<i64>,
    ) -> Result<ImportResult, SoneError> {
        if !crate::embedded_lastfm::has_stream_keys() {
            return Err(SoneError::Scrobble("Last.fm not configured".into()));
        }
        let username: String = {
            let providers = self.providers.read().await;
            let provider = providers
                .iter()
                .find(|p| p.name() == "lastfm" && p.is_authenticated())
                .ok_or_else(|| SoneError::Scrobble("lastfm: not connected".into()))?;
            provider
                .username()
                .await
                .ok_or_else(|| SoneError::Scrobble("lastfm: no username".into()))?
        };
        let api_key = crate::embedded_lastfm::stream_key_a();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| SoneError::Scrobble(format!("import client build failed: {e}")))?;

        let from_ts = since_unix.filter(|t| *t > 0);
        let mut imported: u64 = 0;
        let mut skipped: u64 = 0;
        let mut pages: u32 = 0;
        let mut consecutive_dupes: u32 = 0;
        const PAGE_SIZE: u32 = 200;
        const MAX_PAGES: u32 = 250;

        for page_idx in 1..=MAX_PAGES {
            let page = lastfm::fetch_recent_tracks(
                &client,
                &api_key,
                &username,
                from_ts,
                page_idx,
                PAGE_SIZE,
            )
            .await?;
            pages = page_idx;

            if page.tracks.is_empty() {
                break;
            }

            let oldest_in_page = page
                .tracks
                .iter()
                .map(|t| t.listened_at)
                .min()
                .unwrap_or(0);

            // Last.fm doesn't return duration. 180 s is the same default
            // the LB importer uses — gives reasonable listened_secs for
            // aggregate stats without making API calls per row.
            let dur: u32 = 180;
            let records: Vec<PlayRecord> = page
                .tracks
                .iter()
                .map(|t| PlayRecord {
                    started_at: t.listened_at,
                    finished_at: t.listened_at + dur as i64,
                    track_id: None,
                    title: t.track_name.as_str(),
                    artist: t.artist_name.as_str(),
                    album: t.album_name.as_deref(),
                    album_artist: None,
                    duration_secs: dur,
                    listened_secs: dur,
                    completed: true,
                    isrc: None,
                    chosen_by_user: true,
                    source: "lastfm",
                    recording_mbid: t.recording_mbid.as_deref(),
                    release_group_mbid: None,
                    artist_mbid: None,
                    work_mbid: None,
                })
                .collect();

            let res = self
                .stats
                .bulk_import_plays(&records)
                .map_err(|e| SoneError::Scrobble(format!("import db error: {e}")))?;
            imported += res.imported;
            skipped += res.skipped;

            let _ = self.app_handle.emit(
                "import-lastfm-progress",
                serde_json::json!({
                    "page": pages,
                    "totalPages": page.total_pages,
                    "imported": imported,
                    "skipped": skipped,
                    "oldestTs": oldest_in_page,
                }),
            );

            let total = res.imported + res.skipped;
            if total > 0 && res.skipped * 100 / total >= 95 {
                consecutive_dupes += 1;
                if consecutive_dupes >= 3 {
                    log::info!("[lfm-import] stopping — three consecutive duplicate pages");
                    break;
                }
            } else {
                consecutive_dupes = 0;
            }

            if page.total_pages > 0 && page_idx >= page.total_pages {
                break;
            }
            tokio::time::sleep(Duration::from_millis(350)).await;
        }

        Ok(ImportResult {
            imported,
            skipped,
            pages,
            username,
        })
    }

    /// Fire now_playing to all providers (non-blocking, with timeout).
    async fn fire_now_playing(&self, track: &ScrobbleTrack) {
        let names: Vec<String> = {
            let providers = self.providers.read().await;
            providers
                .iter()
                .filter(|p| p.is_authenticated())
                .map(|p| p.name().to_string())
                .collect()
        };

        for name in names {
            let providers = self.providers.read().await;
            let Some(provider) = providers.iter().find(|p| p.name() == name) else {
                continue;
            };
            let result =
                tokio::time::timeout(Duration::from_secs(5), provider.now_playing(track)).await;
            drop(providers);

            match result {
                Ok(ScrobbleResult::Ok) => {
                    log::debug!("Now playing sent to {name}");
                }
                Ok(ScrobbleResult::AuthError(msg)) => {
                    log::warn!("Now playing auth error for {name}: {msg}");
                    let _ = self.app_handle.emit("scrobble-auth-error", &name);
                }
                Ok(ScrobbleResult::Retryable(msg)) => {
                    log::debug!("Now playing failed for {name} (non-critical): {msg}");
                }
                Err(_) => {
                    log::debug!("Now playing timed out for {name}");
                }
            }
        }
    }
}
