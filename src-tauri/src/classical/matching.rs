//! D-010 cascade matching: ISRC → text-search with confidence scoring.
//!
//! Phase 0 spike showed MB has ISRCs for only ~15% of canonical
//! recordings. Tidal has the catalogue at ~100% — we just need a second
//! path that doesn't depend on ISRC. This module implements the cascade
//! and the scoring model described in `phase-1-foundation.md` §B2.
//!
//! Scoring weights (sum to 1.0):
//!   * artist substring match  → 0.40
//!   * work title substring    → 0.30
//!   * year ±2 (when known)    → 0.20
//!   * duration ±10% (when known) → 0.10
//!
//! A top hit needs ≥ 0.6 to be classified as `TextSearchInferred`. Below
//! 0.6 → `NotFound`. The threshold is intentionally conservative for
//! Phase 1; QA in Phase 4 may relax it for long-tail repertoire.
//!
//! Movement penalty: titles that look like a single movement
//! ("I.", "II.", roman numerals, "Andante", "Allegro" alone) get a
//! −0.25 penalty. Tidal often returns movements first when the search
//! query is just "Mahler 9 Bernstein"; we want the album-level track,
//! not the third movement.

use crate::tidal_api::TidalTrack;

use super::buckets::{bucket_from_album_title, buckets_compatible};
use super::types::{MatchConfidence, Recording, WorkBucket};

/// Threshold above which a text-search hit becomes
/// `TextSearchInferred`. Anything strictly less is `NotFound`.
pub const INFERRED_THRESHOLD: f64 = 0.6;

/// D-037 / D-041 (Phase 8.9) — threshold for the work-level fallback.
/// Originally 0.55 to catch clean composer + title hits. D-041 raises
/// it to 0.62 for two reasons. First, `build_canonical_query` now
/// appends the catalogue number ("Op. 83", "BWV 244", "K. 466") so
/// clean queries score above 0.65 with the catalogue token
/// contributing to title overlap. Second, the genre-aware penalty
/// (`GENRE_BUCKET_PENALTY`) cuts cross-bucket false positives (lieder
/// query matching a symphony album) by 0.30, which makes the lower
/// 0.55 floor too permissive. The result is a tighter band that
/// drops the Beethoven Op. 83 → Eroica false-positive (which used to
/// score 0.775) once the catalogue and genre signals fire.
pub const WORK_LEVEL_THRESHOLD: f64 = 0.62;

/// Maximum number of synthetic recordings produced by a single
/// work-level fallback (D-041 / A1). The fallback used to take only
/// the top-1 hit; D-041 broadens that to a cap-N list because a
/// well-formed canonical query (composer + title + catalogue) returns
/// 8 candidates that are consistently parents/children of the same
/// work — typically different conductors of the same lied or
/// symphony. Cap is conservative to keep the WorkPage rendering
/// reasonable; further versions of the same work surface through the
/// cascade once MB exposes recording-rels.
pub const MAX_WORK_LEVEL_SYNTH: usize = 12;

const MOVEMENT_PENALTY: f64 = 0.25;

/// D-041 (A3) — penalty applied when the candidate album's inferred
/// kind is incompatible with the work's expected bucket (e.g. a
/// "Symphonies Nos. 1-9" album candidate against a Vocal work).
/// Calibrated so a clean title hit minus this penalty (1.00 − 0.30 =
/// 0.70) still crosses the per-recording threshold (0.60) but a
/// catalogue-less, partial-overlap match (~0.65) drops below the
/// work-level threshold (0.62).
///
/// Phase 9 refactor (B9.1): the inference and the compatibility
/// matrix moved to `classical::buckets` and now operate on
/// `WorkBucket × WorkBucket`. The matcher just calls them.
const GENRE_BUCKET_PENALTY: f64 = 0.30;

/// Outcome of a matching attempt.
#[derive(Debug, Clone)]
pub struct MatchOutcome {
    pub track_id: Option<u64>,
    pub album_id: Option<u64>,
    pub quality_tags: Vec<String>,
    pub audio_modes: Vec<String>,
    pub duration_secs: Option<u32>,
    pub cover_url: Option<String>,
    pub confidence: MatchConfidence,
    pub query_used: Option<String>,
    pub score: Option<f64>,
}

impl MatchOutcome {
    pub fn not_found() -> Self {
        Self {
            track_id: None,
            album_id: None,
            quality_tags: Vec::new(),
            audio_modes: Vec::new(),
            duration_secs: None,
            cover_url: None,
            confidence: MatchConfidence::NotFound,
            query_used: None,
            score: None,
        }
    }
}

/// Apply a `MatchOutcome` onto a `Recording` shell. Mutates in-place.
pub fn apply_outcome(recording: &mut Recording, outcome: MatchOutcome) {
    recording.tidal_track_id = outcome.track_id;
    recording.tidal_album_id = outcome.album_id;
    recording.audio_quality_tags = outcome.quality_tags;
    recording.audio_modes = outcome.audio_modes;
    if recording.duration_secs.is_none() {
        recording.duration_secs = outcome.duration_secs;
    }
    if recording.cover_url.is_none() {
        recording.cover_url = outcome.cover_url;
    }
    recording.match_confidence = outcome.confidence;
    recording.match_query = outcome.query_used;
    recording.match_score = outcome.score;
}

/// Score a candidate Tidal track against a recording context.
///
/// `expected_artist`: usually the conductor or a soloist last name.
/// `expected_title`: the work title (stripped of catalogue suffix).
/// `expected_year`: recording year ±2 is full credit.
/// `expected_duration`: ±10% is full credit.
/// `expected_bucket` (D-041 / A3, refactored in B9.1): when known, the
///     work's `WorkBucket` (D-040). If the candidate album's inferred
///     bucket is explicitly incompatible (Vocal ⊥ Symphonies, Chamber
///     ⊥ Stage, etc.) per `buckets::buckets_compatible` the score is
///     penalised by `GENRE_BUCKET_PENALTY`. Albums that yield no
///     bucket signal (`None` from `bucket_from_album_title`) are
///     never penalised — we prefer false-negatives on the penalty
///     path over false-positive bucket matches.
pub fn score_candidate(
    candidate: &TidalTrack,
    expected_artist: Option<&str>,
    expected_title: &str,
    expected_year: Option<i32>,
    expected_duration: Option<u32>,
    expected_bucket: Option<WorkBucket>,
) -> f64 {
    let mut score = 0.0_f64;

    // Artist substring
    if let Some(want) = expected_artist {
        let want_lower = want.to_lowercase();
        let cand_artist = candidate
            .artist
            .as_ref()
            .map(|a| a.name.to_lowercase())
            .or_else(|| {
                candidate.artists.as_ref().and_then(|arr| {
                    arr.first().map(|a| a.name.to_lowercase())
                })
            });
        if let Some(ca) = cand_artist {
            if ca.contains(&want_lower) || want_lower.contains(&ca) {
                score += 0.40;
            }
        }
    } else {
        // No expected artist → award full weight (caller didn't constrain).
        score += 0.40;
    }

    // Title substring
    let want_title_lower = expected_title.to_lowercase();
    let cand_title_lower = candidate.title.to_lowercase();
    if cand_title_lower.contains(&want_title_lower)
        || want_title_lower.contains(&cand_title_lower)
    {
        score += 0.30;
    } else {
        // Soft credit for partial overlap (any 3-word window in common).
        let overlap = max_word_overlap(&want_title_lower, &cand_title_lower);
        score += 0.30 * (overlap as f64 / 4.0).min(1.0);
    }

    // Year proximity
    if let (Some(want_year), Some(cand_year)) =
        (expected_year, parse_year_from_album_release(candidate))
    {
        let diff = (want_year - cand_year).abs();
        if diff <= 2 {
            score += 0.20;
        } else if diff <= 5 {
            score += 0.10;
        }
    } else if expected_year.is_none() {
        score += 0.20;
    }

    // Duration proximity
    if let (Some(want_dur), cand_dur) = (expected_duration, candidate.duration) {
        if cand_dur > 0 {
            let ratio =
                (cand_dur as f64 - want_dur as f64).abs() / want_dur as f64;
            if ratio <= 0.10 {
                score += 0.10;
            } else if ratio <= 0.20 {
                score += 0.05;
            }
        }
    } else if expected_duration.is_none() {
        score += 0.10;
    }

    // Movement penalty
    if looks_like_movement(&candidate.title) {
        score = (score - MOVEMENT_PENALTY).max(0.0);
    }

    // D-041 (A3) — genre-bucket penalty. Refactored in B9.1 to delegate
    // the album-title inference and the compatibility matrix to
    // `classical::buckets`. Only fires when:
    //   1. Caller supplied a work bucket (the catalog cascade does;
    //      legacy callers without one keep their original behaviour).
    //   2. The album title yields a confident bucket hint.
    //   3. The two are explicitly incompatible per the lattice.
    if let Some(work_bucket) = expected_bucket {
        if let Some(album_bucket) = bucket_from_album_title(candidate) {
            if !buckets_compatible(work_bucket, album_bucket) {
                score = (score - GENRE_BUCKET_PENALTY).max(0.0);
            }
        }
    }

    score.min(1.0)
}

/// Heuristic: title starts with `I.` `II.` `III.` etc., or is a single
/// tempo marking (`Andante`, `Allegro con brio`). These are usually
/// movements within a larger work, not a complete recording.
pub fn looks_like_movement(title: &str) -> bool {
    let trimmed = title.trim();
    let lower = trimmed.to_lowercase();

    // Roman numeral prefix: "I.", "II.", "III.", "IV.", "V.", "VI.", "VII.",
    // "VIII.", "IX.", "X."
    for prefix in [
        "i. ", "ii. ", "iii. ", "iv. ", "v. ", "vi. ", "vii. ", "viii. ", "ix. ",
        "x. ",
    ] {
        if lower.starts_with(prefix) {
            return true;
        }
    }
    // Decimal index prefix: "1.", "2.", ..., "10."
    let bytes = trimmed.as_bytes();
    if !bytes.is_empty() && bytes[0].is_ascii_digit() {
        let mut i = 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'.' {
            return true;
        }
    }

    // Standalone tempo markings.
    const TEMPO_MARKINGS: &[&str] = &[
        "andante", "allegro", "adagio", "presto", "moderato", "largo",
        "vivace", "scherzo", "rondo", "menuetto", "minuet", "molto vivace",
        "allegretto", "lento", "grave", "andantino",
    ];
    for marking in TEMPO_MARKINGS {
        if lower == *marking || lower.starts_with(&format!("{marking} ")) {
            return true;
        }
    }
    false
}

/// Earliest release date year. Tidal's track responses include album
/// releaseDate in `Track.album.releaseDate` (ISO YYYY-MM-DD).
fn parse_year_from_album_release(track: &TidalTrack) -> Option<i32> {
    track
        .album
        .as_ref()
        .and_then(|a| a.release_date.as_deref())
        .and_then(|s| s.get(0..4))
        .and_then(|s| s.parse::<i32>().ok())
}

/// Count overlapping word tokens between two lowercase strings.
fn max_word_overlap(a: &str, b: &str) -> usize {
    let a_words: std::collections::HashSet<&str> =
        a.split_whitespace().filter(|w| w.len() > 2).collect();
    let b_words: std::collections::HashSet<&str> =
        b.split_whitespace().filter(|w| w.len() > 2).collect();
    a_words.intersection(&b_words).count()
}

/// D-037 (bug 3) / D-041 — pick the single best candidate from a
/// *work-level* Tidal search. Kept for callers that want top-1 only;
/// the catalog now uses [`best_work_level_candidates_multiple`] to
/// surface several recordings per work-level fallback.
///
/// Differences vs. [`best_candidate`]:
///
///   * Confidence is `TidalDirectInferred` (not `TextSearchInferred`).
///   * Threshold is `WORK_LEVEL_THRESHOLD` (0.62 since D-041), tighter
///     than the per-recording 0.6 — the catalogue token in the query
///     means clean hits score above 0.65 while crosses-bucket false
///     positives drop below.
///   * Caller has no artist / year / duration constraint, so this helper
///     scores with `expected_artist=None`, `expected_year=None`,
///     `expected_duration=None`. `expected_bucket` is forwarded so
///     the genre-bucket penalty can fire.
///
/// Returns `NotFound` outcome when the top score doesn't cross.
pub fn best_work_level_candidate(
    candidates: &[TidalTrack],
    expected_title: &str,
    expected_bucket: Option<WorkBucket>,
    query_used: String,
) -> MatchOutcome {
    let mut best: Option<(f64, &TidalTrack)> = None;
    for track in candidates.iter() {
        let s = score_candidate(
            track,
            None,
            expected_title,
            None,
            None,
            expected_bucket,
        );
        match best {
            None => best = Some((s, track)),
            Some((bs, _)) if s > bs => best = Some((s, track)),
            _ => {}
        }
    }

    let Some((score, track)) = best else {
        return MatchOutcome::not_found();
    };

    if score < WORK_LEVEL_THRESHOLD {
        let mut nf = MatchOutcome::not_found();
        nf.query_used = Some(query_used);
        nf.score = Some(score);
        return nf;
    }

    let cover = track.album.as_ref().and_then(|a| a.cover.clone());
    let album_id = track.album.as_ref().map(|a| a.id);
    let quality_tags = track
        .media_metadata
        .as_ref()
        .map(|m| m.tags.clone())
        .unwrap_or_default();
    let audio_modes = track.audio_modes.clone().unwrap_or_default();

    MatchOutcome {
        track_id: Some(track.id),
        album_id,
        quality_tags,
        audio_modes,
        duration_secs: Some(track.duration),
        cover_url: cover,
        confidence: MatchConfidence::TidalDirectInferred,
        query_used: Some(query_used),
        score: Some(score),
    }
}

/// D-041 (A1) — pick the top-N candidates from a work-level Tidal
/// search instead of just the top-1. For repertoire where MB has zero
/// recording-rels but Tidal has the catalogue (a common shape — Pedro's
/// telemetry on Beethoven Op. 83 showed 8 candidates, all valid lieder
/// recordings), the top-1 fallback was producing exactly one synthetic
/// Recording when the user expected several alternative readings.
///
/// Returns up to `MAX_WORK_LEVEL_SYNTH` outcomes, each individually
/// scored against `WORK_LEVEL_THRESHOLD` (0.62). The list is sorted by
/// score descending so the WorkPage's default ordering naturally
/// surfaces the strongest matches first; the per-row movement penalty
/// (−0.25) and genre-bucket penalty (−0.30) still apply per candidate
/// — incompatible buckets drop out before reaching the cap.
///
/// `query_used` is duplicated into every outcome so the UI can render
/// the same provenance ("matched via composer + title + Op. 83") on
/// each synthetic row. Caller is responsible for assigning unique
/// synthetic MBIDs (`synthetic:tidal:{work_mbid}:{idx}` per D-041).
pub fn best_work_level_candidates_multiple(
    candidates: &[TidalTrack],
    expected_title: &str,
    expected_bucket: Option<WorkBucket>,
    query_used: String,
) -> Vec<MatchOutcome> {
    // Score every candidate; collect (score, &track) pairs.
    let mut scored: Vec<(f64, &TidalTrack)> = Vec::with_capacity(candidates.len());
    for track in candidates.iter() {
        let s = score_candidate(
            track,
            None,
            expected_title,
            None,
            None,
            expected_bucket,
        );
        scored.push((s, track));
    }
    // Sort by score descending; ties keep input order (stable sort).
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut out: Vec<MatchOutcome> = Vec::new();
    for (score, track) in scored.iter() {
        if *score < WORK_LEVEL_THRESHOLD {
            // Sorted descending — once we drop below threshold, the
            // rest is too low.
            break;
        }
        if out.len() >= MAX_WORK_LEVEL_SYNTH {
            break;
        }
        let cover = track.album.as_ref().and_then(|a| a.cover.clone());
        let album_id = track.album.as_ref().map(|a| a.id);
        let quality_tags = track
            .media_metadata
            .as_ref()
            .map(|m| m.tags.clone())
            .unwrap_or_default();
        let audio_modes = track.audio_modes.clone().unwrap_or_default();

        out.push(MatchOutcome {
            track_id: Some(track.id),
            album_id,
            quality_tags,
            audio_modes,
            duration_secs: Some(track.duration),
            cover_url: cover,
            confidence: MatchConfidence::TidalDirectInferred,
            query_used: Some(query_used.clone()),
            score: Some(*score),
        });
    }
    out
}

/// Pick the best candidate from a Tidal search result. Returns the
/// outcome (which carries the score). Caller decides whether the score
/// crosses `INFERRED_THRESHOLD` before promoting to `TextSearchInferred`.
///
/// `expected_bucket` (D-041 / A3, B9.1 refactor): when known,
/// propagates the genre-bucket penalty into the per-recording
/// cascade. Callers that don't have a bucket pass `None`.
pub fn best_candidate(
    candidates: &[TidalTrack],
    expected_artist: Option<&str>,
    expected_title: &str,
    expected_year: Option<i32>,
    expected_duration: Option<u32>,
    expected_bucket: Option<WorkBucket>,
    query_used: String,
) -> MatchOutcome {
    let mut best: Option<(f64, &TidalTrack)> = None;
    for track in candidates.iter() {
        let s = score_candidate(
            track,
            expected_artist,
            expected_title,
            expected_year,
            expected_duration,
            expected_bucket,
        );
        match best {
            None => best = Some((s, track)),
            Some((bs, _)) if s > bs => best = Some((s, track)),
            _ => {}
        }
    }

    let Some((score, track)) = best else {
        return MatchOutcome::not_found();
    };

    let confidence = if score >= INFERRED_THRESHOLD {
        MatchConfidence::TextSearchInferred
    } else {
        MatchConfidence::NotFound
    };

    let cover = track.album.as_ref().and_then(|a| a.cover.clone());
    let album_id = track.album.as_ref().map(|a| a.id);
    let quality_tags = track
        .media_metadata
        .as_ref()
        .map(|m| m.tags.clone())
        .unwrap_or_default();
    let audio_modes = track.audio_modes.clone().unwrap_or_default();

    MatchOutcome {
        track_id: if matches!(confidence, MatchConfidence::TextSearchInferred) {
            Some(track.id)
        } else {
            None
        },
        album_id: if matches!(confidence, MatchConfidence::TextSearchInferred) {
            album_id
        } else {
            None
        },
        quality_tags: if matches!(confidence, MatchConfidence::TextSearchInferred) {
            quality_tags
        } else {
            Vec::new()
        },
        audio_modes: if matches!(confidence, MatchConfidence::TextSearchInferred) {
            audio_modes
        } else {
            Vec::new()
        },
        duration_secs: if matches!(confidence, MatchConfidence::TextSearchInferred) {
            Some(track.duration)
        } else {
            None
        },
        cover_url: if matches!(confidence, MatchConfidence::TextSearchInferred) {
            cover
        } else {
            None
        },
        confidence,
        query_used: Some(query_used),
        score: Some(score),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tidal_api::{MediaMetadata, TidalAlbum, TidalArtist, TidalTrack};

    fn make_track(
        id: u64,
        title: &str,
        artist_name: &str,
        year: Option<i32>,
        duration: u32,
    ) -> TidalTrack {
        TidalTrack {
            id,
            title: title.to_string(),
            duration,
            version: None,
            artist: Some(TidalArtist {
                id: 0,
                name: artist_name.to_string(),
                picture: None,
                artist_type: None,
                handle: None,
            }),
            artists: None,
            album: Some(TidalAlbum {
                id: 1000 + id,
                title: format!("{title} album"),
                cover: None,
                vibrant_color: None,
                video_cover: None,
                release_date: year.map(|y| format!("{y}-01-01")),
            }),
            audio_quality: None,
            track_number: None,
            volume_number: None,
            date_added: None,
            isrc: None,
            explicit: None,
            popularity: None,
            replay_gain: None,
            peak: None,
            copyright: None,
            url: None,
            stream_ready: None,
            allow_streaming: None,
            premium_streaming_only: None,
            stream_start_date: None,
            audio_modes: Some(vec!["STEREO".into()]),
            media_metadata: Some(MediaMetadata {
                tags: vec!["LOSSLESS".into()],
            }),
            mixes: None,
        }
    }

    #[test]
    fn detects_movement_titles() {
        assert!(looks_like_movement("II. Molto vivace"));
        assert!(looks_like_movement("IV. Presto"));
        assert!(looks_like_movement("3. Adagio molto e cantabile"));
        assert!(looks_like_movement("Andante con moto"));
        assert!(!looks_like_movement("Symphony No. 9"));
        assert!(!looks_like_movement("Goldberg Variations"));
    }

    #[test]
    fn full_match_scores_above_threshold() {
        let t = make_track(1, "Symphony No. 9", "Karajan", Some(1962), 4400);
        let s = score_candidate(
            &t,
            Some("Karajan"),
            "Symphony No. 9",
            Some(1962),
            Some(4400),
            None,
        );
        assert!(s >= 0.95, "expected near-perfect score, got {s}");
    }

    #[test]
    fn movement_hit_penalised_below_threshold() {
        let t = make_track(2, "II. Molto vivace", "Karajan", Some(1962), 700);
        let s = score_candidate(
            &t,
            Some("Karajan"),
            "Symphony No. 9",
            Some(1962),
            Some(4400),
            None,
        );
        // Without movement penalty score would be ~0.4 (artist+year). With
        // penalty of 0.25 it falls below 0.6 threshold.
        assert!(
            s < INFERRED_THRESHOLD,
            "movement should not cross threshold: {s}"
        );
    }

    #[test]
    fn missing_artist_in_query_awards_partial_credit() {
        let t = make_track(3, "Glassworks", "Riesman", None, 2700);
        // Query without artist constraint.
        let s = score_candidate(&t, None, "Glassworks", None, None, None);
        assert!(
            s >= INFERRED_THRESHOLD,
            "no-constraint query should still match: {s}"
        );
    }

    #[test]
    fn wrong_year_drags_score_down() {
        let t = make_track(4, "Symphony No. 9", "Karajan", Some(1990), 4400);
        let s = score_candidate(
            &t,
            Some("Karajan"),
            "Symphony No. 9",
            Some(1962),
            Some(4400),
            None,
        );
        // Year is 28 years off — no year credit (0.2 lost). Score
        // should fall below 0.9 but still possibly above threshold.
        assert!(s < 0.9, "year-mismatch should reduce score: {s}");
    }

    #[test]
    fn best_candidate_picks_highest_score() {
        let movement = make_track(10, "II. Molto vivace", "Karajan", Some(1962), 700);
        let full = make_track(11, "Symphony No. 9", "Karajan", Some(1962), 4400);
        let outcome = best_candidate(
            &[movement, full],
            Some("Karajan"),
            "Symphony No. 9",
            Some(1962),
            Some(4400),
            None,
            "Beethoven Symphony No. 9 Karajan 1962".to_string(),
        );
        assert_eq!(outcome.track_id, Some(11));
        assert!(matches!(
            outcome.confidence,
            MatchConfidence::TextSearchInferred
        ));
    }

    #[test]
    fn empty_candidates_returns_not_found() {
        let outcome = best_candidate(
            &[],
            Some("Karajan"),
            "Symphony No. 9",
            None,
            None,
            None,
            "q".into(),
        );
        assert!(matches!(outcome.confidence, MatchConfidence::NotFound));
        assert!(outcome.track_id.is_none());
    }

    // ---- D-037 / D-041 — work-level fallback ----

    #[test]
    fn work_level_clean_title_match_yields_tidal_direct_inferred() {
        // Clean title hit, no artist constraint, no year, no duration.
        // Score = 0.40 (no-artist credit) + 0.30 (title) + 0.20 (no-year)
        //       + 0.10 (no-duration) = 1.00. Above 0.62 (D-041).
        let track = make_track(20, "Glassworks", "Riesman", None, 2700);
        let outcome = best_work_level_candidate(
            &[track],
            "Glassworks",
            None,
            "Glass Glassworks".to_string(),
        );
        assert!(matches!(
            outcome.confidence,
            MatchConfidence::TidalDirectInferred
        ));
        assert_eq!(outcome.track_id, Some(20));
        assert!(outcome.score.unwrap() >= WORK_LEVEL_THRESHOLD);
        assert_eq!(outcome.query_used.as_deref(), Some("Glass Glassworks"));
    }

    #[test]
    fn work_level_movement_only_falls_below_threshold() {
        // All candidates are movements ("II. Molto vivace") with movement
        // penalty −0.25. Even with no-artist + no-year + no-duration full
        // credit (0.7) + title overlap (variable), penalty drags top below
        // WORK_LEVEL_THRESHOLD.
        let movement = make_track(21, "II. Molto vivace", "Karajan", None, 700);
        let outcome = best_work_level_candidate(
            &[movement],
            "Symphony No. 9",
            None,
            "Beethoven Symphony No. 9".to_string(),
        );
        assert!(matches!(outcome.confidence, MatchConfidence::NotFound));
        assert!(outcome.track_id.is_none());
    }

    #[test]
    fn work_level_empty_candidates_returns_not_found() {
        let outcome = best_work_level_candidate(
            &[],
            "Symphony No. 9",
            None,
            "Beethoven Symphony No. 9".to_string(),
        );
        assert!(matches!(outcome.confidence, MatchConfidence::NotFound));
        assert!(outcome.track_id.is_none());
    }

    #[test]
    fn work_level_picks_clean_over_movement() {
        // Mixed bag: a movement and a clean title. Clean wins.
        let movement = make_track(22, "II. Molto vivace", "Karajan", None, 700);
        let clean = make_track(23, "Symphony No. 9", "Karajan", None, 4400);
        let outcome = best_work_level_candidate(
            &[movement, clean],
            "Symphony No. 9",
            None,
            "Beethoven Symphony No. 9".to_string(),
        );
        assert!(matches!(
            outcome.confidence,
            MatchConfidence::TidalDirectInferred
        ));
        assert_eq!(outcome.track_id, Some(23));
    }

    // ---- D-041 (A1) — top-N work-level synthesis ----

    /// Build a track whose album title carries a distinct
    /// performer-artist combination. Used by the multi-fallback tests
    /// to simulate "8 different lieder recordings" from one canonical
    /// query.
    fn make_lieder_track(id: u64, performer: &str) -> TidalTrack {
        let mut t = make_track(id, "3 Gesänge von Goethe, Op. 83", performer, None, 600);
        // Tag the album as a lieder collection to keep the genre
        // hint Vocal — compatible with WorkBucket::Vocal.
        if let Some(ref mut album) = t.album {
            album.title = format!("Beethoven Lieder — {performer}");
        }
        t
    }

    #[test]
    fn work_level_multiple_returns_topn_for_lieder_canon() {
        // 8 distinct vocal recordings of Op. 83. With WorkBucket::Vocal
        // expected, all of them are bucket-compatible (no penalty)
        // and all carry strong title overlap → above threshold.
        let candidates: Vec<TidalTrack> = [
            "Schreier",
            "Fischer-Dieskau",
            "Prey",
            "Bostridge",
            "Goerne",
            "Pregardien",
            "Hampson",
            "Quasthoff",
        ]
        .iter()
        .enumerate()
        .map(|(i, name)| make_lieder_track(100 + i as u64, name))
        .collect();

        let outcomes = best_work_level_candidates_multiple(
            &candidates,
            "3 Gesänge von Goethe",
            Some(WorkBucket::Vocal),
            "Beethoven 3 Gesänge von Goethe Op. 83".to_string(),
        );

        assert!(
            outcomes.len() >= 5,
            "expected at least 5 synthetic outcomes, got {}",
            outcomes.len()
        );
        // Every outcome must be TidalDirectInferred and carry the
        // canonical query unchanged.
        for o in outcomes.iter() {
            assert!(matches!(o.confidence, MatchConfidence::TidalDirectInferred));
            assert_eq!(
                o.query_used.as_deref(),
                Some("Beethoven 3 Gesänge von Goethe Op. 83"),
            );
        }
        // Sorted by score desc — strictly non-increasing.
        for win in outcomes.windows(2) {
            let a = win[0].score.unwrap_or(0.0);
            let b = win[1].score.unwrap_or(0.0);
            assert!(a >= b, "outcomes not sorted: {a} then {b}");
        }
    }

    #[test]
    fn work_level_multiple_filters_movements_keeps_parents() {
        // 4 movements + 4 parent-level recordings. After movement
        // penalty (−0.25) the movements drop below threshold; only
        // the 4 clean ones survive.
        let movements: Vec<TidalTrack> = [
            "I. Allegro",
            "II. Molto vivace",
            "III. Adagio",
            "IV. Presto",
        ]
        .iter()
        .enumerate()
        .map(|(i, t)| make_track(200 + i as u64, t, "Karajan", None, 600))
        .collect();
        let parents: Vec<TidalTrack> = [
            "Symphony No. 9",
            "Symphony No. 9 (Choral)",
            "Symphony No. 9 in D minor",
            "Symphony No. 9, Op. 125",
        ]
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut tr = make_track(300 + i as u64, t, "Karajan", None, 4400);
            if let Some(ref mut album) = tr.album {
                album.title = format!("{t} - Symphonies").to_string();
            }
            tr
        })
        .collect();

        let mut all = movements;
        all.extend(parents);

        let outcomes = best_work_level_candidates_multiple(
            &all,
            "Symphony No. 9",
            Some(WorkBucket::Symphonies),
            "Beethoven Symphony No. 9 Op. 125".to_string(),
        );
        assert_eq!(
            outcomes.len(),
            4,
            "expected exactly the 4 parent recordings to survive, got {}",
            outcomes.len()
        );
        for o in outcomes.iter() {
            // None of the surviving track ids belong to the movement
            // batch (200..204).
            let id = o.track_id.unwrap_or(0);
            assert!(
                !(200..204).contains(&id),
                "movement track {id} should have been dropped"
            );
        }
    }

    #[test]
    fn work_level_multiple_empty_below_threshold() {
        // All candidates are movements → all below threshold → empty
        // outcome list (NOT a NotFound — caller treats empty as
        // "leave fallback as no-op").
        let movements: Vec<TidalTrack> = [
            "I. Allegro",
            "II. Andante",
            "III. Scherzo",
        ]
        .iter()
        .enumerate()
        .map(|(i, t)| make_track(400 + i as u64, t, "Karajan", None, 600))
        .collect();

        let outcomes = best_work_level_candidates_multiple(
            &movements,
            "Symphony No. 9",
            Some(WorkBucket::Symphonies),
            "q".to_string(),
        );
        assert!(outcomes.is_empty(), "expected no synth, got {}", outcomes.len());
    }

    #[test]
    fn work_level_multiple_caps_at_max_synth() {
        // 20 strong matches → capped to MAX_WORK_LEVEL_SYNTH = 12.
        let candidates: Vec<TidalTrack> = (0..20)
            .map(|i| make_lieder_track(500 + i, &format!("Singer{i}")))
            .collect();
        let outcomes = best_work_level_candidates_multiple(
            &candidates,
            "3 Gesänge von Goethe",
            Some(WorkBucket::Vocal),
            "q".to_string(),
        );
        assert!(
            outcomes.len() <= MAX_WORK_LEVEL_SYNTH,
            "cap exceeded: {}",
            outcomes.len()
        );
        assert_eq!(outcomes.len(), MAX_WORK_LEVEL_SYNTH);
    }

    // ---- D-041 (A3) — genre-bucket penalty ----

    #[test]
    fn lieder_query_against_symphony_album_is_penalised() {
        // Beethoven Op. 83 (lieder) with Tidal candidate whose album
        // is "Symphonies Nos. 1-9". Without the penalty, title overlap
        // ("Symphony" ∩ ... no actual word match) gives a low title
        // score but no-artist (0.40) + no-year (0.20) + no-duration
        // (0.10) sums to 0.70+ which would clear the 0.62 threshold.
        // With the −0.30 penalty, score falls to ~0.40 — well below.
        let mut t = make_track(60, "Symphony No. 3 in E♭ — I. Allegro con brio",
            "Karajan", None, 1000);
        if let Some(ref mut album) = t.album {
            album.title = "Beethoven: Symphonies Nos. 1-9".to_string();
        }
        let s_no_penalty = score_candidate(
            &t,
            None,
            "3 Gesänge von Goethe",
            None,
            None,
            None,
        );
        let s_with_penalty = score_candidate(
            &t,
            None,
            "3 Gesänge von Goethe",
            None,
            None,
            Some(WorkBucket::Vocal),
        );
        assert!(
            s_with_penalty + 0.001 < s_no_penalty,
            "penalty should reduce score: {s_with_penalty} vs {s_no_penalty}"
        );
        // And the penalised score must drop below the work-level
        // threshold so the fallback won't synthesise it.
        assert!(
            s_with_penalty < WORK_LEVEL_THRESHOLD,
            "penalised score {s_with_penalty} should be below {WORK_LEVEL_THRESHOLD}"
        );
    }

    #[test]
    fn symphony_query_against_symphony_album_is_not_penalised() {
        let mut t = make_track(61, "Symphony No. 9 in D minor", "Karajan", None, 4400);
        if let Some(ref mut album) = t.album {
            album.title = "Beethoven Symphony No. 9".to_string();
        }
        let s_no_type = score_candidate(
            &t,
            None,
            "Symphony No. 9",
            None,
            None,
            None,
        );
        let s_with_type = score_candidate(
            &t,
            None,
            "Symphony No. 9",
            None,
            None,
            Some(WorkBucket::Symphonies),
        );
        assert_eq!(
            s_no_type, s_with_type,
            "matching bucket should produce identical scores"
        );
    }

    #[test]
    fn unknown_album_kind_yields_no_penalty() {
        let mut t = make_track(62, "Sonata in C", "Pollini", None, 1500);
        if let Some(ref mut album) = t.album {
            // Generic title with no bucket signal.
            album.title = "Pollini plays Beethoven".to_string();
        }
        let s_no_type = score_candidate(
            &t,
            None,
            "Sonata in C",
            None,
            None,
            None,
        );
        let s_with_type = score_candidate(
            &t,
            None,
            "Sonata in C",
            None,
            None,
            Some(WorkBucket::Chamber),
        );
        assert_eq!(
            s_no_type, s_with_type,
            "Unknown album-kind hint must never penalise"
        );
    }

    // ---- D-038 (bug 4) — error classification ----

    #[test]
    fn http_status_classifies_429_and_5xx_as_transient() {
        use crate::SoneError;
        let e429 = SoneError::from_http_status(429, "rate limited".into());
        assert!(e429.is_transient());
        let e500 = SoneError::from_http_status(500, "internal".into());
        assert!(e500.is_transient());
        let e503 = SoneError::from_http_status(503, "unavailable".into());
        assert!(e503.is_transient());
    }

    #[test]
    fn http_status_classifies_4xx_non_429_as_permanent() {
        use crate::SoneError;
        let e404 = SoneError::from_http_status(404, "not found".into());
        assert!(!e404.is_transient());
        assert!(e404.is_network());
        let e403 = SoneError::from_http_status(403, "forbidden".into());
        assert!(!e403.is_transient());
    }

    #[test]
    fn is_network_covers_both_transient_and_permanent() {
        use crate::SoneError;
        let perm = SoneError::Network("permanent".into());
        let trans = SoneError::NetworkTransient("blip".into());
        assert!(perm.is_network());
        assert!(trans.is_network());
        assert!(!perm.is_transient());
        assert!(trans.is_transient());
    }
}
