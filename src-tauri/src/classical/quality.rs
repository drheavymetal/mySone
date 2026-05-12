//! Audio-quality ranking and "best available" aggregator for the
//! Classical Hub. Pure logic — no I/O, no async, no audio routing.
//!
//! Reference: D-018 (numeric scoring), CLASSICAL_DESIGN.md §4.1 (USP).
//!
//! Design notes:
//!   * `score_recording` returns a `u32` so consumers can sort with a
//!     plain `Ord` impl (no NaN edge cases).
//!   * `best_available` is a single pass over the recording slice; it
//!     never allocates per-call.
//!   * The ATMOS audio mode is treated as a *bonus* on top of the tier
//!     score (it represents a different mixing decision, not a
//!     replacement). For sorting purposes, ATMOS edges out HIRES_LOSSLESS
//!     16/44.1 but stays behind HIRES_LOSSLESS 24/96 — see tests.
//!
//! Bit-perfect contract: ZERO contact with audio.rs, hw_volume.rs, or
//! signal_path.rs. This module is read-only over `Recording` data the
//! catalog already owns.

use super::types::{BestAvailableQuality, Recording};

// ---------------------------------------------------------------------------
// Tier constants — kept private so external code can't hand-roll
// comparable values that drift away from the tests below.
// ---------------------------------------------------------------------------

/// Approximate "weight" of each tier. Within a tier, we add a bonus
/// derived from the sample rate and bit depth so 24/192 outranks 24/96.
const TIER_HIRES_LOSSLESS: u32 = 4_000;
const TIER_LOSSLESS: u32 = 3_000;
/// MQA is a controversial lossy-folded format the project does not
/// promote. We keep it scoreable (so playable matches still beat
/// nothing) but rank it below LOSSLESS — yet still ahead of plain
/// lossy HIGH (160-320 kbps AAC), since MQA carries effectively more
/// information when unfolded.
const TIER_MQA: u32 = 2_000;
const TIER_HIGH: u32 = 1_000;
/// Dolby Atmos contributes a small bonus on top of whatever the tier
/// score is. It is *not* a replacement tier, so we add — never override.
const ATMOS_BONUS: u32 = 200;

// ---------------------------------------------------------------------------
// Public scoring API
// ---------------------------------------------------------------------------

/// Numeric quality score for a recording. Higher is better. Pure: depends
/// only on `audio_quality_tags`, `audio_modes`, `sample_rate_hz`,
/// `bit_depth`. Returns 0 for recordings with no playable Tidal match.
pub fn score_recording(rec: &Recording) -> u32 {
    if rec.audio_quality_tags.is_empty() && rec.audio_modes.is_empty() {
        return 0;
    }

    let mut score = base_tier_score(&rec.audio_quality_tags);
    if score > 0 {
        score += refinement_bonus(rec.sample_rate_hz, rec.bit_depth);
    }
    if has_atmos(&rec.audio_modes) {
        score += ATMOS_BONUS;
    }
    score
}

/// Sample-rate / bit-depth refinement bonus added on top of the tier score.
/// Capped so a 16-bit MQA can never overtake a 24-bit LOSSLESS at the
/// boundary — the tier difference (LOSSLESS vs MQA) is at least 200 by
/// construction.
fn refinement_bonus(sample_rate_hz: Option<u32>, bit_depth: Option<u8>) -> u32 {
    let mut bonus: u32 = 0;
    if let Some(bd) = bit_depth {
        if bd >= 24 {
            bonus += 60;
        } else if bd >= 16 {
            bonus += 20;
        }
    }
    if let Some(sr) = sample_rate_hz {
        // Brackets chosen so 192k > 96k > 48k > 44.1k.
        bonus += match sr {
            r if r >= 352_800 => 90,
            r if r >= 192_000 => 80,
            r if r >= 96_000 => 60,
            r if r >= 88_200 => 55,
            r if r >= 48_000 => 30,
            r if r >= 44_100 => 20,
            _ => 0,
        };
    }
    bonus
}

fn base_tier_score(tags: &[String]) -> u32 {
    if tags.iter().any(|t| t == "HIRES_LOSSLESS") {
        return TIER_HIRES_LOSSLESS;
    }
    if tags.iter().any(|t| t == "LOSSLESS") {
        return TIER_LOSSLESS;
    }
    if tags.iter().any(|t| t == "MQA") {
        return TIER_MQA;
    }
    if tags.iter().any(|t| t == "HIGH") {
        return TIER_HIGH;
    }
    0
}

fn has_atmos(modes: &[String]) -> bool {
    modes.iter().any(|m| m == "DOLBY_ATMOS")
}

// ---------------------------------------------------------------------------
// Best-of-N aggregator
// ---------------------------------------------------------------------------

/// Reduce a slice of recordings to a `BestAvailableQuality` summary.
/// Returns `None` when no recording has any Tidal metadata yet.
///
/// Strategy: pick the recording with the highest `score_recording`
/// result; surface its tier + refined rate. The `has_atmos` flag is
/// independent — it's true if *any* recording in the set has the Atmos
/// mode, even if it isn't the best-scoring one (so the UI can show
/// "Best available 24/192 · Atmos available" correctly).
pub fn best_available(recordings: &[Recording]) -> Option<BestAvailableQuality> {
    let mut best: Option<&Recording> = None;
    let mut best_score: u32 = 0;
    let mut atmos_seen = false;
    for rec in recordings.iter() {
        if has_atmos(&rec.audio_modes) {
            atmos_seen = true;
        }
        let s = rec.quality_score;
        if s > best_score {
            best_score = s;
            best = Some(rec);
        }
    }
    let chosen = best?;
    if best_score == 0 {
        return None;
    }
    let tier = primary_tier(&chosen.audio_quality_tags)?;
    Some(BestAvailableQuality {
        tier: tier.to_string(),
        sample_rate_hz: chosen.sample_rate_hz,
        bit_depth: chosen.bit_depth,
        has_atmos: atmos_seen,
    })
}

/// First-wins primary-tier selector used by the aggregator. Order
/// mirrors `base_tier_score` so the "tier" surfaced for the banner is
/// the same axis used for the score.
fn primary_tier(tags: &[String]) -> Option<&'static str> {
    if tags.iter().any(|t| t == "HIRES_LOSSLESS") {
        return Some("HIRES_LOSSLESS");
    }
    if tags.iter().any(|t| t == "LOSSLESS") {
        return Some("LOSSLESS");
    }
    if tags.iter().any(|t| t == "MQA") {
        return Some("MQA");
    }
    if tags.iter().any(|t| t == "HIGH") {
        return Some("HIGH");
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classical::types::{MatchConfidence, Recording};

    fn make_rec(
        tags: &[&str],
        modes: &[&str],
        sample_rate_hz: Option<u32>,
        bit_depth: Option<u8>,
    ) -> Recording {
        let mut r = Recording::shell("rec-test", "work-test");
        r.audio_quality_tags = tags.iter().map(|s| s.to_string()).collect();
        r.audio_modes = modes.iter().map(|s| s.to_string()).collect();
        r.sample_rate_hz = sample_rate_hz;
        r.bit_depth = bit_depth;
        r.match_confidence = MatchConfidence::IsrcBound;
        r
    }

    #[test]
    fn empty_recording_scores_zero() {
        let r = make_rec(&[], &[], None, None);
        assert_eq!(score_recording(&r), 0);
    }

    #[test]
    fn hires_outranks_lossless() {
        let hires = make_rec(&["HIRES_LOSSLESS"], &[], Some(96_000), Some(24));
        let lossless = make_rec(&["LOSSLESS"], &[], Some(44_100), Some(16));
        assert!(score_recording(&hires) > score_recording(&lossless));
    }

    #[test]
    fn lossless_outranks_mqa() {
        let lossless = make_rec(&["LOSSLESS"], &[], Some(44_100), Some(16));
        let mqa = make_rec(&["MQA"], &[], Some(96_000), Some(24));
        // Even with MQA at "higher" rate, LOSSLESS wins on tier.
        assert!(score_recording(&lossless) > score_recording(&mqa));
    }

    #[test]
    fn mqa_outranks_high() {
        let mqa = make_rec(&["MQA"], &[], None, None);
        let high = make_rec(&["HIGH"], &[], None, None);
        assert!(score_recording(&mqa) > score_recording(&high));
    }

    #[test]
    fn hires_24_192_outranks_24_96() {
        let hi192 = make_rec(&["HIRES_LOSSLESS"], &[], Some(192_000), Some(24));
        let hi96 = make_rec(&["HIRES_LOSSLESS"], &[], Some(96_000), Some(24));
        assert!(score_recording(&hi192) > score_recording(&hi96));
    }

    #[test]
    fn hires_24_96_outranks_24_48() {
        let hi96 = make_rec(&["HIRES_LOSSLESS"], &[], Some(96_000), Some(24));
        let hi48 = make_rec(&["HIRES_LOSSLESS"], &[], Some(48_000), Some(24));
        assert!(score_recording(&hi96) > score_recording(&hi48));
    }

    #[test]
    fn atmos_bonus_applies_on_top_of_tier() {
        let plain_lossless = make_rec(&["LOSSLESS"], &[], Some(44_100), Some(16));
        let atmos_lossless =
            make_rec(&["LOSSLESS"], &["DOLBY_ATMOS"], Some(44_100), Some(16));
        assert!(score_recording(&atmos_lossless) > score_recording(&plain_lossless));
        assert_eq!(
            score_recording(&atmos_lossless),
            score_recording(&plain_lossless) + ATMOS_BONUS
        );
    }

    #[test]
    fn atmos_does_not_promote_above_hires_24_96() {
        // Atmos LOSSLESS should still rank below HIRES 24/96 — the tier
        // jump is bigger than the Atmos bonus.
        let atmos_lossless =
            make_rec(&["LOSSLESS"], &["DOLBY_ATMOS"], Some(44_100), Some(16));
        let hi96 = make_rec(&["HIRES_LOSSLESS"], &[], Some(96_000), Some(24));
        assert!(score_recording(&hi96) > score_recording(&atmos_lossless));
    }

    #[test]
    fn missing_rate_still_scored_by_tier() {
        let bare_hires = make_rec(&["HIRES_LOSSLESS"], &[], None, None);
        let bare_lossless = make_rec(&["LOSSLESS"], &[], None, None);
        assert!(score_recording(&bare_hires) > score_recording(&bare_lossless));
        assert!(score_recording(&bare_lossless) > 0);
    }

    #[test]
    fn best_available_picks_highest_tier_with_known_rate() {
        let mut hi192 = make_rec(&["HIRES_LOSSLESS"], &[], Some(192_000), Some(24));
        hi192.quality_score = score_recording(&hi192);
        let mut hi96 = make_rec(&["HIRES_LOSSLESS"], &[], Some(96_000), Some(24));
        hi96.quality_score = score_recording(&hi96);
        let mut lossless = make_rec(&["LOSSLESS"], &[], Some(44_100), Some(16));
        lossless.quality_score = score_recording(&lossless);

        let best = best_available(&[lossless, hi96, hi192]).unwrap();
        assert_eq!(best.tier, "HIRES_LOSSLESS");
        assert_eq!(best.sample_rate_hz, Some(192_000));
        assert_eq!(best.bit_depth, Some(24));
        assert!(!best.has_atmos);
    }

    #[test]
    fn best_available_flags_atmos_even_if_best_is_not_atmos() {
        // Best rec is HIRES 24/96 stereo; another rec is plain LOSSLESS Atmos.
        // The summary should keep tier=HIRES but flag has_atmos=true so the UI
        // can render "Best 24/96 · Atmos available".
        let mut hi96 = make_rec(&["HIRES_LOSSLESS"], &[], Some(96_000), Some(24));
        hi96.quality_score = score_recording(&hi96);
        let mut atmos =
            make_rec(&["LOSSLESS"], &["DOLBY_ATMOS"], Some(44_100), Some(16));
        atmos.quality_score = score_recording(&atmos);

        let best = best_available(&[atmos, hi96]).unwrap();
        assert_eq!(best.tier, "HIRES_LOSSLESS");
        assert!(best.has_atmos);
    }

    #[test]
    fn best_available_returns_none_when_no_metadata() {
        let r = make_rec(&[], &[], None, None);
        // quality_score not populated -> 0
        assert!(best_available(&[r]).is_none());
    }

    #[test]
    fn best_available_returns_none_for_empty_slice() {
        let empty: Vec<Recording> = Vec::new();
        assert!(best_available(&empty).is_none());
    }

    #[test]
    fn best_available_picks_atmos_only_track_when_alone() {
        let mut atmos = make_rec(&["LOSSLESS"], &["DOLBY_ATMOS"], None, None);
        atmos.quality_score = score_recording(&atmos);
        let best = best_available(&[atmos]).unwrap();
        assert_eq!(best.tier, "LOSSLESS");
        assert!(best.has_atmos);
    }

    // -----------------------------------------------------------------
    // Phase 4 acceptance scenario — end-to-end ranking for the Beethoven
    // 9 use case described in CLASSICAL_DESIGN.md §11. We model the
    // recordings list with the same tier/rate combinations the design
    // doc cites (Karajan 16/44.1 LOSSLESS, Bernstein 24/96 HIRES, Solti
    // 24/192 HIRES + ATMOS, Gardiner 16/44.1 LOSSLESS, Furtwängler
    // 1951 not-on-Tidal). The acceptance is:
    //   1. The "best available" surfaces 24/192 HIRES_LOSSLESS.
    //   2. Sort by quality_score descending puts Solti first,
    //      Bernstein second, Karajan/Gardiner equally below.
    //   3. A "Hi-Res only" filter (tier == HIRES_LOSSLESS) keeps only
    //      Solti + Bernstein.
    // -----------------------------------------------------------------

    fn build_beethoven9_fixture() -> Vec<Recording> {
        let mut karajan = make_rec(&["LOSSLESS"], &[], Some(44_100), Some(16));
        karajan.quality_score = score_recording(&karajan);
        let mut bernstein = make_rec(&["HIRES_LOSSLESS"], &[], Some(96_000), Some(24));
        bernstein.quality_score = score_recording(&bernstein);
        let mut solti = make_rec(
            &["HIRES_LOSSLESS"],
            &["DOLBY_ATMOS"],
            Some(192_000),
            Some(24),
        );
        solti.quality_score = score_recording(&solti);
        let mut gardiner = make_rec(&["LOSSLESS"], &[], Some(44_100), Some(16));
        gardiner.quality_score = score_recording(&gardiner);
        let furtwangler_offline = make_rec(&[], &[], None, None);
        // not on Tidal -> quality_score stays 0.

        vec![
            karajan,
            bernstein,
            solti,
            gardiner,
            furtwangler_offline,
        ]
    }

    #[test]
    fn beethoven9_acceptance_best_available_is_24_192() {
        let recs = build_beethoven9_fixture();
        let best = best_available(&recs).unwrap();
        assert_eq!(best.tier, "HIRES_LOSSLESS");
        assert_eq!(best.sample_rate_hz, Some(192_000));
        assert_eq!(best.bit_depth, Some(24));
        assert!(best.has_atmos);
    }

    #[test]
    fn beethoven9_acceptance_sort_by_quality_score() {
        let recs = build_beethoven9_fixture();
        let mut sorted: Vec<&Recording> = recs.iter().collect();
        sorted.sort_by(|a, b| b.quality_score.cmp(&a.quality_score));

        // 24/192 + Atmos must be at the top.
        assert_eq!(sorted[0].sample_rate_hz, Some(192_000));
        assert!(sorted[0].audio_modes.iter().any(|m| m == "DOLBY_ATMOS"));
        // 24/96 HIRES second.
        assert_eq!(sorted[1].sample_rate_hz, Some(96_000));
        assert!(sorted[1].audio_quality_tags.iter().any(|t| t == "HIRES_LOSSLESS"));
        // Furtwängler (no metadata) at the bottom.
        assert_eq!(sorted.last().unwrap().quality_score, 0);
    }

    #[test]
    fn beethoven9_acceptance_hires_only_filter() {
        let recs = build_beethoven9_fixture();
        let hires_only: Vec<&Recording> = recs
            .iter()
            .filter(|r| r.audio_quality_tags.iter().any(|t| t == "HIRES_LOSSLESS"))
            .collect();
        assert_eq!(hires_only.len(), 2);
        for rec in hires_only.iter() {
            assert!(rec
                .audio_quality_tags
                .iter()
                .any(|t| t == "HIRES_LOSSLESS"));
        }
    }
}
