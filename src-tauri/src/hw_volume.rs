//! Out-of-band hardware volume via the DAC's ALSA mixer.
//!
//! Bit-perfect contract — read this before touching anything.
//!
//! This module operates EXCLUSIVELY on the ALSA mixer (the device's
//! control endpoint, separate from the PCM stream). It never reads,
//! writes, or otherwise touches the audio data flowing through the
//! alsa-writer thread in `audio.rs`. In bit-perfect mode this is the
//! ONLY allowed volume path — software sample scaling is dead code
//! enforced by a hard guard at the alsa-writer call site.
//!
//! For DACs that expose a UAC2 volume control (most modern audiophile
//! USB DACs: Hiby R-series, Topping E-series, FiiO K-series, etc.)
//! this maps to the analog gain stage on the DAC itself: bits remain
//! identical, only post-DAC analog level changes.
//!
//! For DACs that don't expose any mixer control, `try_open` returns
//! None; the frontend then locks the volume slider and instructs the
//! user to use the DAC's physical control. Bit-perfect remains intact.

#[cfg(target_os = "linux")]
use alsa::mixer::{Mixer, Selem, SelemChannelId, SelemId};
#[cfg(target_os = "linux")]
use std::sync::Mutex;

#[cfg(target_os = "linux")]
struct Inner {
    mixer: Mixer,
    selem_id: SelemId,
    range_lo: i64,
    range_hi: i64,
}

// SAFETY: alsa-sys's snd_mixer_t is not thread-safe at the C level. We
// wrap every access in a `Mutex<Inner>` and never expose the inner Mixer
// or borrowed Selem reference outside a locked scope, so external
// callers cannot trigger concurrent C-level access. With those
// invariants honoured the wrapper is safely Send + Sync.
#[cfg(target_os = "linux")]
unsafe impl Send for Inner {}
#[cfg(target_os = "linux")]
unsafe impl Sync for Inner {}

#[cfg(target_os = "linux")]
pub struct HwVolume {
    inner: Mutex<Inner>,
    selem_name: String,
}

#[cfg(target_os = "linux")]
impl HwVolume {
    /// Open ALSA mixer for `device` and find a usable playback volume control.
    /// Returns None if the card has no mixer or no playback volume controls.
    ///
    /// `device` is in ALSA syntax — `hw:CARD=R4,DEV=0` or `hw:1,0`. Only the
    /// card portion is relevant for the mixer (DEV is for PCM streams).
    pub fn try_open(device: &str) -> Option<Self> {
        let card = extract_card_arg(device)?;
        let mixer = match Mixer::new(&card, false) {
            Ok(m) => m,
            Err(e) => {
                log::debug!("[hw_volume] Mixer::new({card}) failed: {e}");
                return None;
            }
        };
        let (selem_id, selem_name, range_lo, range_hi) = {
            let selem = find_playback_selem(&mixer)?;
            let id = selem.get_id();
            let (lo, hi) = selem.get_playback_volume_range();
            if hi <= lo {
                log::debug!("[hw_volume] selem has zero range, skipping");
                return None;
            }
            let name = id
                .get_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "?".into());
            (id, name, lo, hi)
        };
        log::info!(
            "[hw_volume] opened mixer for {card}: control={selem_name} range={range_lo}..{range_hi}"
        );
        Some(Self {
            inner: Mutex::new(Inner {
                mixer,
                selem_id,
                range_lo,
                range_hi,
            }),
            selem_name,
        })
    }

    pub fn selem_name(&self) -> &str {
        &self.selem_name
    }

    /// Read current volume in [0..1]. None on transient error.
    pub fn get(&self) -> Option<f32> {
        let inner = self.inner.lock().ok()?;
        // Refresh state — picks up wheel turns / external mixer writes.
        let _ = inner.mixer.handle_events();
        let selem = inner.mixer.find_selem(&inner.selem_id)?;
        let cur = selem.get_playback_volume(SelemChannelId::FrontLeft).ok()?;
        let range = (inner.range_hi - inner.range_lo) as f32;
        if range <= 0.0 {
            return Some(0.0);
        }
        Some(((cur - inner.range_lo) as f32 / range).clamp(0.0, 1.0))
    }

    /// Write volume `level` in [0..1]. Linearly maps to the device's range.
    /// (UAC2 reports the range in dB internally; the driver exposes a
    /// linearised representation. For audiophile DACs this is fine —
    /// they implement their own perceptual curve in firmware.)
    pub fn set(&self, level: f32) -> Result<(), String> {
        let level = level.clamp(0.0, 1.0);
        let inner = self
            .inner
            .lock()
            .map_err(|_| "Mixer mutex poisoned".to_string())?;
        let selem = inner
            .mixer
            .find_selem(&inner.selem_id)
            .ok_or_else(|| "Mixer control disappeared".to_string())?;
        let scaled =
            inner.range_lo + ((inner.range_hi - inner.range_lo) as f32 * level).round() as i64;
        selem
            .set_playback_volume_all(scaled)
            .map_err(|e| format!("set_playback_volume_all: {e}"))?;
        Ok(())
    }
}

/// Extract `hw:CARD=R4` (or `hw:1`) from `hw:CARD=R4,DEV=0` (or `hw:1,0`).
/// Mixer addressing only uses the card identifier.
#[cfg(target_os = "linux")]
fn extract_card_arg(device: &str) -> Option<String> {
    let rest = device.strip_prefix("hw:")?;
    let card_part = rest.split(',').next()?;
    if card_part.is_empty() {
        return None;
    }
    Some(format!("hw:{card_part}"))
}

/// Heuristic: pick the most likely playback volume control.
/// Order matches what audiophile USB DACs commonly expose under UAC2:
/// `PCM` first (the master playback gain on most UAC2 devices), then
/// generic fallbacks. If none of the named candidates exist, fall back
/// to the first selem with a playback volume.
#[cfg(target_os = "linux")]
fn find_playback_selem(mixer: &Mixer) -> Option<Selem<'_>> {
    const PREFERRED: &[&str] = &[
        "PCM",
        "Master",
        "Speaker",
        "Headphone",
        "Playback",
        "Digital",
    ];
    for name in PREFERRED {
        let id = SelemId::new(name, 0);
        if let Some(s) = mixer.find_selem(&id) {
            if s.has_playback_volume() {
                log::debug!("[hw_volume] using preferred selem '{name}'");
                return Some(s);
            }
        }
    }
    // Fallback: first selem with a usable playback volume.
    for el in mixer.iter() {
        if let Some(s) = Selem::new(el) {
            if s.has_playback_volume() {
                if let Ok(name) = s.get_id().get_name() {
                    log::debug!("[hw_volume] using fallback selem '{name}'");
                }
                return Some(s);
            }
        }
    }
    None
}

/// Spawn a polling thread that mirrors the DAC's physical volume control
/// (e.g. the rotary on a Hiby R4) into a `volume-changed` event whenever
/// it changes from outside SONE. The thread holds an Arc<HwVolume> and
/// stops when the device is closed (open_hw_volume swaps it).
///
/// Bit-perfect-safe: only reads from the ALSA mixer, never writes.
#[cfg(target_os = "linux")]
pub fn spawn_wheel_mirror(
    hw: std::sync::Arc<HwVolume>,
    app: tauri::AppHandle,
    bus: std::sync::Arc<std::sync::atomic::AtomicU64>,
    bus_id: u64,
) {
    use std::sync::atomic::Ordering;
    use tauri::Emitter;

    std::thread::Builder::new()
        .name("hw-wheel-mirror".into())
        .spawn(move || {
            // Threshold: only emit if the change exceeds this fraction.
            // Avoids spam when the rotary jitters between adjacent steps.
            const EPS: f32 = 0.005;
            const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

            let mut last_seen = hw.get().unwrap_or(1.0);
            loop {
                // Stop if a newer mixer (different device) has been opened.
                if bus.load(Ordering::Relaxed) != bus_id {
                    log::debug!("[hw_volume] wheel mirror stopping (bus changed)");
                    return;
                }
                std::thread::sleep(POLL_INTERVAL);
                let cur = match hw.get() {
                    Some(v) => v,
                    None => continue,
                };
                if (cur - last_seen).abs() > EPS {
                    last_seen = cur;
                    let _ = app.emit(
                        "volume-changed",
                        serde_json::json!({
                            "level": cur,
                            "route": "hw",
                            "source": "hw-wheel",
                        }),
                    );
                }
            }
        })
        .ok();
}

// ─── Stub for non-Linux platforms (SONE is Linux-only, but keep the
//     module type-checkable to avoid scattered cfg(target_os) elsewhere). ──

#[cfg(not(target_os = "linux"))]
pub struct HwVolume;

#[cfg(not(target_os = "linux"))]
impl HwVolume {
    pub fn try_open(_device: &str) -> Option<Self> {
        None
    }
    pub fn selem_name(&self) -> &str {
        ""
    }
    pub fn get(&self) -> Option<f32> {
        None
    }
    pub fn set(&self, _level: f32) -> Result<(), String> {
        Err("HW volume only supported on Linux".into())
    }
}
