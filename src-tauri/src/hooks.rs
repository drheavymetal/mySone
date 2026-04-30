//! User-defined shell hooks that fire on playback lifecycle events.
//!
//! Drop an executable script in `~/.config/sone/hooks/<event>` and SONE
//! will run it on the matching event. Track metadata is exposed via
//! environment variables so scripts can integrate with Polybar/Waybar,
//! Hyprland workspaces, custom scrobblers, dimming the lights — whatever.
//!
//! Recognised events:
//! - `on-track-change`  — fired when the currently-playing track changes
//!   (excludes pause/resume of the same track).
//! - `on-play`          — fired when playback starts on a fresh track.
//! - `on-pause`         — fired when playback is paused.
//! - `on-resume`        — fired when paused playback resumes.
//! - `on-stop`          — fired when playback stops (queue end, manual stop).
//! - `on-volume-change` — fired on any volume change (UI slider, MPRIS,
//!   CLI, or the DAC's physical wheel via mirror).
//!
//! Hooks are spawned fire-and-forget; SONE never blocks waiting for them.
//! Stdout/stderr is inherited from the SONE process, so log to a file
//! from the script if you need persistent output.

use std::path::PathBuf;
use std::sync::Mutex;
use tokio::process::Command;

use crate::scrobble::ScrobbleTrack;
use crate::VolumeRoute;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    TrackChange,
    Play,
    Pause,
    Resume,
    Stop,
    VolumeChange,
}

impl HookEvent {
    fn filename(self) -> &'static str {
        match self {
            Self::TrackChange => "on-track-change",
            Self::Play => "on-play",
            Self::Pause => "on-pause",
            Self::Resume => "on-resume",
            Self::Stop => "on-stop",
            Self::VolumeChange => "on-volume-change",
        }
    }
}

pub struct HooksManager {
    hooks_dir: PathBuf,
    /// Last track id we fired `on-track-change` for. Used to dedupe spurious
    /// re-emissions of `track-started` for the same logical track (e.g.
    /// MPRIS metadata refresh, auth refresh mid-playback).
    last_track_id: Mutex<Option<u64>>,
}

impl HooksManager {
    pub fn new(config_dir: &std::path::Path) -> Self {
        Self {
            hooks_dir: config_dir.join("hooks"),
            last_track_id: Mutex::new(None),
        }
    }

    pub fn on_track_started(&self, track: &ScrobbleTrack) {
        let prev = {
            let mut guard = self.last_track_id.lock().unwrap();
            let prev = *guard;
            *guard = track.track_id;
            prev
        };

        // on-play fires for every fresh playback
        self.spawn(HookEvent::Play, |cmd| apply_track_env(cmd, track));

        // on-track-change only when track id actually changed (or first track)
        let changed = match (prev, track.track_id) {
            (None, _) => true,
            (Some(a), Some(b)) => a != b,
            (Some(_), None) => true,
        };
        if changed {
            self.spawn(HookEvent::TrackChange, |cmd| apply_track_env(cmd, track));
        }
    }

    pub fn on_pause(&self) {
        self.spawn(HookEvent::Pause, |_| {});
    }

    pub fn on_resume(&self) {
        self.spawn(HookEvent::Resume, |_| {});
    }

    pub fn on_stop(&self) {
        // Clear last track so the next fresh playback fires on-track-change.
        *self.last_track_id.lock().unwrap() = None;
        self.spawn(HookEvent::Stop, |_| {});
    }

    /// Fired on any volume change — UI slider, MPRIS, CLI, or DAC wheel mirror.
    /// Exposes the level (0..1), the routing (`hw`/`sw`/`locked`/`gst`), and
    /// the active ALSA device when applicable, so user scripts can react
    /// (notify, log, drive smart bulbs, whatever).
    pub fn on_volume_change(&self, level: f32, route: VolumeRoute, device: Option<&str>) {
        let level_str = format!("{:.4}", level.clamp(0.0, 1.0));
        let route_str = match route {
            VolumeRoute::Hw => "hw",
            VolumeRoute::Sw => "sw",
            VolumeRoute::Locked => "locked",
            VolumeRoute::Gst => "gst",
        };
        let device = device.map(|s| s.to_string());
        self.spawn(HookEvent::VolumeChange, move |cmd| {
            cmd.env("SONE_VOLUME", &level_str);
            cmd.env("SONE_VOLUME_ROUTE", route_str);
            if let Some(ref d) = device {
                cmd.env("SONE_DEVICE", d);
            }
        });
    }

    /// Spawn the hook script for `event` if it exists and is executable.
    /// Fire-and-forget — does not wait for completion. The closure
    /// receives the partially-built Command so callers can apply
    /// event-specific env vars without forcing a single payload type.
    fn spawn(&self, event: HookEvent, with_env: impl FnOnce(&mut Command)) {
        let path = self.hooks_dir.join(event.filename());
        if !is_executable(&path) {
            return;
        }

        let mut cmd = Command::new(&path);
        cmd.env("SONE_EVENT", event.filename());
        with_env(&mut cmd);

        // Detach: don't keep stdio handles open, don't await.
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        match cmd.spawn() {
            Ok(mut child) => {
                let event_name = event.filename();
                tokio::spawn(async move {
                    match child.wait().await {
                        Ok(status) if status.success() => {
                            log::debug!("[hooks] {event_name} exited 0");
                        }
                        Ok(status) => {
                            log::warn!("[hooks] {event_name} exited {status}");
                        }
                        Err(e) => {
                            log::warn!("[hooks] {event_name} wait error: {e}");
                        }
                    }
                });
            }
            Err(e) => {
                log::warn!("[hooks] spawn {} failed: {e}", event.filename());
            }
        }
    }
}

/// Apply the standard track-related env vars to a hook command.
fn apply_track_env(cmd: &mut Command, t: &ScrobbleTrack) {
    cmd.env("SONE_TRACK_TITLE", &t.track);
    cmd.env("SONE_TRACK_ARTIST", &t.artist);
    cmd.env("SONE_TRACK_DURATION", t.duration_secs.to_string());
    if let Some(ref album) = t.album {
        cmd.env("SONE_TRACK_ALBUM", album);
    }
    if let Some(ref aa) = t.album_artist {
        cmd.env("SONE_TRACK_ALBUM_ARTIST", aa);
    }
    if let Some(id) = t.track_id {
        cmd.env("SONE_TRACK_ID", id.to_string());
    }
    if let Some(ref isrc) = t.isrc {
        cmd.env("SONE_TRACK_ISRC", isrc);
    }
    if let Some(n) = t.track_number {
        cmd.env("SONE_TRACK_NUMBER", n.to_string());
    }
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(md) if md.is_file() => md.permissions().mode() & 0o111 != 0,
        _ => false,
    }
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}
