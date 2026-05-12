//! Phase 5 (B5.5) — listening-guides reader.
//!
//! Reference: D-022 + CLASSICAL_DESIGN.md §4.6.
//!
//! A *listening guide* is a community-fillable LRC-style text file that
//! lives in the user's config directory:
//!
//!   ~/.config/sone/listening-guides/{work_mbid}.lrc
//!
//! Format: each non-empty line is either
//!   * `[mm:ss.cs] description` — a time-synced cue
//!   * `description` — an untimed line (rendered as a header / note)
//!
//! Decoded shape is exposed to the frontend via the
//! `read_classical_listening_guide` Tauri command. The reader is
//! read-only over the filesystem; missing files produce `Ok(None)`.
//!
//! Bit-perfect contract: zero contact with audio routing.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::SoneError;

/// One time-synced (or untimed) line of the guide. `tsMs` is `None` for
/// header lines that don't carry a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrcLine {
    /// Absolute position from the start of the work, milliseconds.
    /// `None` for unsynchronised lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts_ms: Option<u64>,
    /// Free text — formatted plain in V1; markdown rendering is left to
    /// the UI (Phase 6+).
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrcGuide {
    pub work_mbid: String,
    pub lines: Vec<LrcLine>,
}

/// Where the guides live on disk. Resolved via `dirs::config_dir`
/// fallback to the env var so tests can override.
fn guide_path(work_mbid: &str) -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let mut path = base;
    path.push("sone");
    path.push("listening-guides");
    path.push(format!("{work_mbid}.lrc"));
    Some(path)
}

/// Read the guide for a work. Returns `Ok(None)` when the file does not
/// exist (the typical case — guides are opt-in).
pub fn read_guide(work_mbid: &str) -> Result<Option<LrcGuide>, SoneError> {
    if work_mbid.is_empty() {
        return Err(SoneError::Parse("empty work mbid".into()));
    }
    let path = match guide_path(work_mbid) {
        Some(p) => p,
        None => {
            return Err(SoneError::Parse("config_dir unavailable".into()));
        }
    };
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| {
        SoneError::Network(format!("read guide {}: {e}", path.display()))
    })?;
    Ok(Some(parse_lrc(work_mbid, &content)))
}

/// Parse LRC text. Tolerant: ignores blank lines, recognises
/// `[mm:ss.cs]`, `[mm:ss]`, and `[hh:mm:ss.cs]` prefixes.
pub fn parse_lrc(work_mbid: &str, raw: &str) -> LrcGuide {
    let mut lines: Vec<LrcLine> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(parsed) = parse_lrc_line(trimmed) {
            lines.push(parsed);
        }
    }
    LrcGuide {
        work_mbid: work_mbid.to_string(),
        lines,
    }
}

fn parse_lrc_line(line: &str) -> Option<LrcLine> {
    if !line.starts_with('[') {
        return Some(LrcLine {
            ts_ms: None,
            text: line.to_string(),
        });
    }
    let close = line.find(']')?;
    let ts_str = &line[1..close];
    let text = line[close + 1..].trim().to_string();
    let ts_ms = parse_lrc_timestamp(ts_str);
    Some(LrcLine { ts_ms, text })
}

/// Parse `mm:ss.cs`, `mm:ss`, or `hh:mm:ss.cs`. Returns `None` for
/// unparseable timestamps so the caller can skip them.
fn parse_lrc_timestamp(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, s) = match parts.len() {
        2 => (0u64, parts[0].parse::<u64>().ok()?, parts[1]),
        3 => (
            parts[0].parse::<u64>().ok()?,
            parts[1].parse::<u64>().ok()?,
            parts[2],
        ),
        _ => return None,
    };
    let (sec, cs) = match s.split_once('.') {
        Some((secs, cs)) => {
            let cs_value: u64 = cs.parse::<u64>().ok()?;
            // Tolerate cs / ms — if the fractional part has 3 digits we
            // treat it as ms, else as centiseconds.
            let frac_ms = if cs.len() == 3 { cs_value } else { cs_value * 10 };
            (secs.parse::<u64>().ok()?, frac_ms)
        }
        None => (s.parse::<u64>().ok()?, 0),
    };
    Some(h * 3_600_000 + m * 60_000 + sec * 1_000 + cs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_lrc() {
        let raw = "\
[00:00.00] Allegro
[00:30.50] First subject enters
[01:00] Development
";
        let guide = parse_lrc("work-1", raw);
        assert_eq!(guide.lines.len(), 3);
        assert_eq!(guide.lines[0].ts_ms, Some(0));
        assert_eq!(guide.lines[1].ts_ms, Some(30_500));
        assert_eq!(guide.lines[2].ts_ms, Some(60_000));
        assert_eq!(guide.lines[0].text, "Allegro");
    }

    #[test]
    fn parse_handles_hours() {
        let line = parse_lrc_line("[01:23:45.50] Wagner Act II ends").unwrap();
        // 1h 23m 45.5s = 5025500 ms
        assert_eq!(line.ts_ms, Some(5_025_500));
    }

    #[test]
    fn parse_handles_milliseconds() {
        // 3-digit fractional → milliseconds, not centiseconds.
        let line = parse_lrc_line("[00:30.250] Quarter past").unwrap();
        assert_eq!(line.ts_ms, Some(30_250));
    }

    #[test]
    fn parse_untimed_line() {
        let line = parse_lrc_line("Untimed header").unwrap();
        assert_eq!(line.ts_ms, None);
        assert_eq!(line.text, "Untimed header");
    }

    #[test]
    fn parse_skips_blank_lines() {
        let raw = "\n\n[00:01.00] Hello\n\n\n";
        let guide = parse_lrc("work-2", raw);
        assert_eq!(guide.lines.len(), 1);
    }

    #[test]
    fn parse_empty_input() {
        let guide = parse_lrc("work-3", "");
        assert!(guide.lines.is_empty());
    }
}
