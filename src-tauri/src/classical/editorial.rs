//! Phase 5 (B5.2) — editorial seeds provider.
//!
//! Reference: D-020. CLASSICAL_DESIGN.md §4.6 (open author bylines + editorial).
//!
//! Loads `src-tauri/data/editorial.json` once into a `OnceLock` and
//! exposes lookups by:
//!   * composer MBID + title-canonical → work editorial entry,
//!   * composer MBID → composer editor note,
//!   * a flat list of all picks (Hub home).
//!
//! The Catalog service queries this provider during `get_work` /
//! `get_composer` so the UI receives `editor_note` + `editors_choice`
//! seamlessly.
//!
//! Bit-perfect contract: ZERO contact with audio routing. This module
//! is pure in-memory lookup.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::types::WorkBucket;

const SEED_BYTES: &[u8] = include_bytes!("../../data/editorial.json");
/// Phase 9 (D-044 + D-045) — extended schema v2 snapshot. Co-existing
/// with the v1 snapshot; lookups cascade extended → v1 → none.
const EXTENDED_BYTES: &[u8] = include_bytes!("../../data/editorial-extended.json");

/// Parsed v1 snapshot, computed lazily on first access.
static SNAPSHOT: OnceLock<EditorialSnapshot> = OnceLock::new();
/// Parsed v2 (extended) snapshot, computed lazily on first access.
static EXTENDED_SNAPSHOT: OnceLock<ExtendedSnapshot> = OnceLock::new();

fn snapshot() -> &'static EditorialSnapshot {
    SNAPSHOT.get_or_init(|| {
        let raw: RawSnapshot = serde_json::from_slice(SEED_BYTES)
            .expect("editorial.json is malformed — fix the curated file");
        EditorialSnapshot::from_raw(raw)
    })
}

fn extended_snapshot() -> &'static ExtendedSnapshot {
    EXTENDED_SNAPSHOT.get_or_init(|| {
        let raw: RawExtendedSnapshot = serde_json::from_slice(EXTENDED_BYTES)
            .expect("editorial-extended.json is malformed — fix the curated file");
        ExtendedSnapshot::from_raw(raw)
    })
}

// ---------------------------------------------------------------------------
// Public types — also serialised for Tauri commands
// ---------------------------------------------------------------------------

/// Single editorial pick surfaced on the Hub home + work pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorsChoice {
    /// Conductor name, when applicable. None for solo/chamber.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conductor: Option<String>,
    /// Performer / ensemble (e.g. "Berliner Philharmoniker").
    pub performer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Editorial note: defends the choice (Gramophone Hall of Fame, etc).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkEditorial {
    pub composer_mbid: String,
    pub title_canonical: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editors_choice: Option<EditorsChoice>,
    /// Phase 9 (D-040 + D-045) — explicit bucket override. When
    /// present in the snapshot, beats the heuristic in
    /// `classical::buckets::bucket_for(...)`. Used for canon entries
    /// where the heuristic would mis-classify (e.g. Bach Passions
    /// without P136 fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<WorkBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposerEditorial {
    pub composer_mbid: String,
    pub name: String,
    pub editor_note: String,
}

// ---------------------------------------------------------------------------
// Phase 9 (D-044 + D-045) — extended schema v2: 5-section essays per work
// ---------------------------------------------------------------------------

/// Phase 9 (D-044) — single source attribution for an extended note.
/// Multiple sources per note are common (Wikipedia + Wikidata +
/// "mySone team" curation); each carries its own license tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedSource {
    /// "wikipedia" | "wikidata" | "editor" | (other)
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// Phase 9 (D-044) — the 5 sub-sections rendered by `AboutThisWork`.
/// Each field is markdown-light: italics with `_text_`, bold with
/// `**text**`, links as `[label](url)`. The frontend renders without
/// a full markdown parser to keep the attack surface small.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedNoteBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premiere: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlights: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notable_recordings_essay: Option<String>,
}

/// Phase 9 (D-044) — full extended note for a work, including
/// translations and source attribution. Returned by
/// `EditorialProvider::lookup_extended` after locale resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedNote {
    /// Locale of the resolved body — "en" by default; if the caller
    /// requested "es" and a translation exists, "es"; if the caller
    /// requested "es" and no translation exists, "en" (fallback).
    pub language: String,
    pub body: ExtendedNoteBody,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<ExtendedSource>,
}

/// Phase 9 (D-045) — health summary returned by `schema_health()`. Used
/// by the integration tests + the `editorial-debug` command to verify
/// both snapshots loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedSchemaHealth {
    pub v1_composers: usize,
    pub v1_works: usize,
    pub v2_works: usize,
}

/// Hub-home pick: a flattened view used by the "Editor's Choice" section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorialPick {
    pub composer_mbid: String,
    pub composer_name: String,
    pub title_canonical: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogue: Option<String>,
    pub editors_choice: EditorsChoice,
}

// ---------------------------------------------------------------------------
// Wire shapes (raw JSON parsed once)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawSnapshot {
    #[allow(dead_code)]
    schema_version: u32,
    #[allow(dead_code)]
    generated_at: String,
    #[serde(default)]
    composers: Vec<RawComposer>,
    #[serde(default)]
    works: Vec<RawWork>,
}

#[derive(Debug, Deserialize)]
struct RawComposer {
    composer_mbid: String,
    name: String,
    editor_note: String,
}

#[derive(Debug, Deserialize)]
struct RawWork {
    composer_mbid: String,
    title_canonical: String,
    #[serde(default)]
    catalogue: Option<String>,
    #[serde(default)]
    editor_note: Option<String>,
    #[serde(default)]
    editors_choice: Option<EditorsChoice>,
    #[serde(default)]
    bucket: Option<WorkBucket>,
}

// ---------------------------------------------------------------------------
// Wire shapes for `editorial-extended.json` (schema v2)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawExtendedSnapshot {
    #[allow(dead_code)]
    schema_version: u32,
    #[allow(dead_code)]
    #[serde(default)]
    generated_at: String,
    #[serde(default)]
    works: Vec<RawExtendedWork>,
}

#[derive(Debug, Deserialize)]
struct RawExtendedWork {
    work_mbid: String,
    composer_mbid: String,
    title_canonical: String,
    #[serde(default)]
    match_titles: Vec<String>,
    #[serde(default)]
    catalogue: Option<String>,
    #[serde(default)]
    bucket: Option<WorkBucket>,
    #[serde(default)]
    extended: Option<RawExtendedBody>,
    #[serde(default)]
    #[allow(dead_code)]
    editors_choice: Option<RawExtendedEditorsChoice>,
}

#[derive(Debug, Deserialize)]
struct RawExtendedBody {
    #[serde(default = "default_extended_language")]
    language: String,
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    premiere: Option<String>,
    #[serde(default)]
    highlights: Option<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    notable_recordings_essay: Option<String>,
    #[serde(default)]
    sources: Vec<ExtendedSource>,
    #[serde(default)]
    translations: HashMap<String, RawTranslationBody>,
}

fn default_extended_language() -> String {
    "en".to_string()
}

#[derive(Debug, Deserialize)]
struct RawTranslationBody {
    #[serde(default)]
    origin: Option<String>,
    #[serde(default)]
    premiere: Option<String>,
    #[serde(default)]
    highlights: Option<String>,
    #[serde(default)]
    context: Option<String>,
    #[serde(default)]
    notable_recordings_essay: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawExtendedEditorsChoice {
    #[serde(default)]
    recording_mbid: Option<String>,
    #[serde(default)]
    rationale: Option<String>,
}

// ---------------------------------------------------------------------------
// Snapshot — indexed for O(1) lookup by composer + normalized title
// ---------------------------------------------------------------------------

pub struct EditorialSnapshot {
    /// `composer_mbid → ComposerEditorial`.
    composers: HashMap<String, ComposerEditorial>,
    /// `composer_mbid → Vec<WorkEditorial>` so we can both list a
    /// composer's curated picks *and* match a known work title against
    /// the seeds.
    works_by_composer: HashMap<String, Vec<WorkEditorial>>,
}

/// Phase 9 (D-044 + D-045) — extended snapshot keyed by `work_mbid`
/// for O(1) lookup. Each entry carries the full `ExtendedNoteBody`
/// for the default language plus any translations supplied.
struct ExtendedEntry {
    /// Kept for future debug/admin tooling that lists v2 entries by
    /// composer; not consumed by the production lookup paths today.
    #[allow(dead_code)]
    composer_mbid: String,
    /// Same as above — useful for tooling, not load-bearing.
    #[allow(dead_code)]
    title_canonical: String,
    #[allow(dead_code)]
    match_titles: Vec<String>,
    #[allow(dead_code)]
    catalogue: Option<String>,
    bucket: Option<WorkBucket>,
    body_default_language: String,
    body: ExtendedNoteBody,
    sources: Vec<ExtendedSource>,
    translations: HashMap<String, ExtendedNoteBody>,
}

pub struct ExtendedSnapshot {
    by_work_mbid: HashMap<String, ExtendedEntry>,
    /// Secondary index `composer_mbid → list of (normalised title,
    /// work_mbid)` so we can resolve an extended note from
    /// `(composer_mbid, title)` when the work_mbid is not yet in
    /// hand. Used by `lookup_bucket_extended`.
    titles_by_composer: HashMap<String, Vec<(String, String)>>,
}

impl ExtendedSnapshot {
    fn from_raw(raw: RawExtendedSnapshot) -> Self {
        let mut by_work_mbid: HashMap<String, ExtendedEntry> = HashMap::new();
        let mut titles_by_composer: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for w in raw.works.into_iter() {
            let entry_titles = std::iter::once(w.title_canonical.clone())
                .chain(w.match_titles.iter().cloned())
                .collect::<Vec<_>>();
            for title in entry_titles.iter() {
                titles_by_composer
                    .entry(w.composer_mbid.clone())
                    .or_default()
                    .push((normalize_title(title), w.work_mbid.clone()));
            }
            let (default_lang, body, sources, translations) = match w.extended {
                Some(body) => {
                    let lang = body.language.clone();
                    let main = ExtendedNoteBody {
                        origin: body.origin,
                        premiere: body.premiere,
                        highlights: body.highlights,
                        context: body.context,
                        notable_recordings_essay: body.notable_recordings_essay,
                    };
                    let sources = body.sources;
                    let mut t_map: HashMap<String, ExtendedNoteBody> = HashMap::new();
                    for (locale, raw_t) in body.translations.into_iter() {
                        t_map.insert(
                            locale,
                            ExtendedNoteBody {
                                origin: raw_t.origin,
                                premiere: raw_t.premiere,
                                highlights: raw_t.highlights,
                                context: raw_t.context,
                                notable_recordings_essay: raw_t.notable_recordings_essay,
                            },
                        );
                    }
                    (lang, main, sources, t_map)
                }
                None => (
                    "en".to_string(),
                    ExtendedNoteBody::default(),
                    Vec::new(),
                    HashMap::new(),
                ),
            };
            by_work_mbid.insert(
                w.work_mbid.clone(),
                ExtendedEntry {
                    composer_mbid: w.composer_mbid,
                    title_canonical: w.title_canonical,
                    match_titles: w.match_titles,
                    catalogue: w.catalogue,
                    bucket: w.bucket,
                    body_default_language: default_lang,
                    body,
                    sources,
                    translations,
                },
            );
        }
        Self {
            by_work_mbid,
            titles_by_composer,
        }
    }
}

impl EditorialSnapshot {
    fn from_raw(raw: RawSnapshot) -> Self {
        let mut composers: HashMap<String, ComposerEditorial> = HashMap::new();
        for c in raw.composers.into_iter() {
            composers.insert(
                c.composer_mbid.clone(),
                ComposerEditorial {
                    composer_mbid: c.composer_mbid,
                    name: c.name,
                    editor_note: c.editor_note,
                },
            );
        }
        let mut works_by_composer: HashMap<String, Vec<WorkEditorial>> = HashMap::new();
        for w in raw.works.into_iter() {
            let entry = WorkEditorial {
                composer_mbid: w.composer_mbid.clone(),
                title_canonical: w.title_canonical,
                catalogue: w.catalogue,
                editor_note: w.editor_note,
                editors_choice: w.editors_choice,
                bucket: w.bucket,
            };
            works_by_composer
                .entry(w.composer_mbid)
                .or_default()
                .push(entry);
        }
        Self {
            composers,
            works_by_composer,
        }
    }
}

// ---------------------------------------------------------------------------
// Provider — pure lookup, no I/O
// ---------------------------------------------------------------------------

pub struct EditorialProvider;

impl EditorialProvider {
    pub fn new() -> Self {
        // Force parse at startup so a malformed snapshot fails fast
        // rather than at the first user navigation. Same idiom as
        // `OpenOpusProvider::new`. Phase 9 (D-045) parses BOTH the v1
        // and the v2 (extended) snapshots — either malformed file
        // crashes the boot deterministically.
        let _ = snapshot();
        let _ = extended_snapshot();
        Self
    }

    /// Return the composer-level editorial entry, when curated.
    pub fn lookup_composer(&self, composer_mbid: &str) -> Option<ComposerEditorial> {
        snapshot().composers.get(composer_mbid).cloned()
    }

    /// Return the work-level editorial entry that matches both
    /// `composer_mbid` and `title` (loose normalised match — handles
    /// MB punctuation drift). Returns the first match; D-020 deliberately
    /// keeps one curated entry per work.
    pub fn lookup_work(&self, composer_mbid: &str, title: &str) -> Option<WorkEditorial> {
        let target = normalize_title(title);
        snapshot()
            .works_by_composer
            .get(composer_mbid)?
            .iter()
            .find(|w| {
                let candidate = normalize_title(&w.title_canonical);
                candidate == target
                    || candidate.contains(&target)
                    || target.contains(&candidate)
            })
            .cloned()
    }

    /// Phase 9 (D-040) — explicit bucket override from the snapshot,
    /// when curated. Cascade:
    ///   1. v2 extended snapshot (per `work_mbid` ideally; here we
    ///      match by composer + title because the catalog calls this
    ///      before persisting `work_mbid → bucket`).
    ///   2. v1 snapshot.
    ///   3. None.
    ///
    /// `composer_mbid` may be empty when the work hasn't been linked
    /// to a composer yet — returns `None` immediately in that case.
    pub fn lookup_bucket(
        &self,
        composer_mbid: &str,
        title: &str,
    ) -> Option<WorkBucket> {
        if composer_mbid.is_empty() {
            return None;
        }
        // (1) v2 extended snapshot — composer + title-normalised match.
        let target = normalize_title(title);
        if let Some(list) = extended_snapshot().titles_by_composer.get(composer_mbid) {
            for (norm_title, work_mbid) in list.iter() {
                if norm_title == &target
                    || norm_title.contains(&target)
                    || target.contains(norm_title)
                {
                    if let Some(entry) = extended_snapshot().by_work_mbid.get(work_mbid) {
                        if let Some(b) = entry.bucket {
                            return Some(b);
                        }
                    }
                }
            }
        }
        // (2) v1 snapshot.
        self.lookup_work(composer_mbid, title)
            .and_then(|w| w.bucket)
    }

    /// Phase 9 (D-044) — lookup an extended editorial note keyed by
    /// `work_mbid`. Returns `None` when the work is outside the v2
    /// snapshot (caller falls back to the Phase 5 `editor_note`
    /// or to Wikipedia summary).
    ///
    /// `locale`: ISO language tag, lowercased. Resolution:
    ///   1. `translations[locale]` if present.
    ///   2. The default-language body (usually "en").
    ///   3. None when the entry exists but the body is empty
    ///      (treated as a missing entry — frontend hides the
    ///      section).
    pub fn lookup_extended(
        &self,
        work_mbid: &str,
        locale: Option<&str>,
    ) -> Option<ExtendedNote> {
        let entry = extended_snapshot().by_work_mbid.get(work_mbid)?;
        let want_locale = locale.unwrap_or("en").to_lowercase();
        let (resolved_lang, body) = if want_locale != entry.body_default_language {
            match entry.translations.get(&want_locale) {
                Some(b) => (want_locale.clone(), b.clone()),
                None => (entry.body_default_language.clone(), entry.body.clone()),
            }
        } else {
            (entry.body_default_language.clone(), entry.body.clone())
        };
        // Empty body → nothing renders; frontend hides AboutThisWork.
        let any_section_present = body.origin.is_some()
            || body.premiere.is_some()
            || body.highlights.is_some()
            || body.context.is_some()
            || body.notable_recordings_essay.is_some();
        if !any_section_present {
            return None;
        }
        Some(ExtendedNote {
            language: resolved_lang,
            body,
            sources: entry.sources.clone(),
        })
    }

    /// Phase 9 (D-044) — composer + title fallback for extended
    /// lookup. Used when the frontend has the work title in hand but
    /// not its MBID (search results, cross-version comparisons).
    pub fn lookup_extended_by_title(
        &self,
        composer_mbid: &str,
        title: &str,
        locale: Option<&str>,
    ) -> Option<ExtendedNote> {
        if composer_mbid.is_empty() {
            return None;
        }
        let target = normalize_title(title);
        let snap = extended_snapshot();
        let list = snap.titles_by_composer.get(composer_mbid)?;
        for (norm_title, work_mbid) in list.iter() {
            if norm_title == &target
                || norm_title.contains(&target)
                || target.contains(norm_title)
            {
                if let Some(note) = self.lookup_extended(work_mbid, locale) {
                    return Some(note);
                }
            }
        }
        None
    }

    /// Force-parse both snapshots at startup so a malformed JSON
    /// fails fast rather than at the first user navigation.
    pub fn schema_health(&self) -> ExtendedSchemaHealth {
        let v1_works: usize = snapshot()
            .works_by_composer
            .values()
            .map(|v| v.len())
            .sum();
        let v2_works = extended_snapshot().by_work_mbid.len();
        ExtendedSchemaHealth {
            v1_composers: snapshot().composers.len(),
            v1_works,
            v2_works,
        }
    }

    /// Return all curated picks for a composer (used by the UI to render
    /// "Editor's recommendations" on a composer page in future). Phase 5
    /// does not surface this directly — placeholder for Phase 6.
    pub fn list_works_for_composer(&self, composer_mbid: &str) -> Vec<WorkEditorial> {
        snapshot()
            .works_by_composer
            .get(composer_mbid)
            .cloned()
            .unwrap_or_default()
    }

    /// Flat list of every work entry that has an `editors_choice` for
    /// the Hub home grid. Capped at `limit`. Stable order: snapshot order.
    pub fn list_picks(&self, limit: usize) -> Vec<EditorialPick> {
        let snap = snapshot();
        let mut out: Vec<EditorialPick> = Vec::new();
        for (composer_mbid, works) in snap.works_by_composer.iter() {
            let composer_name = snap
                .composers
                .get(composer_mbid)
                .map(|c| c.name.clone())
                .unwrap_or_default();
            for w in works.iter() {
                if let Some(choice) = w.editors_choice.as_ref() {
                    out.push(EditorialPick {
                        composer_mbid: composer_mbid.clone(),
                        composer_name: composer_name.clone(),
                        title_canonical: w.title_canonical.clone(),
                        catalogue: w.catalogue.clone(),
                        editors_choice: choice.clone(),
                    });
                }
            }
        }
        // Stable: alphabetical by composer name then title — keeps the UI
        // deterministic across reloads of the snapshot.
        out.sort_by(|a, b| {
            a.composer_name
                .cmp(&b.composer_name)
                .then_with(|| a.title_canonical.cmp(&b.title_canonical))
        });
        if out.len() > limit {
            out.truncate(limit);
        }
        out
    }
}

impl Default for EditorialProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Normalise a work title for cross-source matching. Same shape as
/// `catalog::normalize_title_for_match` — kept duplicated locally so we
/// don't widen `catalog`'s public surface.
fn normalize_title(title: &str) -> String {
    let mut s = title.to_lowercase();
    s = s.replace(['“', '”', '"'], "");
    s.retain(|c| c.is_ascii_alphanumeric() || c.is_whitespace());
    s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BEETHOVEN: &str = "1f9df192-a621-4f54-8850-2c5373b7eac9";
    const BACH: &str = "24f1766e-9635-4d58-a4d4-9413f9f98a4c";

    #[test]
    fn snapshot_loads() {
        let provider = EditorialProvider::new();
        let beet = provider.lookup_composer(BEETHOVEN);
        assert!(beet.is_some());
        assert!(beet.unwrap().editor_note.contains("Beethoven"));
    }

    #[test]
    fn lookup_work_exact_title() {
        let provider = EditorialProvider::new();
        let entry = provider.lookup_work(BEETHOVEN, "Symphony No. 9 in D minor");
        assert!(entry.is_some(), "Beethoven 9 must be in the snapshot");
        let e = entry.unwrap();
        assert_eq!(e.catalogue.as_deref(), Some("Op. 125"));
        assert!(e.editors_choice.is_some());
        let choice = e.editors_choice.unwrap();
        assert_eq!(choice.conductor.as_deref(), Some("Karajan"));
        assert!(choice.performer.contains("Berliner"));
    }

    #[test]
    fn lookup_work_handles_punctuation() {
        // MB sometimes adds a comma or quote. The matcher is loose.
        let provider = EditorialProvider::new();
        let entry = provider.lookup_work(
            BEETHOVEN,
            "Symphony No. 9 in D minor, Op. 125 \"Choral\"",
        );
        assert!(entry.is_some(), "loose match must find the seed");
    }

    #[test]
    fn lookup_work_no_match_returns_none() {
        let provider = EditorialProvider::new();
        let entry = provider.lookup_work(BEETHOVEN, "Bagatelle in G minor");
        assert!(entry.is_none());
    }

    #[test]
    fn list_picks_returns_curated_entries() {
        let provider = EditorialProvider::new();
        let picks = provider.list_picks(100);
        assert!(picks.len() >= 30, "snapshot ships ≥ 30 work picks (got {})", picks.len());
        // Each pick must have an editors_choice.
        for p in picks.iter() {
            assert!(!p.editors_choice.performer.is_empty());
        }
    }

    #[test]
    fn list_picks_caps_at_limit() {
        let provider = EditorialProvider::new();
        let small = provider.list_picks(5);
        assert_eq!(small.len(), 5);
    }

    #[test]
    fn snapshot_has_canon_coverage() {
        // Sanity check: the V1 snapshot must cover at least Beethoven,
        // Bach, Mozart, Mahler, Brahms, Glass — the canon block referenced
        // in the §11 acceptance.
        let provider = EditorialProvider::new();
        for mbid in [
            BEETHOVEN,
            BACH,
            "b972f589-fb0e-474e-b64a-803b0364fa75", // Mozart
            "8d610e51-64b4-4654-b8df-064b0fb7a9d9", // Mahler
            "c70d12a2-24fe-4f83-a6e6-57d84f8efb51", // Brahms
            "5ae54dee-4dba-49c0-802a-a3b3b3adfe9b", // Glass
        ] {
            let c = provider.lookup_composer(mbid);
            assert!(c.is_some(), "missing composer note for {mbid}");
            let works = provider.list_works_for_composer(mbid);
            assert!(!works.is_empty(), "missing works for {mbid}");
        }
    }

    #[test]
    fn lookup_work_finds_bach_goldberg() {
        // Phase 0 spike used Goldberg as canon test; ensure it's curated.
        let provider = EditorialProvider::new();
        let entry = provider.lookup_work(BACH, "Goldberg Variations");
        assert!(entry.is_some());
        let e = entry.unwrap();
        let choice = e.editors_choice.unwrap();
        assert!(choice.performer.contains("Gould"));
    }

    #[test]
    fn normalize_title_strips_punctuation() {
        assert_eq!(normalize_title("Symphony No. 9"), "symphony no 9");
        assert_eq!(
            normalize_title("Symphony No. 9 in D minor, Op. 125 \"Choral\""),
            "symphony no 9 in d minor op 125 choral"
        );
    }

    // -----------------------------------------------------------------
    // Phase 9 (B9.7) — extended schema v2 + locale fallback.
    // -----------------------------------------------------------------

    const BEETHOVEN_9_MBID: &str = "c35b4956-d4f8-321a-865b-5b13d9ed192b";
    const GOLDBERG_MBID: &str = "1d51e560-2a59-4e97-8943-13052b6adc03";
    const REQUIEM_MBID: &str = "3b11692b-cdc7-4107-9708-e5b9ee386af3";

    #[test]
    fn extended_schema_loads_three_pocs() {
        let provider = EditorialProvider::new();
        let health = provider.schema_health();
        assert!(
            health.v2_works >= 3,
            "v2 snapshot must ship at least the 3 POC works (got {})",
            health.v2_works
        );
    }

    #[test]
    fn lookup_extended_returns_beethoven_9_default_en() {
        let provider = EditorialProvider::new();
        let note = provider
            .lookup_extended(BEETHOVEN_9_MBID, None)
            .expect("Beethoven 9 must be in v2 snapshot");
        assert_eq!(note.language, "en");
        assert!(note.body.origin.is_some());
        assert!(note.body.premiere.is_some());
        assert!(note.body.highlights.is_some());
        assert!(note.body.context.is_some());
        assert!(note.body.notable_recordings_essay.is_some());
        // Sources list must be non-empty (CC BY-SA attribution).
        assert!(!note.sources.is_empty());
    }

    #[test]
    fn lookup_extended_resolves_es_translation_when_present() {
        let provider = EditorialProvider::new();
        let note = provider
            .lookup_extended(BEETHOVEN_9_MBID, Some("es"))
            .expect("Beethoven 9 must have an es translation");
        assert_eq!(note.language, "es");
        assert!(
            note.body.origin.as_deref().unwrap_or("").contains("Beethoven"),
            "es origin should mention Beethoven"
        );
    }

    #[test]
    fn lookup_extended_falls_back_to_default_when_locale_missing() {
        let provider = EditorialProvider::new();
        // 'fr' is not shipped — should fall back to 'en' rather than
        // returning None.
        let note = provider
            .lookup_extended(BEETHOVEN_9_MBID, Some("fr"))
            .expect("missing locale must fall back to default");
        assert_eq!(note.language, "en");
    }

    #[test]
    fn lookup_extended_returns_none_for_unknown_work() {
        let provider = EditorialProvider::new();
        let note =
            provider.lookup_extended("00000000-0000-0000-0000-000000000000", None);
        assert!(note.is_none());
    }

    #[test]
    fn lookup_extended_by_title_resolves_via_composer_match() {
        let provider = EditorialProvider::new();
        // Mozart Requiem — composer MBID + canonical title.
        let note = provider.lookup_extended_by_title(
            "b972f589-fb0e-474e-b64a-803b0364fa75",
            "Requiem in D minor",
            None,
        );
        assert!(note.is_some(), "Mozart Requiem must resolve by title");
    }

    #[test]
    fn lookup_extended_by_title_handles_loose_match() {
        let provider = EditorialProvider::new();
        // Bach Goldberg — partial title that should still match.
        let note = provider.lookup_extended_by_title(
            "24f1766e-9635-4d58-a4d4-9413f9f98a4c",
            "Goldberg Variations, BWV 988",
            None,
        );
        assert!(note.is_some(), "loose title match must find Goldberg");
    }

    #[test]
    fn lookup_bucket_extended_v2_overrides_heuristic() {
        let provider = EditorialProvider::new();
        // Mozart Requiem — v2 snapshot pins bucket=ChoralSacred.
        let bucket = provider.lookup_bucket(
            "b972f589-fb0e-474e-b64a-803b0364fa75",
            "Requiem in D minor",
        );
        assert_eq!(bucket, Some(super::WorkBucket::ChoralSacred));
    }

    #[test]
    fn poc_works_have_all_five_subsections() {
        let provider = EditorialProvider::new();
        for mbid in [BEETHOVEN_9_MBID, GOLDBERG_MBID, REQUIEM_MBID] {
            let note = provider
                .lookup_extended(mbid, None)
                .unwrap_or_else(|| panic!("POC {mbid} missing from v2"));
            assert!(note.body.origin.is_some(), "{mbid} missing origin");
            assert!(note.body.premiere.is_some(), "{mbid} missing premiere");
            assert!(note.body.highlights.is_some(), "{mbid} missing highlights");
            assert!(note.body.context.is_some(), "{mbid} missing context");
            assert!(
                note.body.notable_recordings_essay.is_some(),
                "{mbid} missing notable_recordings_essay"
            );
        }
    }
}
