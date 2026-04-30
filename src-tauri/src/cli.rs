//! Lightweight CLI for controlling a running SONE instance.
//!
//! When `sone` is invoked with a known verb (`pause`, `next`, `status`, …),
//! `main()` dispatches here BEFORE Tauri starts, so we don't spawn a second
//! GUI instance. The CLI talks to the running app over its existing MPRIS
//! D-Bus interface — no new IPC channel needed.
//!
//! Returns an exit code: 0 success, 1 error, 2 SONE not running.
//!
//! Designed to be drop-in for window-manager status bars (Polybar, Waybar,
//! eww) and for shell aliases. The `status --json` form emits parseable
//! output suitable for piping into `jq`.

use std::collections::HashMap;
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::blocking::Proxy;
use zbus::zvariant::OwnedValue;

const BUS_NAME: &str = "org.mpris.MediaPlayer2.io.github.lullabyX.sone";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";

/// Verbs handled by the CLI. Anything not in this list falls through
/// to the regular Tauri startup (so `tidal://...` deep links still work).
const CLI_VERBS: &[&str] = &[
    "play",
    "pause",
    "toggle",
    "play-pause",
    "next",
    "prev",
    "previous",
    "stop",
    "status",
    "vol",
    "help",
    "--help",
    "-h",
];

/// True if `arg` should be handled by the CLI (vs. forwarded to Tauri).
pub fn is_cli_command(arg: &str) -> bool {
    if arg.contains("://") {
        return false; // deep link — let Tauri handle it
    }
    CLI_VERBS.contains(&arg)
}

/// Dispatch a CLI invocation. `args` is the full argv (program name at [0]).
pub fn run(args: &[String]) -> i32 {
    if args.len() < 2 {
        return print_help();
    }
    let verb = args[1].as_str();
    match verb {
        "help" | "--help" | "-h" => print_help(),
        "play" => transport(|p| p.call_method("Play", &()).map(|_| ())),
        "pause" => transport(|p| p.call_method("Pause", &()).map(|_| ())),
        "toggle" | "play-pause" => transport(|p| p.call_method("PlayPause", &()).map(|_| ())),
        "next" => transport(|p| p.call_method("Next", &()).map(|_| ())),
        "prev" | "previous" => transport(|p| p.call_method("Previous", &()).map(|_| ())),
        "stop" => transport(|p| p.call_method("Stop", &()).map(|_| ())),
        "status" => {
            let json = args.iter().any(|a| a == "--json");
            run_status(json)
        }
        "vol" => run_vol(&args[2..]),
        _ => {
            eprintln!("sone: unknown command '{verb}'");
            print_help();
            1
        }
    }
}

fn print_help() -> i32 {
    println!(
        "{}",
        r"SONE — control a running instance from the terminal.

USAGE:
    sone [COMMAND] [FLAGS]

COMMANDS:
    play              Resume playback
    pause             Pause playback
    toggle            Toggle play/pause (alias: play-pause)
    next              Skip to next track
    prev              Skip to previous track (alias: previous)
    stop              Stop playback
    status            Print current track and playback state
                      --json   emit machine-readable JSON
    vol <ARG>         Adjust DAC hardware volume (bit-perfect safe)
                      +N    raise by N percent
                      -N    lower by N percent
                      N     set to N percent (0..100)
                      mute  set to 0
                      get   print current level
                      Routed through MPRIS when SONE is running, else
                      directly via the ALSA mixer of the configured
                      exclusive device. Never touches the PCM stream.
    help              Show this message (alias: --help, -h)

With no arguments, SONE launches the desktop application as usual.

EXIT CODES:
    0  success
    1  command failed
    2  SONE is not running (transport / status only)"
    );
    0
}

/// Run a transport command (Play/Pause/Next/etc.) — anything that is a
/// fire-and-forget MPRIS method call returning no useful data.
fn transport<F>(op: F) -> i32
where
    F: FnOnce(&Proxy) -> zbus::Result<()>,
{
    match with_player(|p| op(p)) {
        Ok(()) => 0,
        Err(CliError::NotRunning) => {
            eprintln!("sone: not running");
            2
        }
        Err(CliError::Bus(e)) => {
            eprintln!("sone: {e}");
            1
        }
    }
}

fn run_status(json: bool) -> i32 {
    match with_player(read_status) {
        Ok(s) => {
            if json {
                println!("{}", s.to_json());
            } else {
                println!("{}", s.to_human());
            }
            0
        }
        Err(CliError::NotRunning) => {
            if json {
                println!(r#"{{"state":"not-running"}}"#);
            } else {
                eprintln!("sone: not running");
            }
            2
        }
        Err(CliError::Bus(e)) => {
            eprintln!("sone: {e}");
            1
        }
    }
}

#[derive(Debug)]
enum CliError {
    NotRunning,
    Bus(String),
}

impl From<zbus::Error> for CliError {
    fn from(e: zbus::Error) -> Self {
        // ServiceUnknown / NameHasNoOwner mean the bus name is unowned —
        // which means SONE is not running. Distinguish from generic errors.
        let msg = e.to_string();
        if msg.contains("not provided by any .service files")
            || msg.contains("ServiceUnknown")
            || msg.contains("NameHasNoOwner")
            || msg.contains("was not provided")
        {
            return CliError::NotRunning;
        }
        CliError::Bus(msg)
    }
}

fn with_player<R, F>(op: F) -> Result<R, CliError>
where
    F: FnOnce(&Proxy) -> zbus::Result<R>,
{
    let connection = Connection::session().map_err(CliError::from)?;
    let proxy =
        Proxy::new(&connection, BUS_NAME, OBJECT_PATH, PLAYER_IFACE).map_err(CliError::from)?;
    op(&proxy).map_err(CliError::from)
}

#[derive(Debug, Default)]
struct Status {
    state: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_secs: Option<f64>,
    position_secs: Option<f64>,
}

impl Status {
    fn to_human(&self) -> String {
        let label = match self.state.as_str() {
            "Playing" => "▶",
            "Paused" => "⏸",
            "Stopped" => "⏹",
            _ => "?",
        };
        let title = self.title.as_deref().unwrap_or("(unknown)");
        let artist = self.artist.as_deref().unwrap_or("");
        let mut line = format!("{label}  {title}");
        if !artist.is_empty() {
            line.push_str(" — ");
            line.push_str(artist);
        }
        if let (Some(pos), Some(dur)) = (self.position_secs, self.duration_secs) {
            line.push_str(&format!("  [{} / {}]", fmt_secs(pos), fmt_secs(dur)));
        }
        line
    }

    fn to_json(&self) -> String {
        // Hand-rolled JSON to avoid pulling serde_json into a tiny CLI path
        // when the rest of the app already depends on it. Keep the shape
        // stable — scripts will rely on it.
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let mut parts: Vec<String> = vec![format!("\"state\":\"{}\"", esc(&self.state))];
        if let Some(ref t) = self.title {
            parts.push(format!("\"title\":\"{}\"", esc(t)));
        }
        if let Some(ref a) = self.artist {
            parts.push(format!("\"artist\":\"{}\"", esc(a)));
        }
        if let Some(ref al) = self.album {
            parts.push(format!("\"album\":\"{}\"", esc(al)));
        }
        if let Some(d) = self.duration_secs {
            parts.push(format!("\"durationSecs\":{d:.3}"));
        }
        if let Some(p) = self.position_secs {
            parts.push(format!("\"positionSecs\":{p:.3}"));
        }
        format!("{{{}}}", parts.join(","))
    }
}

fn fmt_secs(s: f64) -> String {
    let total = s.max(0.0) as u64;
    let m = total / 60;
    let r = total % 60;
    format!("{m}:{r:02}")
}

fn read_status(p: &Proxy) -> zbus::Result<Status> {
    let mut st = Status::default();

    let state: String = p.get_property("PlaybackStatus")?;
    st.state = state;

    if let Ok(meta) = p.get_property::<HashMap<String, OwnedValue>>("Metadata") {
        st.title = read_str(&meta, "xesam:title");
        st.album = read_str(&meta, "xesam:album");
        st.artist =
            read_str(&meta, "xesam:artist").or_else(|| read_str_array(&meta, "xesam:artist"));

        if let Some(v) = meta.get("mpris:length") {
            if let Ok(usecs) = v.try_clone().and_then(i64::try_from) {
                st.duration_secs = Some(usecs as f64 / 1_000_000.0);
            } else if let Ok(usecs) = v.try_clone().and_then(u64::try_from) {
                st.duration_secs = Some(usecs as f64 / 1_000_000.0);
            }
        }
    }

    if let Ok(pos_us) = p.get_property::<i64>("Position") {
        st.position_secs = Some(pos_us as f64 / 1_000_000.0);
    }

    // Brief drain to ensure the proxy has settled (otherwise zbus may not
    // have read the reply yet). Cheap on success.
    std::thread::sleep(Duration::from_millis(0));

    Ok(st)
}

fn read_str(meta: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let v = meta.get(key)?;
    let cloned = v.try_clone().ok()?;
    String::try_from(cloned).ok()
}

fn read_str_array(meta: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let v = meta.get(key)?;
    let cloned = v.try_clone().ok()?;
    let arr: Vec<String> = Vec::try_from(cloned).ok()?;
    if arr.is_empty() {
        None
    } else {
        Some(arr.join(", "))
    }
}

// ─── Volume subcommand ──────────────────────────────────────────────────
//
// Bit-perfect contract: this code path NEVER touches the PCM stream.
// It writes only to the ALSA mixer of the active exclusive device, via
// the running SONE instance's MPRIS interface (preferred — keeps a single
// source of truth + fires hooks + emits volume-changed events) or, if no
// instance is running, opens the mixer directly with crate::hw_volume so
// keybinds work even when the GUI is closed.

fn run_vol(args: &[String]) -> i32 {
    let arg = match args.first() {
        Some(a) => a.as_str(),
        None => {
            eprintln!("sone: vol requires an argument (+N | -N | N | mute | get)");
            return 1;
        }
    };

    if arg == "get" {
        return run_vol_get();
    }

    let target = match parse_vol_arg(arg) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("sone: {e}");
            return 1;
        }
    };

    // Prefer MPRIS path — it goes through the VolumeRouter which honours
    // the bit-perfect contract and fires hooks + UI events.
    match with_player(|p| {
        let level = match target {
            VolTarget::Set(v) => v,
            VolTarget::Delta(d) => {
                let cur: f64 = p.get_property("Volume").unwrap_or(0.0);
                (cur + d).clamp(0.0, 1.0)
            }
            VolTarget::Mute => 0.0,
        };
        p.set_property("Volume", level)?;
        Ok(level)
    }) {
        Ok(level) => {
            println!("vol: {}%", (level * 100.0).round() as i32);
            0
        }
        Err(CliError::NotRunning) => run_vol_direct(target),
        Err(CliError::Bus(e)) => {
            eprintln!("sone: {e}");
            1
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum VolTarget {
    Set(f64),
    Delta(f64),
    Mute,
}

fn parse_vol_arg(s: &str) -> Result<VolTarget, String> {
    if s == "mute" {
        return Ok(VolTarget::Mute);
    }
    if let Some(rest) = s.strip_prefix('+') {
        let n: f64 = rest.parse().map_err(|_| format!("bad delta: {s}"))?;
        return Ok(VolTarget::Delta(n / 100.0));
    }
    if let Some(rest) = s.strip_prefix('-') {
        let n: f64 = rest.parse().map_err(|_| format!("bad delta: {s}"))?;
        return Ok(VolTarget::Delta(-n / 100.0));
    }
    let n: f64 = s.parse().map_err(|_| format!("bad volume: {s}"))?;
    Ok(VolTarget::Set((n / 100.0).clamp(0.0, 1.0)))
}

fn run_vol_get() -> i32 {
    match with_player(|p| {
        let v: f64 = p.get_property("Volume")?;
        Ok(v)
    }) {
        Ok(v) => {
            println!("{}%", (v * 100.0).round() as i32);
            0
        }
        Err(CliError::NotRunning) => match read_hw_volume_offline() {
            Ok(Some(v)) => {
                println!("{}%", (v * 100.0).round() as i32);
                0
            }
            Ok(None) => {
                eprintln!("sone: not running and no HW volume available offline");
                2
            }
            Err(e) => {
                eprintln!("sone: {e}");
                1
            }
        },
        Err(CliError::Bus(e)) => {
            eprintln!("sone: {e}");
            1
        }
    }
}

/// Direct ALSA-mixer path for when the GUI is closed. Reads the
/// configured exclusive device from settings.json (transparently decrypted
/// using the same crypto subsystem the GUI uses) and operates on it.
/// Bit-perfect-safe — only the mixer is touched.
fn run_vol_direct(target: VolTarget) -> i32 {
    let level = match resolve_offline(target) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("sone: {e}");
            return 1;
        }
    };
    let hw = match open_offline_mixer() {
        Ok(hw) => hw,
        Err(e) => {
            eprintln!("sone: {e}");
            return 1;
        }
    };
    if let Err(e) = hw.set(level as f32) {
        eprintln!("sone: {e}");
        return 1;
    }
    println!(
        "vol: {}% (offline, direct mixer)",
        (level * 100.0).round() as i32
    );
    0
}

fn resolve_offline(target: VolTarget) -> Result<f64, String> {
    Ok(match target {
        VolTarget::Set(v) => v,
        VolTarget::Mute => 0.0,
        VolTarget::Delta(d) => {
            let hw = open_offline_mixer()?;
            let cur = hw.get().unwrap_or(0.0) as f64;
            (cur + d).clamp(0.0, 1.0)
        }
    })
}

fn read_hw_volume_offline() -> Result<Option<f32>, String> {
    let hw = open_offline_mixer()?;
    Ok(hw.get())
}

fn open_offline_mixer() -> Result<crate::hw_volume::HwVolume, String> {
    let device = read_exclusive_device_from_settings()?;
    crate::hw_volume::HwVolume::try_open(&device)
        .ok_or_else(|| format!("device {device} has no usable HW volume control"))
}

/// Read `exclusive_device` from `~/.config/sone/settings.json`,
/// transparently decrypting via the same Crypto module the GUI uses.
fn read_exclusive_device_from_settings() -> Result<String, String> {
    let mut config_dir = dirs::config_dir().ok_or("no config dir")?;
    config_dir.push("sone");
    let settings_path = config_dir.join("settings.json");
    let raw = std::fs::read(&settings_path).map_err(|e| format!("read settings.json: {e}"))?;
    let crypto =
        crate::crypto::Crypto::new(&config_dir).map_err(|e| format!("crypto init: {e}"))?;
    let plain = crypto
        .decrypt(&raw)
        .map_err(|e| format!("decrypt settings: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_slice(&plain).map_err(|e| format!("parse settings: {e}"))?;
    v.get("exclusive_device")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "no exclusive_device set in settings".to_string())
}
