mod audio;
pub mod cache;
pub mod cli;
mod commands;
mod crypto;
mod discord;
mod embedded_config;
mod embedded_lastfm;
mod embedded_librefm;
mod error;
mod hooks;
mod hw_volume;
mod idle_inhibit;
mod llm;
#[cfg(target_os = "linux")]
mod mpris;
mod scrobble;
mod share_link;
mod signal_path;
mod stats;
#[cfg(target_os = "linux")]
mod tray;
mod tidal_api;

pub use error::SoneError;
pub use signal_path::{SignalPath, SignalPathTracker};

use audio::{AudioDevice, AudioPlayer};
use cache::DiskCache;
use crypto::Crypto;
use hw_volume::HwVolume;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};
use tidal_api::{AuthTokens, TidalClient};
use tokio::sync::Mutex;

mod defaults {
    pub fn yes() -> bool { true }
    pub fn volume() -> f32 { 1.0 }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LastfmCredentials {
    pub session_key: String,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ListenBrainzCredentials {
    pub token: String,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ScrobbleSettings {
    pub lastfm: Option<LastfmCredentials>,
    pub librefm: Option<LastfmCredentials>,
    pub listenbrainz: Option<ListenBrainzCredentials>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    Http,
    Socks5,
}

impl Default for ProxyType {
    fn default() -> Self {
        Self::Http
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProxySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub proxy_type: ProxyType,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub auth_tokens: Option<AuthTokens>,
    #[serde(default = "defaults::volume")]
    pub volume: f32,
    pub last_track_id: Option<u64>,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub llm: llm::LLMSettings,
    #[serde(default)]
    pub minimize_to_tray: bool,
    #[serde(default = "defaults::yes")]
    pub decorations: bool,
    #[serde(default)]
    pub volume_normalization: bool,
    #[serde(default)]
    pub exclusive_mode: bool,
    #[serde(default)]
    pub exclusive_device: Option<String>,
    #[serde(default)]
    pub bit_perfect: bool,
    #[serde(default)]
    pub scrobble: ScrobbleSettings,
    #[serde(default)]
    pub proxy: ProxySettings,
    #[serde(default = "defaults::yes")]
    pub discord_rpc: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auth_tokens: None,
            volume: 1.0,
            last_track_id: None,
            client_id: String::new(),
            client_secret: String::new(),
            llm: Default::default(),
            minimize_to_tray: false,
            decorations: true,
            volume_normalization: false,
            exclusive_mode: false,
            exclusive_device: None,
            bit_perfect: false,
            scrobble: Default::default(),
            proxy: Default::default(),
            discord_rpc: true,
        }
    }
}

pub struct AppState {
    pub audio_player: AudioPlayer,
    pub llm_settings: Mutex<llm::LLMSettings>,
    pub share_link: Arc<share_link::ShareLink>,
    pub tidal_client: Mutex<TidalClient>,
    pub settings_path: PathBuf,
    pub cache_dir: PathBuf,
    pub disk_cache: DiskCache,
    pub crypto: Arc<Crypto>,
    pub minimize_to_tray: AtomicBool,
    pub decorations: AtomicBool,
    pub volume_normalization: AtomicBool,
    pub exclusive_mode: AtomicBool,
    pub bit_perfect: AtomicBool,
    pub exclusive_device: std::sync::Mutex<Option<String>>,
    pub cached_audio_devices: std::sync::Mutex<Option<Vec<AudioDevice>>>,
    /// Current track's selected replay gain (dB) stored as f64 bits. NAN = no data.
    /// Album or track gain depending on playback context.
    pub last_replay_gain: AtomicU64,
    /// Current track's selected peak amplitude (linear) stored as f64 bits. NAN = no data.
    /// Album or track peak depending on playback context.
    pub last_peak_amplitude: AtomicU64,
    #[cfg(target_os = "linux")]
    pub mpris: mpris::MprisHandle,
    pub scrobble_manager: scrobble::ScrobbleManager,
    pub discord: discord::DiscordHandle,
    pub idle_inhibitor: Mutex<idle_inhibit::IdleInhibitor>,
    pub signal_path: Arc<SignalPathTracker>,
    pub hooks: Arc<hooks::HooksManager>,
    pub stats: Arc<stats::StatsDb>,
    /// Hardware volume control on the active exclusive ALSA device, if the
    /// DAC exposes a usable mixer control. None when not in DirectAlsa mode
    /// or the device has no playback volume control.
    pub hw_volume: std::sync::Mutex<Option<Arc<HwVolume>>>,
    /// Monotonic id used by the wheel-mirror thread to know when the
    /// underlying device has been swapped — incremented on every
    /// `open_hw_volume`. Old mirror threads exit when they see a newer id.
    pub hw_volume_bus: Arc<std::sync::atomic::AtomicU64>,
    /// AppHandle stash — the hw_volume open path needs it to spawn the
    /// wheel-mirror thread. Set once at startup.
    pub app_handle: std::sync::Mutex<Option<tauri::AppHandle>>,
    /// Last quality tier that succeeded against the current TIDAL session.
    /// Used as the starting point for the next track's cascade so we skip
    /// dead tiers (saves 1-2 s per track change).
    pub last_successful_quality: std::sync::Mutex<String>,
    /// Pre-fetched stream URLs keyed by track id with their fetch instant.
    /// Entries older than 60 s are discarded — TIDAL signed URLs expire.
    pub stream_url_cache:
        std::sync::Mutex<std::collections::HashMap<u64, (tidal_api::StreamInfo, std::time::Instant)>>,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl AppState {
    fn new(app_handle: tauri::AppHandle) -> Self {
        // Get config dir
        let mut config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        config_dir.push("sone");
        fs::create_dir_all(&config_dir).ok();

        let settings_path = config_dir.join("settings.json");
        let cache_dir = config_dir.join("cache");
        fs::create_dir_all(&cache_dir).ok();

        // Initialize encryption
        let crypto = match Crypto::new(&config_dir) {
            Ok(c) => Arc::new(c),
            Err(e) => {
                log::error!("Failed to initialize crypto: {e}. Data-at-rest encryption disabled.");
                panic!("Crypto initialization failed: {e}");
            }
        };

        let disk_cache = DiskCache::new(&cache_dir, crypto.clone());

        // Load preferences from saved settings (decrypt if needed)
        let saved = fs::read(&settings_path)
            .ok()
            .and_then(|data| crypto.decrypt(&data).ok())
            .and_then(|plain| String::from_utf8(plain).ok())
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok());

        // Eager migration: if settings exist but aren't encrypted, re-save encrypted
        if settings_path.exists() {
            if let Ok(raw) = fs::read(&settings_path) {
                if !crypto::is_encrypted(&raw) {
                    if let Some(ref settings) = saved {
                        if let Ok(json) = serde_json::to_string_pretty(settings) {
                            if let Ok(encrypted) = crypto.encrypt(json.as_bytes()) {
                                if let Err(e) = fs::write(&settings_path, encrypted) {
                                    log::warn!("Failed to migrate settings to encrypted: {e}");
                                } else {
                                    log::info!("Migrated settings.json to encrypted format");
                                }
                            }
                        }
                    }
                }
            }
        }

        let minimize_to_tray = saved.as_ref().map(|s| s.minimize_to_tray).unwrap_or(false);
        let decorations = saved.as_ref().map(|s| s.decorations).unwrap_or(true);
        let volume_normalization = saved
            .as_ref()
            .map(|s| s.volume_normalization)
            .unwrap_or(false);
        let exclusive_mode = saved.as_ref().map(|s| s.exclusive_mode).unwrap_or(false);
        let bit_perfect = saved.as_ref().map(|s| s.bit_perfect).unwrap_or(false);
        let exclusive_device = saved.as_ref().and_then(|s| s.exclusive_device.clone());

        let proxy_settings = saved.as_ref().map(|s| s.proxy.clone()).unwrap_or_default();
        let scrobble_http_client = crate::tidal_api::build_http_client(&proxy_settings)
            .unwrap_or_else(|_| {
                reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .unwrap()
            });

        let stats = Arc::new(stats::StatsDb::open(&config_dir).unwrap_or_else(|e| {
            log::error!("Failed to open stats db: {e}. Listening stats disabled.");
            panic!("StatsDb open failed: {e}");
        }));

        let scrobble_manager = scrobble::ScrobbleManager::new(
            app_handle.clone(),
            crypto.clone(),
            &config_dir,
            scrobble_http_client,
            Arc::clone(&stats),
        );

        let discord_rpc_enabled = saved.as_ref().map(|s| s.discord_rpc).unwrap_or(true);
        let discord_handle = discord::DiscordHandle::new();
        if discord_rpc_enabled {
            discord_handle.send(discord::DiscordCommand::Connect);
        }

        let signal_path = Arc::new(SignalPathTracker::new(app_handle.clone()));
        signal_path.set_audio_modes(exclusive_mode, bit_perfect);
        signal_path.set_normalization_enabled(volume_normalization);

        let llm_settings_initial = saved
            .as_ref()
            .map(|s| s.llm.clone())
            .unwrap_or_default();

        // Shared broadcast channel: audio thread pushes Opus/Ogg pages,
        // share-link HTTP listeners subscribe. Capacity sized for ~6 s of
        // audio at 256 kbps (~24 KB/s of Ogg pages, multiple pages per buffer
        // depending on muxer flush behavior). 256 buffers is generous; older
        // pages drop if a listener is too slow (handled gracefully).
        let share_broadcaster: audio::ShareBroadcast = {
            let (tx, _) = tokio::sync::broadcast::channel(256);
            Arc::new(tx)
        };
        let audio_player = AudioPlayer::new(
            app_handle.clone(),
            Arc::clone(&signal_path),
            Arc::clone(&share_broadcaster),
        );
        let share_link = Arc::new(share_link::ShareLink::new(
            audio_player.clone(),
            app_handle.clone(),
        ));

        Self {
            audio_player,
            llm_settings: Mutex::new(llm_settings_initial),
            share_link,
            tidal_client: Mutex::new(TidalClient::new(&proxy_settings)),
            settings_path,
            cache_dir,
            disk_cache,
            crypto,
            minimize_to_tray: AtomicBool::new(minimize_to_tray),
            decorations: AtomicBool::new(decorations),
            volume_normalization: AtomicBool::new(volume_normalization),
            exclusive_mode: AtomicBool::new(exclusive_mode),
            bit_perfect: AtomicBool::new(bit_perfect),
            exclusive_device: std::sync::Mutex::new(exclusive_device),
            cached_audio_devices: std::sync::Mutex::new(None),
            last_replay_gain: AtomicU64::new(f64::NAN.to_bits()),
            last_peak_amplitude: AtomicU64::new(f64::NAN.to_bits()),
            #[cfg(target_os = "linux")]
            mpris: mpris::MprisHandle::new(app_handle.clone()),
            scrobble_manager,
            discord: discord_handle,
            idle_inhibitor: Mutex::new(idle_inhibit::IdleInhibitor::new()),
            signal_path,
            hooks: Arc::new(hooks::HooksManager::new(&config_dir)),
            stats,
            hw_volume: std::sync::Mutex::new(None),
            hw_volume_bus: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            app_handle: std::sync::Mutex::new(Some(app_handle)),
            last_successful_quality: std::sync::Mutex::new(String::new()),
            stream_url_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn load_settings(&self) -> Option<Settings> {
        let data = fs::read(&self.settings_path).ok()?;
        let plain = self.crypto.decrypt(&data).ok()?;
        let text = String::from_utf8(plain).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), SoneError> {
        let json = serde_json::to_string_pretty(settings)?;
        let encrypted = self.crypto.encrypt(json.as_bytes())?;
        fs::write(&self.settings_path, encrypted)?;
        Ok(())
    }

    // ---- Persistent state (not cache — survives restarts) ----

    pub fn read_state_file(&self, name: &str) -> Option<String> {
        let path = self.cache_dir.join(name);
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                log::warn!("Failed to read state file {name}: {e}");
                return None;
            }
        };
        let plain = match self.crypto.decrypt(&data) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Failed to decrypt state file {name}: {e}");
                return None;
            }
        };
        match String::from_utf8(plain) {
            Ok(s) => Some(s),
            Err(e) => {
                log::warn!("State file {name} contains invalid UTF-8: {e}");
                None
            }
        }
    }

    pub fn write_state_file(&self, name: &str, content: &str) -> Result<(), SoneError> {
        let path = self.cache_dir.join(name);
        let encrypted = self.crypto.encrypt(content.as_bytes())?;
        fs::write(&path, encrypted)?;
        Ok(())
    }

    /// Open (or replace) the HW volume mixer for the given exclusive ALSA
    /// device. Returns whether a usable HW control was found. Called from
    /// the exclusive-mode lifecycle (set_exclusive_device, set_exclusive_mode).
    /// Spawns a wheel-mirror polling thread so the DAC's physical control
    /// reflects in the UI / MPRIS in real time.
    pub fn open_hw_volume(&self, device: &str) -> bool {
        let opened = HwVolume::try_open(device).map(Arc::new);
        let available = opened.is_some();
        // Bump the bus id BEFORE swapping, so any in-flight mirror thread
        // exits on its next tick instead of polling the old (about-to-be-
        // dropped) HwVolume.
        let bus_id = self
            .hw_volume_bus
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        *self.hw_volume.lock().unwrap() = opened.clone();

        if let (Some(hw), Some(app)) = (opened, self.app_handle.lock().unwrap().clone()) {
            #[cfg(target_os = "linux")]
            hw_volume::spawn_wheel_mirror(hw, app, Arc::clone(&self.hw_volume_bus), bus_id);
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (hw, app, bus_id);
            }
        }
        available
    }

    pub fn close_hw_volume(&self) {
        // Bump bus so any active mirror thread observes the change and exits.
        self.hw_volume_bus
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        *self.hw_volume.lock().unwrap() = None;
    }

    pub fn current_hw_volume(&self) -> Option<Arc<HwVolume>> {
        self.hw_volume.lock().unwrap().as_ref().map(Arc::clone)
    }
}

/// Result of a routed volume change. Reported back to the frontend so the
/// UI can reflect what actually happened (HW vs SW vs rejection).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeRoute {
    /// Applied to the DAC's analog gain via ALSA mixer (preserves bit-perfect).
    Hw,
    /// Applied to the PCM stream via software scaling (NOT bit-perfect).
    Sw,
    /// Rejected: bit-perfect on, no HW control available. Volume locked at 1.0.
    Locked,
    /// Applied to the GStreamer volume element in the Normal pipeline.
    Gst,
}

/// Single-source-of-truth volume routing.
///
/// Bit-perfect contract: when `bit_perfect=true`, the only paths that may
/// run are `Hw` (mixer) or `Locked` (no-op). The `Sw` path is FORBIDDEN
/// in bit-perfect mode — and even if a future bug bypassed this routing,
/// the alsa-writer thread has a hard guard that refuses to apply software
/// volume when bit_perfect is on.
pub fn route_volume_change(state: &AppState, level: f32) -> Result<VolumeRoute, SoneError> {
    let level = level.clamp(0.0, 1.0);
    let exclusive = state.exclusive_mode.load(Ordering::Relaxed);
    let bit_perfect = state.bit_perfect.load(Ordering::Relaxed);

    if !exclusive && !bit_perfect {
        // Normal pipeline: GStreamer volume element handles it.
        state
            .audio_player
            .set_volume(level)
            .map_err(SoneError::Audio)?;
        return Ok(VolumeRoute::Gst);
    }

    // DirectAlsa territory. Try HW first.
    if let Some(hw) = state.current_hw_volume() {
        hw.set(level).map_err(SoneError::Audio)?;
        // In bit-perfect mode the SW path is forbidden; freeze user_volume
        // at 1.0 so combined_vol stays unity (irrelevant due to writer
        // guard, but keeps internal state honest).
        // In exclusive (non-BP) mode we ALSO skip the SW path because HW is
        // strictly better — sample-scaling adds quantization noise.
        state
            .audio_player
            .set_volume(1.0)
            .map_err(SoneError::Audio)?;
        return Ok(VolumeRoute::Hw);
    }

    // No HW control available.
    if bit_perfect {
        // Hard reject: bit-perfect promise must hold. Frontend will read
        // this and lock the slider.
        state
            .audio_player
            .set_volume(1.0)
            .map_err(SoneError::Audio)?;
        return Ok(VolumeRoute::Locked);
    }

    // Exclusive without bit-perfect, no HW: fall back to SW sample scaling.
    // Acceptable here because the user explicitly chose exclusive (system-
    // bypass for latency / format-locking) without the strict-bits promise.
    state
        .audio_player
        .set_volume(level)
        .map_err(SoneError::Audio)?;
    Ok(VolumeRoute::Sw)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::SIZE,
                )
                .build(),
        )
        .setup(|app| {
            // Single-instance: focus existing window if launched again
            app.handle().plugin(
                tauri_plugin_single_instance::init(|app, _args, _cwd| {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }),
            )?;
            // Deep link: register tidal:// scheme handler
            app.handle().plugin(tauri_plugin_deep_link::init())?;
            #[cfg(target_os = "linux")]
            if let Err(e) = app.deep_link().register_all() {
                log::warn!("Deep link registration failed: {e}");
            }

            app.manage(AppState::new(app.handle().clone()));

            // Apply saved audio mode to audio thread
            {
                let state = app.state::<AppState>();
                let excl = state
                    .exclusive_mode
                    .load(std::sync::atomic::Ordering::Relaxed);
                let bp = state.bit_perfect.load(std::sync::atomic::Ordering::Relaxed);
                let dev = state.exclusive_device.lock().unwrap().clone();
                if excl || bp {
                    state
                        .audio_player
                        .set_exclusive_mode(excl, dev.clone())
                        .ok();
                    if let Some(ref d) = dev {
                        state.open_hw_volume(d);
                    }
                }
                if bp {
                    state.audio_player.set_bit_perfect(true).ok();
                }
            }

            // Pre-warm audio device cache in background (GStreamer probe is slow)
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    if let Ok(devices) = crate::audio::list_alsa_devices() {
                        let state = handle.state::<AppState>();
                        *state.cached_audio_devices.lock().unwrap() = Some(devices);
                    }
                });
            }

            // Initialize scrobble providers from saved credentials
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<AppState>();
                    if let Some(settings) = state.load_settings() {
                        let http_client = crate::tidal_api::build_http_client(
                            &settings.proxy
                        ).unwrap_or_else(|_| {
                            reqwest::Client::builder()
                                .timeout(std::time::Duration::from_secs(30))
                                .build()
                                .unwrap()
                        });

                        // Last.fm
                        if let Some(ref creds) = settings.scrobble.lastfm {
                            if crate::embedded_lastfm::has_stream_keys() {
                                let provider = crate::scrobble::lastfm::AudioscrobblerProvider::new(
                                    "lastfm",
                                    "https://ws.audioscrobbler.com/2.0/",
                                    "https://www.last.fm/api/auth/",
                                    crate::embedded_lastfm::stream_key_a(),
                                    crate::embedded_lastfm::stream_key_b(),
                                    http_client.clone(),
                                );
                                provider
                                    .set_session(creds.session_key.clone(), creds.username.clone())
                                    .await;
                                state
                                    .scrobble_manager
                                    .add_provider(Box::new(provider))
                                    .await;
                                log::info!("Last.fm scrobbling enabled for {}", creds.username);
                            }
                        }

                        // Libre.fm
                        if let Some(ref creds) = settings.scrobble.librefm {
                            if crate::embedded_librefm::has_stream_keys() {
                                let provider = crate::scrobble::lastfm::AudioscrobblerProvider::new(
                                    "librefm",
                                    crate::scrobble::librefm::LIBREFM_API_URL,
                                    "https://libre.fm/api/auth/",
                                    crate::embedded_librefm::stream_key_a(),
                                    crate::embedded_librefm::stream_key_b(),
                                    http_client.clone(),
                                );
                                provider
                                    .set_session(creds.session_key.clone(), creds.username.clone())
                                    .await;
                                state
                                    .scrobble_manager
                                    .add_provider(Box::new(provider))
                                    .await;
                                log::info!("Libre.fm scrobbling enabled for {}", creds.username);
                            }
                        }

                        // ListenBrainz
                        if let Some(ref creds) = settings.scrobble.listenbrainz {
                            let provider =
                                crate::scrobble::listenbrainz::ListenBrainzProvider::new(http_client.clone());
                            provider
                                .set_token(creds.token.clone(), creds.username.clone())
                                .await;
                            state
                                .scrobble_manager
                                .add_provider(Box::new(provider))
                                .await;
                            log::info!("ListenBrainz scrobbling enabled for {}", creds.username);
                        }
                    }

                    // Drain retry queue in background
                    state.scrobble_manager.drain_queue().await;
                });
            }

            // Scrobble on track-finished (backend listener)
            {
                let handle = app.handle().clone();
                app.listen("track-finished", move |_| {
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = handle.state::<AppState>();
                        state.scrobble_manager.try_scrobble_finished().await;
                    });
                });
            }

            if let Some(window) = app.get_webview_window("main") {
                let state = app.state::<AppState>();
                // Set window icon at runtime (needed for dev mode taskbar icon)
                let icon_bytes = include_bytes!("../icons/icon.png");
                if let Ok(image) = image::load_from_memory(icon_bytes) {
                    let rgba = image.to_rgba8();
                    let (width, height) = rgba.dimensions();
                    let icon = tauri::image::Image::new(rgba.as_raw(), width, height);
                    let _ = window.set_icon(icon);
                }

                // WebKitGTK rendering settings for Linux
                #[cfg(target_os = "linux")]
                {
                    use webkit2gtk::{SettingsExt, WebViewExt};
                    window
                        .with_webview(|webview| {
                            let wv = webview.inner();
                            if let Some(settings) = wv.settings() {
                                // Use OnDemand (default) — Always can cause severe lag
                                // on dual-GPU systems (NVIDIA + iGPU) with WebKitGTK
                                settings.set_hardware_acceleration_policy(
                                    webkit2gtk::HardwareAccelerationPolicy::OnDemand,
                                );
                                settings.set_enable_webgl(true);
                                settings.set_enable_smooth_scrolling(true);
                            }
                        })
                        .ok();
                }
                
                let decorations = state.decorations.load(Ordering::Relaxed);

                if !decorations {
                    window.set_decorations(false).ok();
                }

                let _ = window.show();
            }

            // System tray icon (ksni — native D-Bus StatusNotifierItem)
            #[cfg(target_os = "linux")]
            tray::setup(app);

            // Global media key shortcuts (non-fatal)
            if let Err(e) = app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |app, shortcut, event| {
                        if event.state() != ShortcutState::Pressed {
                            return;
                        }
                        match shortcut.key {
                            Code::MediaPlayPause => {
                                app.emit("tray:toggle-play", ()).ok();
                            }
                            Code::MediaTrackNext => {
                                app.emit("tray:next-track", ()).ok();
                            }
                            Code::MediaTrackPrevious => {
                                app.emit("tray:prev-track", ()).ok();
                            }
                            _ => {}
                        };
                    })
                    .build(),
            ) {
                log::warn!("Failed to initialize global shortcut plugin: {e}");
            } else {
                let shortcuts = [
                    ("MediaPlayPause", Code::MediaPlayPause),
                    ("MediaTrackNext", Code::MediaTrackNext),
                    ("MediaTrackPrevious", Code::MediaTrackPrevious),
                ];
                for (name, code) in shortcuts {
                    if let Err(e) = app.global_shortcut().register(Shortcut::new(None, code)) {
                        log::warn!("Failed to register global {name} shortcut: {e}");
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if window.label() == "main" {
                        let app = window.app_handle();
                        let state = app.state::<AppState>();
                        if state.minimize_to_tray.load(Ordering::Relaxed) {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                    } else if window.label() == "miniplayer" {
                        let _ = window.app_handle().emit_to("main", "miniplayer-closed", ());
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    if window.label() == "miniplayer" {
                        let _ = window.app_handle().emit_to("main", "miniplayer-closed", ());
                    }
                }
                #[cfg(target_os = "linux")]
                tauri::WindowEvent::Focused(true) => {
                    if window.label() == "miniplayer" {
                        if let Some(ww) = window.app_handle().get_webview_window("miniplayer") {
                            let _ = ww.with_webview(|webview| {
                                use gtk::prelude::WidgetExt;
                                let wv: webkit2gtk::WebView = webview.inner();
                                if let Some(toplevel) = wv.toplevel() {
                                    if let Some(gdk_win) = toplevel.window() {
                                        gdk_win.set_shadow_width(36, 36, 28, 48);
                                    }
                                }
                            });
                        }
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            // auth
            commands::auth::greet,
            commands::auth::load_saved_auth,
            commands::auth::get_saved_credentials,
            commands::auth::get_default_credentials,
            commands::auth::parse_token_data,
            commands::auth::import_session,
            commands::auth::start_device_auth,
            commands::auth::poll_device_auth,
            commands::auth::refresh_tidal_auth,
            commands::auth::start_pkce_auth,
            commands::auth::complete_pkce_auth,
            commands::auth::logout,
            commands::auth::get_session_user_id,
            commands::auth::get_user_profile,
            // library
            commands::library::get_user_playlists,
            commands::library::get_all_playlists,
            commands::library::get_playlist_tracks,
            commands::library::get_playlist_tracks_page,
            commands::library::get_favorite_playlists,
            commands::library::get_favorite_albums,
            commands::library::create_playlist,
            commands::library::update_playlist,
            commands::library::add_track_to_playlist,
            commands::library::remove_track_from_playlist,
            commands::library::delete_playlist,
            commands::library::get_favorite_tracks,
            commands::library::get_favorite_track_ids,
            commands::library::is_track_favorited,
            commands::library::add_favorite_track,
            commands::library::remove_favorite_track,
            commands::library::get_favorite_album_ids,
            commands::library::is_album_favorited,
            commands::library::add_favorite_album,
            commands::library::remove_favorite_album,
            commands::library::get_favorite_playlist_uuids,
            commands::library::add_favorite_playlist,
            commands::library::remove_favorite_playlist,
            commands::library::add_tracks_to_playlist,
            commands::library::get_favorite_artist_ids,
            commands::library::get_all_favorite_ids,
            commands::library::add_favorite_artist,
            commands::library::remove_favorite_artist,
            commands::library::add_favorite_mix,
            commands::library::remove_favorite_mix,
            commands::library::get_favorite_mix_ids,
            commands::library::get_favorite_mixes,
            commands::library::get_favorite_artists,
            commands::library::get_playlist_folders,
            commands::library::create_playlist_folder,
            commands::library::rename_playlist_folder,
            commands::library::delete_playlist_folder,
            commands::library::move_playlist_to_folder,
            commands::library::get_playlist_recommendations,
            // pages
            commands::pages::get_album_detail,
            commands::pages::get_album_page,
            commands::pages::get_album_tracks,
            commands::pages::get_home_page,
            commands::pages::refresh_home_page,
            commands::pages::get_home_page_more,
            commands::pages::get_page_section,
            commands::pages::get_mix_items,
            commands::pages::get_artist_detail,
            commands::pages::get_artist_top_tracks,
            commands::pages::get_artist_albums,
            commands::pages::get_artist_bio,
            commands::pages::get_artist_page,
            commands::pages::get_artist_top_tracks_all,
            commands::pages::get_artist_view_all,
            commands::pages::debug_home_page_raw,
            // search
            commands::search::search_tidal,
            commands::search::get_suggestions,
            // metadata
            commands::metadata::get_stream_url,
            commands::metadata::get_playlist_details,
            commands::metadata::get_track,
            commands::metadata::get_track_lyrics,
            commands::metadata::get_track_credits,
            // playback
            commands::playback::play_tidal_track,
            commands::playback::pause_track,
            commands::playback::resume_track,
            commands::playback::stop_track,
            commands::playback::set_volume,
            commands::playback::get_playback_position,
            commands::playback::seek_track,
            commands::playback::is_track_finished,
            commands::playback::save_playback_queue,
            commands::playback::load_playback_queue,
            commands::playback::update_mpris_metadata,
            commands::playback::update_mpris_playback_status,
            commands::playback::update_mpris_shuffle,
            commands::playback::update_mpris_loop_status,
            // scrobble
            commands::scrobble::notify_track_started,
            commands::scrobble::notify_track_paused,
            commands::scrobble::notify_track_resumed,
            commands::scrobble::notify_track_seeked,
            commands::scrobble::notify_track_stopped,
            commands::scrobble::get_scrobble_status,
            commands::scrobble::get_scrobble_queue_size,
            commands::scrobble::connect_listenbrainz,
            commands::scrobble::connect_lastfm,
            commands::scrobble::connect_librefm,
            commands::scrobble::complete_audioscrobbler_auth,
            commands::scrobble::disconnect_provider,
            commands::scrobble::import_listenbrainz_history,
            commands::musicbrainz::lookup_album_cover_caa,
            commands::musicbrainz::get_mb_track_details,
            commands::lastfm::get_lastfm_similar_tracks,
            commands::lastfm::get_lastfm_track_tags,
            commands::lastfm::get_lastfm_artist_tags,
            // utility
            commands::utility::get_image_bytes,
            commands::utility::get_cache_stats,
            commands::utility::clear_disk_cache,
            commands::utility::get_minimize_to_tray,
            commands::utility::set_minimize_to_tray,
            commands::utility::get_decorations,
            commands::utility::set_decorations,
            commands::utility::get_volume_normalization,
            commands::utility::set_volume_normalization,
            commands::utility::update_tray_tooltip,
            commands::utility::get_exclusive_mode,
            commands::utility::set_exclusive_mode,
            commands::utility::get_bit_perfect,
            commands::utility::set_bit_perfect,
            commands::utility::get_exclusive_device,
            commands::utility::set_exclusive_device,
            commands::utility::list_audio_devices,
            commands::utility::get_discord_rpc,
            commands::utility::set_discord_rpc,
            commands::utility::get_proxy_settings,
            commands::utility::set_proxy_settings,
            commands::utility::test_proxy_connection,
            commands::utility::inhibit_idle,
            commands::utility::uninhibit_idle,
            commands::utility::get_signal_path,
            commands::utility::get_hw_volume_status,
            // stats
            commands::stats::get_stats_overview,
            commands::stats::get_top_tracks,
            commands::stats::get_top_artists,
            commands::stats::get_top_albums,
            commands::stats::get_listening_heatmap,
            commands::stats::get_daily_minutes,
            commands::playback::prefetch_stream,
            // llm
            commands::llm::get_llm_settings,
            commands::llm::set_llm_settings,
            commands::llm::llm_chat,
            commands::llm::llm_ping,
            // share link
            commands::share::share_start,
            commands::share::share_stop,
            commands::share::share_status,
            commands::share::share_set_state,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<AppState>();
                state.discord.send(crate::discord::DiscordCommand::Disconnect);
                tauri::async_runtime::block_on(async {
                    state.idle_inhibitor.lock().await.uninhibit().await;
                    state.scrobble_manager.flush().await;
                });
            }
        });
}
