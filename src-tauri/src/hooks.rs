//! User-defined shell hooks that fire on playback lifecycle events.
//!
//! Drop an executable script in `~/.config/sone/hooks/<event>` and SONE
//! will run it on the matching event. Track metadata is exposed via
//! environment variables so scripts can integrate with Polybar/Waybar,
//! Hyprland workspaces, custom scrobblers, dimming the lights — whatever.
//!
//! Recognised events:
//! - `on-track-change` — fired when the currently-playing track changes
//!   (excludes pause/resume of the same track).
//! - `on-play`         — fired when playback starts on a fresh track.
//! - `on-pause`        — fired when playback is paused.
//! - `on-resume`       — fired when paused playback resumes.
//! - `on-stop`         — fired when playback stops (queue end, manual stop).
//!
//! Hooks are spawned fire-and-forget; SONE never blocks waiting for them.
//! Stdout/stderr is inherited from the SONE process, so log to a file
//! from the script if you need persistent output.

use std::path::PathBuf;
use std::sync::Mutex;
use tokio::process::Command;

use crate::scrobble::ScrobbleTrack;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    TrackChange,
    Play,
    Pause,
    Resume,
    Stop,
}

impl HookEvent {
    fn filename(self) -> &'static str {
        match self {
            Self::TrackChange => "on-track-change",
            Self::Play => "on-play",
            Self::Pause => "on-pause",
            Self::Resume => "on-resume",
            Self::Stop => "on-stop",
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
        self.spawn(HookEvent::Play, Some(track));

        // on-track-change only when track id actually changed (or first track)
        let changed = match (prev, track.track_id) {
            (None, _) => true,
            (Some(a), Some(b)) => a != b,
            (Some(_), None) => true,
        };
        if changed {
            self.spawn(HookEvent::TrackChange, Some(track));
        }
    }

    pub fn on_pause(&self) {
        self.spawn(HookEvent::Pause, None);
    }

    pub fn on_resume(&self) {
        self.spawn(HookEvent::Resume, None);
    }

    pub fn on_stop(&self) {
        // Clear last track so the next fresh playback fires on-track-change.
        *self.last_track_id.lock().unwrap() = None;
        self.spawn(HookEvent::Stop, None);
    }

    /// Spawn the hook script for `event` if it exists and is executable.
    /// Fire-and-forget — does not wait for completion.
    fn spawn(&self, event: HookEvent, track: Option<&ScrobbleTrack>) {
        let path = self.hooks_dir.join(event.filename());
        if !is_executable(&path) {
            return;
        }

        let mut cmd = Command::new(&path);
        cmd.env("SONE_EVENT", event.filename());

        if let Some(t) = track {
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
