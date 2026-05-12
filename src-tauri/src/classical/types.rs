//! Domain types for the Classical Hub. Mirrored 1:1 in
//! `src/types/classical.ts` so the backend and frontend agree on shapes.
//!
//! These types intentionally do NOT leak provider-specific fields (no
//! MusicBrainz `recording-rels` arrays, no Tidal raw `mediaMetadata`).
//! Each `ClassicalProvider` knows how to fill subsets of these structs;
//! the `CatalogService` is the only thing that merges them.
//!
//! Reference: CLASSICAL_DESIGN.md §5.1 (entity model).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums shared with the frontend (`#[serde(rename_all = "PascalCase")]` is
// avoided on purpose — the frontend uses literal string unions like
// 'Symphony' | 'Concerto', so `Display`-style PascalCase already matches).
// ---------------------------------------------------------------------------

/// Coarse era buckets used for browse axes. Sourced primarily from
/// OpenOpus.epoch with Wikidata fallback. The labels are stable
/// identifiers — never localize them in storage; localize at render time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Era {
    Medieval,
    Renaissance,
    Baroque,
    Classical,
    EarlyRomantic,
    Romantic,
    LateRomantic,
    TwentiethCentury,
    PostWar,
    Contemporary,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkType {
    Symphony,
    Concerto,
    Sonata,
    StringQuartet,
    Opera,
    Cantata,
    Mass,
    Lieder,
    Suite,
    Etude,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Genre {
    Orchestral,
    Concerto,
    Chamber,
    SoloInstrumental,
    Vocal,
    Choral,
    Opera,
    Sacred,
    Stage,
    Film,
    Other,
}

/// Phase 9 (D-039 + D-040) — taxonomy used by ComposerPage to group works
/// into the 9+2 buckets that mirror the editorial conventions of Apple
/// Music Classical and Idagio. This is a *presentation tier*: it does
/// not replace `WorkType` (which mirrors MusicBrainz `work-type`) nor
/// `Genre` (which mirrors Wikidata P136). The bucket is computed by
/// `classical::buckets::bucket_for(...)` from the data tier and cached
/// on the `Work` so list views never recompute.
///
/// Order of variants is the order of presentation in the ComposerPage:
/// Stage > Choral & sacred > Vocal > Symphonies > Concertos >
/// Orchestral > Chamber > Keyboard > Solo instrumental, then the two
/// conditional buckets Film & theatre and Other.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkBucket {
    Stage,
    ChoralSacred,
    Vocal,
    Symphonies,
    Concertos,
    Orchestral,
    Chamber,
    Keyboard,
    SoloInstrumental,
    FilmTheatre,
    Other,
}

impl WorkBucket {
    /// Parse the canonical PascalCase string the frontend sends back.
    /// Used by Tauri commands that take a bucket as a query parameter
    /// (e.g. `list_classical_works_in_bucket`).
    pub fn parse_literal(s: &str) -> Option<Self> {
        match s {
            "Stage" => Some(WorkBucket::Stage),
            "ChoralSacred" => Some(WorkBucket::ChoralSacred),
            "Vocal" => Some(WorkBucket::Vocal),
            "Symphonies" => Some(WorkBucket::Symphonies),
            "Concertos" => Some(WorkBucket::Concertos),
            "Orchestral" => Some(WorkBucket::Orchestral),
            "Chamber" => Some(WorkBucket::Chamber),
            "Keyboard" => Some(WorkBucket::Keyboard),
            "SoloInstrumental" => Some(WorkBucket::SoloInstrumental),
            "FilmTheatre" => Some(WorkBucket::FilmTheatre),
            "Other" => Some(WorkBucket::Other),
            _ => None,
        }
    }

    /// Canonical presentation order — used to sort buckets in the
    /// ComposerPage Works tab and the `list_classical_composer_buckets`
    /// command response.
    pub fn presentation_order(self) -> u8 {
        match self {
            WorkBucket::Stage => 0,
            WorkBucket::ChoralSacred => 1,
            WorkBucket::Vocal => 2,
            WorkBucket::Symphonies => 3,
            WorkBucket::Concertos => 4,
            WorkBucket::Orchestral => 5,
            WorkBucket::Chamber => 6,
            WorkBucket::Keyboard => 7,
            WorkBucket::SoloInstrumental => 8,
            WorkBucket::FilmTheatre => 9,
            WorkBucket::Other => 10,
        }
    }

    /// Human label in English, ready to render. Spanish lives in the
    /// frontend (`src/types/classical.ts`) since the UI does locale
    /// resolution there.
    pub fn label_en(self) -> &'static str {
        match self {
            WorkBucket::Stage => "Stage works",
            WorkBucket::ChoralSacred => "Choral & sacred",
            WorkBucket::Vocal => "Vocal",
            WorkBucket::Symphonies => "Symphonies",
            WorkBucket::Concertos => "Concertos",
            WorkBucket::Orchestral => "Orchestral",
            WorkBucket::Chamber => "Chamber",
            WorkBucket::Keyboard => "Keyboard",
            WorkBucket::SoloInstrumental => "Solo instrumental",
            WorkBucket::FilmTheatre => "Film & theatre",
            WorkBucket::Other => "Other",
        }
    }

    /// Human label in Spanish — paired with `label_en` for backend
    /// responses that ship both, so the frontend doesn't have to do
    /// the lookup. Cheap; <80 bytes.
    pub fn label_es(self) -> &'static str {
        match self {
            WorkBucket::Stage => "Obras escénicas",
            WorkBucket::ChoralSacred => "Coral y sacro",
            WorkBucket::Vocal => "Vocal",
            WorkBucket::Symphonies => "Sinfonías",
            WorkBucket::Concertos => "Conciertos",
            WorkBucket::Orchestral => "Orquestal",
            WorkBucket::Chamber => "Cámara",
            WorkBucket::Keyboard => "Teclado",
            WorkBucket::SoloInstrumental => "Instrumento solo",
            WorkBucket::FilmTheatre => "Cine y teatro",
            WorkBucket::Other => "Otros",
        }
    }
}

/// Confidence tiers for the cascade matching introduced by D-010
/// (extended by D-037 — work-level Tidal direct inference).
///
/// - `IsrcBound`: MB exposed an ISRC and Tidal accepted it. Deterministic.
/// - `TextSearchInferred`: MB browse returned a recording, Tidal text
///   search resolved it, score ≥ INFERRED_THRESHOLD (0.6). Heuristic.
/// - `TidalDirectInferred` (D-037): MB had no recordings for the work
///   *or* none crossed the per-recording threshold. We ran a work-level
///   Tidal search (composer + work title, no artist constraint) and the
///   top result crossed `WORK_LEVEL_THRESHOLD` (0.55). Lower confidence
///   than `TextSearchInferred`; the UI shows the query used at hover.
/// - `NotFound`: shown as info-only with no play button.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum MatchConfidence {
    IsrcBound,
    TextSearchInferred,
    TidalDirectInferred,
    #[default]
    NotFound,
}

// ---------------------------------------------------------------------------
// Performer credits
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformerCredit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbid: Option<String>,
    pub name: String,
    /// 'person' | 'group' | 'orchestra' | 'choir' | 'ensemble'
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformerCreditWithRole {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbid: Option<String>,
    pub name: String,
    pub kind: String,
    /// "violin" | "soprano" | "piano" | ...
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_mbid: Option<String>,
}

// ---------------------------------------------------------------------------
// Catalogue numbers (BWV 1052, K. 466, Op. 125, etc.)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueNumber {
    /// 'BWV' | 'K' | 'D' | 'RV' | 'Hob' | 'HWV' | 'Op' | 'Other'
    pub system: String,
    /// Pure number, e.g. "1052", "466".
    pub number: String,
    /// Display form, e.g. "BWV 1052", "Op. 125".
    pub display: String,
}

// ---------------------------------------------------------------------------
// Composer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeEvent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Composer {
    /// MusicBrainz artist MBID. Considered the canonical identity.
    pub mbid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_opus_id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birth: Option<LifeEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death: Option<LifeEvent>,
    #[serde(default = "default_unknown_era")]
    pub era: Era,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio_short: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio_long: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio_source_url: Option<String>,
    /// Phase 5 (D-020) — editorial blurb sourced from
    /// `data/editorial.json`. None when this composer is outside the
    /// curated snapshot. Frontend renders inside the hero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_note: Option<String>,
    /// Phase 6 (D-022) — related composers surfaced via Wikidata
    /// genre overlap (P136) + birth-year proximity. Empty when
    /// Wikidata returned no match (no QID, query failure, no genre
    /// overlap). The frontend renders the section as "Related
    /// composers" in the ComposerPage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_composers: Vec<RelatedComposer>,
}

/// Phase 6 (D-022) — a "related composers" list entry on the Composer
/// page. The QID is the Wikidata identifier, the MB MBID is best-effort
/// (MB → Wikidata edges aren't always bidirectional). The portrait URL
/// is direct from Wikidata Commons.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedComposer {
    pub qid: String,
    /// MusicBrainz MBID when we could resolve it from Wikidata's
    /// MusicBrainz-artist-id property (P434). When empty the UI
    /// surfaces the entry but cannot navigate to the Composer page.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mbid: String,
    pub name: String,
    /// Wikidata genre QIDs that this composer shares with the parent.
    /// Surfaced as a tooltip "shared: opera, oratorio".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_genres: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birth_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait_url: Option<String>,
}

fn default_unknown_era() -> Era {
    Era::Unknown
}

impl Era {
    /// Parse a serialized Era literal back into the enum. Used by Tauri
    /// commands that receive an Era as a string payload from the frontend.
    /// Case-sensitive — the frontend always sends the canonical PascalCase
    /// names defined in `src/types/classical.ts`.
    ///
    /// Named `parse_literal` (not `from_str`) so it doesn't collide with
    /// the `std::str::FromStr` trait — we don't want classical Era to
    /// accidentally pick up parser semantics from third-party crates.
    pub fn parse_literal(s: &str) -> Option<Self> {
        match s {
            "Medieval" => Some(Era::Medieval),
            "Renaissance" => Some(Era::Renaissance),
            "Baroque" => Some(Era::Baroque),
            "Classical" => Some(Era::Classical),
            "EarlyRomantic" => Some(Era::EarlyRomantic),
            "Romantic" => Some(Era::Romantic),
            "LateRomantic" => Some(Era::LateRomantic),
            "TwentiethCentury" => Some(Era::TwentiethCentury),
            "PostWar" => Some(Era::PostWar),
            "Contemporary" => Some(Era::Contemporary),
            "Unknown" => Some(Era::Unknown),
            _ => None,
        }
    }
}

impl Genre {
    /// Parse a serialized Genre literal back into the enum. Same shape as
    /// `Era::parse_literal`.
    pub fn parse_literal(s: &str) -> Option<Self> {
        match s {
            "Orchestral" => Some(Genre::Orchestral),
            "Concerto" => Some(Genre::Concerto),
            "Chamber" => Some(Genre::Chamber),
            "SoloInstrumental" => Some(Genre::SoloInstrumental),
            "Vocal" => Some(Genre::Vocal),
            "Choral" => Some(Genre::Choral),
            "Opera" => Some(Genre::Opera),
            "Sacred" => Some(Genre::Sacred),
            "Stage" => Some(Genre::Stage),
            "Film" => Some(Genre::Film),
            "Other" => Some(Genre::Other),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Movement (sub-work)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Movement {
    pub mbid: String,
    /// 1-based index for display.
    pub index: u32,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_approx_secs: Option<u32>,
    /// Index of the next movement when there's an attacca. Used by the
    /// player gapless guard (Phase 3); resolved by editorial layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attacca_to: Option<u32>,
}

// ---------------------------------------------------------------------------
// Work
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub mbid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qid: Option<String>,
    pub title: String,
    /// Composer this work is attributed to. Resolved from MB
    /// composer-rels at fetch time. May be empty until enrichment runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_mbid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_name: Option<String>,
    #[serde(default)]
    pub alternative_titles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogue_number: Option<CatalogueNumber>,
    /// "D minor", "A major", etc. Free-form because Wikidata + MB use
    /// different conventions; the frontend renders verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<Genre>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_type: Option<WorkType>,
    /// Phase 9 (D-040) — presentation bucket computed from
    /// `(work_type, genre, p136_keywords, title)` by
    /// `classical::buckets::bucket_for(...)`. Cached on the work so
    /// list views avoid the recomputation. `None` only for legacy
    /// cached payloads pre-Phase 9; the catalog backfills on the
    /// next fresh build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<WorkBucket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premiere_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_approx_secs: Option<u32>,
    #[serde(default)]
    pub movements: Vec<Movement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_source_url: Option<String>,
    /// Recordings discovered for this work. May be empty on the first
    /// shell response; the catalog hydrates this asynchronously.
    #[serde(default)]
    pub recordings: Vec<Recording>,
    /// Total number reported by MB before truncation. Lets the UI show
    /// "showing N of M".
    #[serde(default)]
    pub recording_count: u32,
    /// "Best available across recordings" — populated by the Phase 4
    /// quality aggregator. `None` when no recording has Tidal metadata
    /// yet (cold cache pre-refinement).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_available_quality: Option<BestAvailableQuality>,
    /// Phase 5 (D-020) — 1-3 sentence editorial blurb from the curated
    /// snapshot (`data/editorial.json`). `None` when this work is outside
    /// the curated set. Frontend renders inside the WorkPage header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_note: Option<String>,
    /// Phase 7 (D-030) — `true` when the cascade ISRC + Tidal text
    /// search produced zero playable recordings on the last fresh
    /// fetch. The frontend renders a "No recordings on Tidal yet"
    /// banner with a Re-check CTA. Cached for 7d (Dynamic tier);
    /// expires on its own and can be force-refreshed via the
    /// `recheck_classical_work_tidal` command.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tidal_unavailable: bool,
}

// ---------------------------------------------------------------------------
// Best-available quality summary (Phase 4 — D-018)
// ---------------------------------------------------------------------------

/// Compact shape rendered as the "Best available" banner in the WorkPage
/// header. Carries the Tidal tier label + the refined sample rate / bit
/// depth when known. Mirrors `BestAvailableQuality` in
/// `src/types/classical.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BestAvailableQuality {
    /// Primary tier: "HIRES_LOSSLESS" | "LOSSLESS" | "DOLBY_ATMOS" | "MQA" | "HIGH".
    pub tier: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u8>,
    /// Whether at least one recording carries the DOLBY_ATMOS audio mode.
    /// Independent from `tier` so UIs that want to show both badges can.
    #[serde(default)]
    pub has_atmos: bool,
}

impl Work {
    /// Empty Work used as the starting point of an enrichment pass.
    pub fn skeleton(mbid: &str) -> Self {
        Self {
            mbid: mbid.to_string(),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub mbid: String,
    /// MBID of the parent work this recording realises. Same recording
    /// can appear under multiple works in MB, but for the Hub we always
    /// fetch in the context of a single work.
    pub work_mbid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conductor: Option<PerformerCredit>,
    #[serde(default)]
    pub orchestras: Vec<PerformerCredit>,
    #[serde(default)]
    pub soloists: Vec<PerformerCreditWithRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ensemble: Option<PerformerCredit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choir: Option<PerformerCredit>,
    /// Year extracted from the earliest associated release date when MB
    /// doesn't explicitly carry the recording date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,
    /// Label name from earliest release. May be inaccurate for compilations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub isrcs: Vec<String>,
    /// Artists credited on the MB recording (raw artist-credit blob,
    /// flattened to a string). Used as the default display when the
    /// conductor/orchestra split has not been resolved yet.
    #[serde(default)]
    pub artist_credits: Vec<String>,
    /// Cover image URL, when known. Backfilled from CAA or Tidal album art.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    // ---- Playback bridge ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tidal_track_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tidal_album_id: Option<u64>,
    #[serde(default)]
    pub audio_quality_tags: Vec<String>,
    #[serde(default)]
    pub audio_modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u32>,
    // ---- Quality refinement (Phase 4 — D-017) ----
    /// Effective sample rate (Hz) reported by the Tidal manifest metadata
    /// path. `None` until the catalog has fetched the per-track detail
    /// (cap top-20 by work). The backend writes Hz exactly as Tidal
    /// returns it (e.g. 44100, 48000, 96000, 192000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
    /// Effective bit depth (16 or 24, occasionally 32 for studio masters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<u8>,
    /// Numeric score for sort/aggregation. Computed by `classical::quality`
    /// at catalog build time. Higher is better. 0 when no playable Tidal
    /// match.
    #[serde(default)]
    pub quality_score: u32,
    // ---- Editorial (Phase 5 — D-020 + D-021) ----
    /// Phase 5 — `true` when this recording is the (snapshot or user)
    /// Editor's Choice for the parent work. The UI renders a star
    /// indicator + tooltip with the rationale.
    #[serde(default)]
    pub is_editors_choice: bool,
    /// Phase 5 — short rationale shown in the star tooltip ("Karajan/BPO
    /// 1962 · Gramophone Hall of Fame"). Empty when the recording is
    /// not the Editor's Choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_note: Option<String>,
    // ---- Match metadata (D-010) ----
    pub match_confidence: MatchConfidence,
    /// For text-search matches: the canonical query that produced the
    /// hit, so the UI can show it on hover. None for ISRC-bound and NotFound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_score: Option<f64>,
}

impl Recording {
    pub fn shell(mbid: &str, work_mbid: &str) -> Self {
        Self {
            mbid: mbid.to_string(),
            work_mbid: work_mbid.to_string(),
            title: None,
            conductor: None,
            orchestras: Vec::new(),
            soloists: Vec::new(),
            ensemble: None,
            choir: None,
            recording_year: None,
            recording_date: None,
            venue: None,
            label: None,
            isrcs: Vec::new(),
            artist_credits: Vec::new(),
            cover_url: None,
            tidal_track_id: None,
            tidal_album_id: None,
            audio_quality_tags: Vec::new(),
            audio_modes: Vec::new(),
            duration_secs: None,
            sample_rate_hz: None,
            bit_depth: None,
            quality_score: 0,
            is_editors_choice: false,
            editor_note: None,
            match_confidence: MatchConfidence::NotFound,
            match_query: None,
            match_score: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2 — browse summaries
// ---------------------------------------------------------------------------
//
// `ComposerSummary` and `WorkSummary` are intentionally small projections of
// `Composer` and `Work`. They populate Hub grid/list views (composer cards,
// work cards) without forcing the catalog to fetch the full entity each time.
// The Phase 1 detail types stay authoritative — these are derived shapes that
// any provider can fill from a snapshot or a lightweight MB browse.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposerSummary {
    pub mbid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_opus_id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birth_year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death_year: Option<i32>,
    pub era: Era,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait_url: Option<String>,
    /// `true` when OpenOpus marks this composer as "popular" — used by the
    /// Hub featured section without having to query the snapshot again.
    #[serde(default)]
    pub popular: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSummary {
    pub mbid: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_mbid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalogue_number: Option<CatalogueNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_type: Option<WorkType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<Genre>,
    /// Phase 9 (D-040) — same cached bucket as `Work.bucket`. Allowed
    /// to be `None` on legacy cached payloads; consumers that need it
    /// (the new ComposerPage Works tab) call
    /// `bucket_for(...)` as a fallback when the field is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<WorkBucket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition_year: Option<i32>,
    /// Mirrors OpenOpus' `popular` flag when known; the MB browse fallback
    /// can't infer popularity, so it stays `false` for non-snapshot composers.
    #[serde(default)]
    pub popular: bool,
}
