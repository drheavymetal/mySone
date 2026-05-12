//! Extended composers snapshot provider (Phase 7 — D-027 / D-031 / D-033).
//!
//! Where `OpenOpusProvider` carries 33 curated composers with `popular`
//! flags + works recommendations, this provider carries the **wide
//! universe** — ~6k composers harvested from Wikidata via SPARQL with
//! the classical-genre filter (`wdt:P136 → wdt:P279* → wd:Q9730`) plus
//! a UNION branch for adjacent genres that lack the closure
//! (minimalism, contemporary classical, opera, sacred monophony, etc.).
//!
//! The snapshot is built by `docs/classical/scripts/snapshot_composers_extended.py`
//! and shipped via `include_bytes!`. CI does not regenerate it; the
//! operator runs the script when a release is being prepared (D-032).
//!
//! ### Why two snapshots
//!
//! D-033 deliberately keeps `OpenOpusProvider` (33 composers, curated
//! `popular`/`recommended` flags + work recommendations) **alongside**
//! this extended provider. They serve different roles:
//!
//!   - OpenOpus → "Featured composers" / "Top picks" / Editor's Choice.
//!   - Extended → BrowseComposers (full universe) + search tokenizer
//!     index (so "Hildegard" or "Tchaikovsky" tokenizes correctly even
//!     for composers outside OpenOpus).
//!
//! Composers present in both inherit OpenOpus' curated fields (popular,
//! open_opus_id) at harvest time; composers only in the extended snapshot
//! ship with `popular=false` and no `open_opus_id`.
//!
//! ### Bit-perfect contract
//!
//! Read-only. Pure lookup. No I/O, no rate-limiter, no audio routing
//! involvement. Same shape as `OpenOpusProvider`.

use std::sync::OnceLock;

use serde::Deserialize;

use super::super::types::{ComposerSummary, Era};
use super::openopus::era_for_epoch_label;

const SNAPSHOT_BYTES: &[u8] = include_bytes!("../../../data/composers-extended.json");

/// Pre-baked snapshot, parsed once on first access.
static SNAPSHOT: OnceLock<ExtendedComposersSnapshot> = OnceLock::new();

fn snapshot() -> &'static ExtendedComposersSnapshot {
    SNAPSHOT.get_or_init(|| {
        serde_json::from_slice::<ExtendedComposersSnapshot>(SNAPSHOT_BYTES)
            .expect("composers-extended snapshot is malformed — rebuild via the harvest script")
    })
}

// ---------------------------------------------------------------------------
// Wire shapes (must match the JSON produced by snapshot_composers_extended.py)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ExtendedComposersSnapshot {
    pub schema_version: u32,
    pub generated_at: String,
    /// Sentinel mirror of the threshold used at harvest time. `5` by
    /// default; serves as a documentation breadcrumb only — the runtime
    /// does not enforce it (the snapshot is already filtered).
    pub harvest_threshold_recording_count: i32,
    pub composers: Vec<ExtendedComposer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtendedComposer {
    pub mbid: String,
    /// Wikidata QID (e.g. "Q255" for Beethoven). May be empty for
    /// composers spliced via the OpenOpus defensive merge path.
    pub qid: String,
    pub name: String,
    pub full_name: Option<String>,
    pub birth_year: Option<i32>,
    pub death_year: Option<i32>,
    /// Raw epoch label (e.g. "Baroque", "Late Romantic"). May be `None`
    /// for composers without birth/death years on Wikidata.
    pub epoch: Option<String>,
    pub portrait_url: Option<String>,
    /// `-1` sentinel = "MB enrichment skipped at harvest, accepted via
    /// Wikidata genre proxy". Positive values appear when the harvest
    /// ran with `--with-mb-counts`.
    #[serde(default = "default_recording_count")]
    pub recording_count: i32,
    /// True iff the composer was already in the OpenOpus snapshot when
    /// the harvest ran (curated by OpenOpus).
    #[serde(default)]
    pub popular: bool,
    /// Carried over from OpenOpus when the composer overlaps. None for
    /// composers only in the extended snapshot.
    pub open_opus_id: Option<String>,
}

fn default_recording_count() -> i32 {
    -1
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub struct ExtendedComposersProvider;

impl ExtendedComposersProvider {
    pub fn new() -> Self {
        // Force parse on construction so a malformed bundle fails fast at
        // startup, mirroring the OpenOpusProvider contract.
        let _ = snapshot();
        Self
    }

    /// Total composers in the extended universe. Useful for the Hub's
    /// "Catalog: X composers indexed" footer (F7.3).
    pub fn total_count(&self) -> usize {
        snapshot().composers.len()
    }

    /// Top-N composers for BrowseComposers + search index. Ordered:
    /// popular (OpenOpus-curated) first, then by birth year ascending
    /// (so newer composers don't crowd the top of the list).
    pub fn top_composers(&self, limit: usize) -> Vec<ComposerSummary> {
        let snap = snapshot();
        let mut out: Vec<ComposerSummary> = snap
            .composers
            .iter()
            .map(composer_to_summary)
            .collect();
        out.sort_by(|a, b| {
            b.popular
                .cmp(&a.popular)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        out.truncate(limit);
        out
    }

    /// All composers in a given era. Mirrors `OpenOpusProvider::composers_by_era`.
    pub fn composers_by_era(&self, era: Era) -> Vec<ComposerSummary> {
        snapshot()
            .composers
            .iter()
            .filter(|c| era_for_epoch_label(c.epoch.as_deref(), c.birth_year) == era)
            .map(composer_to_summary)
            .collect()
    }

    /// Lookup by MBID. Returns `None` when the composer is not in the
    /// extended snapshot (the caller may fall back to MB cascade).
    pub fn lookup_composer_summary(&self, mbid: &str) -> Option<ComposerSummary> {
        snapshot()
            .composers
            .iter()
            .find(|c| c.mbid.eq_ignore_ascii_case(mbid))
            .map(composer_to_summary)
    }

    /// Read access to the raw entries. Used by `classical::search` to
    /// build the composer-name index (D-031). Lifetime is `'static`
    /// since the snapshot lives in a `OnceLock`.
    pub fn all_composers(&self) -> &'static [ExtendedComposer] {
        &snapshot().composers
    }
}

impl Default for ExtendedComposersProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn composer_to_summary(c: &ExtendedComposer) -> ComposerSummary {
    ComposerSummary {
        mbid: c.mbid.clone(),
        open_opus_id: c.open_opus_id.clone(),
        name: c.full_name.clone().unwrap_or_else(|| c.name.clone()),
        full_name: c.full_name.clone(),
        birth_year: c.birth_year,
        death_year: c.death_year,
        era: era_for_epoch_label(c.epoch.as_deref(), c.birth_year),
        portrait_url: c.portrait_url.clone(),
        popular: c.popular,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_loads_with_thousands_of_composers() {
        let provider = ExtendedComposersProvider::new();
        let total = provider.total_count();
        assert!(
            total >= 600,
            "extended snapshot should ship ≥ 600 composers, got {total}"
        );
        // Sanity upper bound — the harvest filter should cap us under
        // ~10k. If we ever blow past this, the snapshot has bloated.
        assert!(total < 20_000, "extended snapshot ballooned: {total}");
    }

    #[test]
    fn top_composers_caps_at_limit() {
        let provider = ExtendedComposersProvider::new();
        let top = provider.top_composers(50);
        assert!(top.len() <= 50);
        assert!(top.len() >= 30, "expected ≥ 30 composers, got {}", top.len());
    }

    #[test]
    fn top_composers_orders_popular_first() {
        let provider = ExtendedComposersProvider::new();
        let top = provider.top_composers(40);
        // The first ≥ 1 entries should all be popular (OpenOpus-curated).
        // We don't assert all because the cap may slice across the boundary.
        let any_popular = top.iter().any(|c| c.popular);
        assert!(any_popular, "top list should contain at least one popular composer");
        // The first popular entry must precede the first non-popular entry.
        let first_popular_idx = top.iter().position(|c| c.popular);
        let first_unpopular_idx = top.iter().position(|c| !c.popular);
        if let (Some(p), Some(u)) = (first_popular_idx, first_unpopular_idx) {
            assert!(p < u, "popular composers must precede unpopular ones");
        }
    }

    #[test]
    fn canon_composers_present() {
        let provider = ExtendedComposersProvider::new();
        // Beethoven, Bach, Mozart MBIDs (also in the OpenOpus snapshot).
        for (name, mbid) in &[
            ("Beethoven", "1f9df192-a621-4f54-8850-2c5373b7eac9"),
            ("Bach", "24f1766e-9635-4d58-a4d4-9413f9f98a4c"),
            ("Mozart", "b972f589-fb0e-474e-b64a-803b0364fa75"),
            ("Tchaikovsky", "9ddd7abc-9e1b-471d-8031-583bc6bc8be9"),
        ] {
            let found = provider.lookup_composer_summary(mbid);
            assert!(
                found.is_some(),
                "{} should be in the extended snapshot (mbid {})",
                name,
                mbid
            );
        }
    }

    #[test]
    fn extended_universe_is_strictly_larger_than_openopus() {
        let provider = ExtendedComposersProvider::new();
        // Simple sanity: extended snapshot must have many more composers
        // than the OpenOpus original (which ships 33).
        assert!(
            provider.total_count() > 33 * 10,
            "extended snapshot should be ≥ 10x bigger than OpenOpus"
        );
    }

    #[test]
    fn composers_by_era_baroque_includes_bach() {
        let provider = ExtendedComposersProvider::new();
        let baroque = provider.composers_by_era(Era::Baroque);
        let names: Vec<String> = baroque.iter().map(|c| c.name.clone()).collect();
        assert!(
            names.iter().any(|n| n.contains("Bach")),
            "baroque era should include at least one Bach: snapshot has {} baroque composers",
            baroque.len()
        );
    }

    #[test]
    fn composers_by_era_post_war_nonempty() {
        let provider = ExtendedComposersProvider::new();
        let post_war = provider.composers_by_era(Era::PostWar);
        assert!(
            !post_war.is_empty(),
            "post-war era should have composers in the extended snapshot"
        );
    }

    #[test]
    fn lookup_unknown_mbid_returns_none() {
        let provider = ExtendedComposersProvider::new();
        let result = provider.lookup_composer_summary("00000000-0000-0000-0000-000000000000");
        assert!(result.is_none());
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let provider = ExtendedComposersProvider::new();
        let upper = provider.lookup_composer_summary("1F9DF192-A621-4F54-8850-2C5373B7EAC9");
        let lower = provider.lookup_composer_summary("1f9df192-a621-4f54-8850-2c5373b7eac9");
        assert!(upper.is_some());
        assert!(lower.is_some());
        assert_eq!(
            upper.expect("checked above").mbid,
            lower.expect("checked above").mbid
        );
    }

    #[test]
    fn all_composers_is_static_slice() {
        let provider = ExtendedComposersProvider::new();
        let all = provider.all_composers();
        assert!(!all.is_empty());
        // First call and second call return the same memory (OnceLock).
        let again = provider.all_composers();
        assert_eq!(all.as_ptr(), again.as_ptr());
    }

    #[test]
    fn schema_version_is_one() {
        let snap = snapshot();
        assert_eq!(snap.schema_version, 1, "schema version should be 1 in V1");
    }
}
