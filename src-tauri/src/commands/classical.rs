//! Tauri commands exposed to the frontend for the Classical Hub.
//!
//! All three commands are read-only catalog lookups: they do not touch
//! audio routing, volume, scrobbling, or any other live state. They are
//! aditive Tauri handlers — registered next to the rest in `lib.rs::run`.

use tauri::State;

use crate::classical::catalog::ArtistDiscography;
use crate::classical::editorial::EditorialPick;
use crate::classical::listening_guide::{self, LrcGuide};
use crate::classical::search::SearchResults;
use crate::classical::{
    movement::MovementContext, Composer, ComposerSummary, Era, Genre, Recording, RelatedComposer,
    Work,
};
use crate::stats::{
    ClassicalFavorite, ClassicalOverview, DiscoveryPoint, RecentClassicalSession,
    RecordingComparisonRow, StatsWindow, TopClassicalComposer, TopClassicalWork,
};
use crate::AppState;
use crate::SoneError;

/// Resolve a full `Work` entity (with description + recordings + bound
/// Tidal track ids) by MBID. Cached for 7 days (StaticMeta tier); SWR
/// allows up to 30 days of stale data while a refresh runs in the
/// background (Phase 4 will wire the background refresh).
#[tauri::command]
pub async fn get_classical_work(
    state: State<'_, AppState>,
    mbid: String,
) -> Result<Work, SoneError> {
    state.classical.get_work(&mbid).await
}

/// Resolve a single `Recording` with conductor / orchestra / soloist
/// detail. Used by hover/click expansion in the work page; the
/// list-level data already comes batched from `get_classical_work`.
#[tauri::command]
pub async fn get_classical_recording(
    state: State<'_, AppState>,
    mbid: String,
    work_mbid: String,
) -> Result<Recording, SoneError> {
    state.classical.get_recording(&mbid, &work_mbid).await
}

/// Resolve a `Composer` by MBID. Phase 1 placeholder — returns name +
/// dates + bio. Phase 2 will hydrate work groupings.
#[tauri::command]
pub async fn get_classical_composer(
    state: State<'_, AppState>,
    mbid: String,
) -> Result<Composer, SoneError> {
    state.classical.get_composer(&mbid).await
}

/// Given a recording MBID, find the parent Work MBID for the "View work"
/// button in the player. Returns `None` if MB doesn't have the link or
/// the recording isn't classical.
#[tauri::command]
pub async fn resolve_classical_work_for_recording(
    state: State<'_, AppState>,
    recording_mbid: String,
) -> Result<Option<String>, SoneError> {
    state
        .classical
        .resolve_work_for_recording(&recording_mbid)
        .await
}

// ---------------------------------------------------------------------------
// Phase 2 — browse commands
// ---------------------------------------------------------------------------

/// Top-N classical composers from the OpenOpus snapshot. Synchronous on
/// the Rust side (no I/O) — wrapped in `async fn` only because Tauri's
/// command machinery prefers a single shape for return types.
#[tauri::command]
pub async fn list_classical_top_composers(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<ComposerSummary>, SoneError> {
    let limit = limit.max(1) as usize;
    Ok(state.classical.list_top_composers(limit))
}

/// Composers in a given era. The frontend sends an Era literal string
/// (PascalCase) and we map it back via `Era::from_str`.
#[tauri::command]
pub async fn list_classical_composers_by_era(
    state: State<'_, AppState>,
    era: String,
) -> Result<Vec<ComposerSummary>, SoneError> {
    let parsed = Era::parse_literal(&era)
        .ok_or_else(|| SoneError::Parse(format!("unknown era: {era}")))?;
    Ok(state.classical.list_composers_by_era(parsed))
}

// ---------------------------------------------------------------------------
// Phase 3 — movement context
// ---------------------------------------------------------------------------

/// Phase 3 (B3.2): resolve which movement of a work the current track
/// represents, so the player can render the "II / IV" indicator + the
/// "Attacca →" hint.
///
/// `track_title` is whatever Tidal exposed for the current track (typically
/// "II. Molto vivace" or "Aria"). `album_position` is the 1-based track
/// number within the Tidal album, used as a last-resort fallback when
/// title matching fails.
///
/// Returns `None` when the work has no movements parsed (single-movement
/// piece) or when no heuristic matches.
#[tauri::command]
pub async fn resolve_classical_movement(
    state: State<'_, AppState>,
    work_mbid: String,
    track_title: String,
    album_position: Option<u32>,
) -> Result<Option<MovementContext>, SoneError> {
    state
        .classical
        .resolve_movement(&work_mbid, &track_title, album_position)
        .await
}

// ---------------------------------------------------------------------------
// Phase 4 — quality refinement
// ---------------------------------------------------------------------------

/// Phase 4 (B4.3): force the catalog to drop its cached `Work` and the
/// per-track quality probes for this work, then rebuild. Returns the
/// freshly-rebuilt `Work` so the UI can update without an extra round
/// trip.
///
/// Use case: the user opened a WorkPage from cold cache and only a
/// subset of recordings showed refined `sample_rate_hz`. They click
/// "Refresh quality" → the backend re-probes top-N recordings with
/// `playbackinfopostpaywall` and fills the gaps.
#[tauri::command]
pub async fn refresh_classical_work_qualities(
    state: State<'_, AppState>,
    work_mbid: String,
) -> Result<Work, SoneError> {
    state
        .classical
        .refresh_work_recording_qualities(&work_mbid)
        .await
}

/// Phase 7 (D-030) — force the catalog to drop its cached `Work`, then
/// re-run the cascade (ISRC + Tidal text-search) from scratch. Used by
/// the "Re-check Tidal" CTA on works that previously came back with
/// `tidal_unavailable=true`. Returns the rebuilt `Work` so the UI knows
/// whether the second attempt found recordings or the banner stays.
#[tauri::command]
pub async fn recheck_classical_work_tidal(
    state: State<'_, AppState>,
    work_mbid: String,
) -> Result<Work, SoneError> {
    state
        .classical
        .refresh_work_recording_qualities(&work_mbid)
        .await
}

/// Phase 7 (F7.3) — total composers in the extended snapshot. Surfaced
/// by the Hub home footer chip ("Catalog: X composers indexed").
/// Synchronous: snapshot is in-process so we just read the OnceLock.
#[tauri::command]
pub fn get_classical_extended_total(state: State<'_, AppState>) -> u32 {
    state.classical.extended_composers_total() as u32
}

/// Works for a composer with optional Genre filter, paginated.
///
/// Phase 7 (D-029) — `offset` is optional (defaults to 0). The first
/// page (offset=0) drives the initial render; subsequent pages
/// (offset=100, 200, ...) power the "Load more" affordance for
/// composers with > 100 works (Bach, Mozart). Cached for 7d per
/// `(mbid, genre, offset)` triple.
#[tauri::command]
pub async fn list_classical_works_by_composer(
    state: State<'_, AppState>,
    composer_mbid: String,
    genre: Option<String>,
    offset: Option<u32>,
) -> Result<crate::classical::catalog::ComposerWorksPage, SoneError> {
    let parsed_genre = match genre.as_deref() {
        Some(g) => Some(
            Genre::parse_literal(g)
                .ok_or_else(|| SoneError::Parse(format!("unknown genre: {g}")))?,
        ),
        None => None,
    };
    state
        .classical
        .list_works_by_composer(&composer_mbid, parsed_genre, offset.unwrap_or(0))
        .await
}

// ---------------------------------------------------------------------------
// Phase 9 (B9.3 / B9.4 / D-040 / D-041) — bucketed composer view.
// ---------------------------------------------------------------------------

/// Phase 9 (B9.3) — bucketed view of a composer's catalogue. Each
/// `BucketSummary` carries a top-12 + optional sub-bucket breakdown
/// for the ComposerPage Works tab. Multi-page MB browse on cold cache
/// (Bach 11 pages × 1.05s ≈ 11s); 7d StaticMeta cached afterwards.
#[tauri::command]
pub async fn list_classical_composer_buckets(
    state: State<'_, AppState>,
    composer_mbid: String,
) -> Result<crate::classical::catalog::ComposerBuckets, SoneError> {
    state
        .classical
        .list_classical_composer_buckets(&composer_mbid)
        .await
}

/// Phase 9 (B9.6 / D-044) — fetch the extended editorial note for a
/// work, with locale fallback handled server-side. Returns `None` (a
/// `null` JSON value) when the work is outside the v2 snapshot — the
/// frontend then falls back to the Phase 5 `editor_note` and the
/// Wikipedia summary as before.
///
/// `locale` is an optional ISO tag ("es", "en"). When omitted the
/// default-language body (typically "en") is returned.
#[tauri::command]
pub async fn get_classical_extended_note(
    state: State<'_, AppState>,
    work_mbid: String,
    locale: Option<String>,
) -> Result<Option<crate::classical::editorial::ExtendedNote>, SoneError> {
    let _ = state;
    let provider = crate::classical::editorial::EditorialProvider::new();
    Ok(provider.lookup_extended(&work_mbid, locale.as_deref()))
}

/// Phase 9 (B9.4) — drill-down into a single bucket. Operates on the
/// cached `composer_buckets:v1:{mbid}` payload + a sibling
/// `bucket-full:v1` cache to avoid re-fetching MB on every navigation.
/// `sort` accepts "Catalog" (default) | "Date" | "Alphabetical".
/// `subBucket` is the literal label produced by the backend's
/// `compute_sub_buckets` (e.g. "Piano", "Quartets", "Études").
#[tauri::command]
pub async fn list_classical_works_in_bucket(
    state: State<'_, AppState>,
    composer_mbid: String,
    bucket: String,
    sub_bucket: Option<String>,
    sort: Option<String>,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<crate::classical::catalog::WorksPage, SoneError> {
    let parsed_bucket = crate::classical::types::WorkBucket::parse_literal(&bucket)
        .ok_or_else(|| SoneError::Parse(format!("unknown bucket: {bucket}")))?;
    state
        .classical
        .list_classical_works_in_bucket(
            &composer_mbid,
            parsed_bucket,
            sub_bucket.as_deref(),
            sort.as_deref(),
            offset.unwrap_or(0),
            limit.unwrap_or(50),
        )
        .await
}

// ---------------------------------------------------------------------------
// Phase 5 — search + editorial + listening guides
// ---------------------------------------------------------------------------

/// Phase 5 (B5.1): tokenized + planned classical search. The frontend
/// receives both the resolved `SearchPlan` (so it can render "Detected:
/// composer:Beethoven · year:1962" chips) and the ranked `SearchHit`
/// list. Default `limit` is 20 — the UI caps below that for the visible
/// list and shows the rest behind a "show more" affordance.
#[tauri::command]
pub async fn search_classical(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<SearchResults, SoneError> {
    let lim = limit.unwrap_or(20).clamp(1, 50) as usize;
    state.classical.search_classical(&query, lim).await
}

/// Phase 5 (B5.2): list curated Editor's Choice picks for the Hub home
/// grid. Synchronous on the backend (snapshot in-process), so this round
/// trip is dominated by IPC.
#[tauri::command]
pub async fn list_classical_editorial_picks(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<EditorialPick>, SoneError> {
    let lim = limit.unwrap_or(12).clamp(1, 100) as usize;
    Ok(state.classical.list_editorial_picks(lim))
}

/// Phase 5 (D-021): persist a user-set Editor's Choice for a work.
/// Subsequent reads of the work surface this recording with the star
/// indicator + the optional note.
#[tauri::command]
pub async fn set_classical_editors_choice(
    state: State<'_, AppState>,
    work_mbid: String,
    recording_mbid: String,
    note: Option<String>,
) -> Result<(), SoneError> {
    state
        .classical
        .set_user_editors_choice(&work_mbid, &recording_mbid, note.as_deref())
        .await
}

/// Phase 5 (D-021): clear a user override. The work falls back to the
/// embedded snapshot pick (or none, when the work is outside the canon
/// curation).
#[tauri::command]
pub async fn clear_classical_editors_choice(
    state: State<'_, AppState>,
    work_mbid: String,
) -> Result<(), SoneError> {
    state.classical.clear_user_editors_choice(&work_mbid).await
}

/// Phase 5 (B5.5): read a community-authored listening guide for a
/// work, when present at `~/.config/sone/listening-guides/{mbid}.lrc`.
/// Returns `None` when no file exists. Errors only on filesystem
/// failures (e.g. permission denied).
#[tauri::command]
pub async fn read_classical_listening_guide(
    work_mbid: String,
) -> Result<Option<LrcGuide>, SoneError> {
    listening_guide::read_guide(&work_mbid)
}

// ---------------------------------------------------------------------------
// Phase 6 — personal listening integration + Wikidata + browse-by-conductor
// ---------------------------------------------------------------------------

/// Phase 6 (B6.1): leaderboard of the user's most-played works in the
/// given window. Aggregates by `work_mbid` so all movements collapse
/// into one row. Returns up to `limit` (clamped to [1, 100]).
#[tauri::command]
pub async fn list_top_classical_works(
    state: State<'_, AppState>,
    window: StatsWindow,
    limit: Option<u32>,
) -> Result<Vec<TopClassicalWork>, SoneError> {
    let lim = limit.unwrap_or(20).clamp(1, 100);
    state.classical.top_classical_works(window, lim)
}

/// Phase 6 (B6.1): leaderboard of the user's most-played composers
/// (filtered to plays whose parent work was resolved as classical).
#[tauri::command]
pub async fn list_top_classical_composers(
    state: State<'_, AppState>,
    window: StatsWindow,
    limit: Option<u32>,
) -> Result<Vec<TopClassicalComposer>, SoneError> {
    let lim = limit.unwrap_or(20).clamp(1, 100);
    state.classical.top_classical_composers(window, lim)
}

/// Phase 6 (B6.1): aggregate footprint of classical listening for the
/// "X works · Y composers · Z hours" badge in the Hub Library hero.
#[tauri::command]
pub async fn get_classical_overview(
    state: State<'_, AppState>,
    window: StatsWindow,
) -> Result<ClassicalOverview, SoneError> {
    state.classical.classical_overview(window)
}

/// Phase 6 (B6.2): classical-only discovery curve. Same shape as the
/// global discovery curve so the frontend reuses the chart component.
#[tauri::command]
pub async fn get_classical_discovery_curve(
    state: State<'_, AppState>,
    window: StatsWindow,
) -> Result<Vec<DiscoveryPoint>, SoneError> {
    state.classical.classical_discovery_curve(window)
}

/// Phase 6 (B6.1): recently-played classical sessions, grouped by
/// `work_mbid`. `windowSecs` defaults to 7 days, `limit` to 20.
#[tauri::command]
pub async fn list_recent_classical_sessions(
    state: State<'_, AppState>,
    window_secs: Option<i64>,
    limit: Option<u32>,
) -> Result<Vec<RecentClassicalSession>, SoneError> {
    let window = window_secs.unwrap_or(7 * 24 * 3600);
    let lim = limit.unwrap_or(20).clamp(1, 100);
    state
        .classical
        .classical_recently_played_works(window, lim)
}

/// Phase 6 (B6.3): recording comparison rows for a single work. Use
/// case: the user has played 3 versions of Beethoven 9; this returns
/// each with their per-recording play counts and completion rate.
#[tauri::command]
pub async fn list_classical_recording_comparison(
    state: State<'_, AppState>,
    work_mbid: String,
) -> Result<Vec<RecordingComparisonRow>, SoneError> {
    state.classical.classical_recording_comparison(&work_mbid)
}

// --- Favorites CRUD (B6.4) -------------------------------------------------

/// Phase 6 (B6.4): persist a saved entity. `kind` ∈ {"work",
/// "recording", "composer", "performer"}. Idempotent: a duplicate add
/// is a no-op. The frontend reflects via `is_classical_favorite`.
#[tauri::command]
pub async fn add_classical_favorite(
    state: State<'_, AppState>,
    kind: String,
    mbid: String,
    display_name: String,
) -> Result<(), SoneError> {
    state
        .classical
        .add_classical_favorite(&kind, &mbid, &display_name)
}

#[tauri::command]
pub async fn remove_classical_favorite(
    state: State<'_, AppState>,
    kind: String,
    mbid: String,
) -> Result<(), SoneError> {
    state.classical.remove_classical_favorite(&kind, &mbid)
}

#[tauri::command]
pub async fn is_classical_favorite(
    state: State<'_, AppState>,
    kind: String,
    mbid: String,
) -> Result<bool, SoneError> {
    state.classical.is_classical_favorite(&kind, &mbid)
}

#[tauri::command]
pub async fn list_classical_favorites(
    state: State<'_, AppState>,
    kind: String,
    limit: Option<u32>,
) -> Result<Vec<ClassicalFavorite>, SoneError> {
    let lim = limit.unwrap_or(50).clamp(1, 500);
    state.classical.list_classical_favorites(&kind, lim)
}

// --- Related composers + browse-by-conductor (D-022) ------------------------

/// Phase 6 (D-022): list related composers for a Composer MBID. Uses
/// Wikidata SPARQL; returns empty when WD has no QID, no genres, or
/// the query failed (UI degrades gracefully).
#[tauri::command]
pub async fn list_classical_related_composers(
    state: State<'_, AppState>,
    composer_mbid: String,
) -> Result<Vec<RelatedComposer>, SoneError> {
    state
        .classical
        .list_related_composers(&composer_mbid)
        .await
}

/// Phase 6 (D-022): discography landing for a conductor / orchestra
/// (or any artist MBID). Returns the flat recordings list + a grouped
/// view ("5 versions of Beethoven 9").
#[tauri::command]
pub async fn get_classical_artist_discography(
    state: State<'_, AppState>,
    artist_mbid: String,
    limit: Option<u32>,
) -> Result<ArtistDiscography, SoneError> {
    let lim = limit.unwrap_or(50).clamp(1, 100);
    state
        .classical
        .artist_discography(&artist_mbid, lim)
        .await
}

/// Phase 6 (B6.5): kick off a canon pre-warm in the background. Async
/// no-op for the caller — returns immediately. The actual fetches run
/// in a detached task so the user doesn't wait. Safe to call multiple
/// times: pre-warm only repopulates cache misses.
#[tauri::command]
pub async fn prewarm_classical_canon(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<(), SoneError> {
    let catalog = std::sync::Arc::clone(&state.classical);
    let lim = limit.unwrap_or(30).clamp(1, 100);
    tokio::spawn(async move {
        catalog.prewarm_canon(lim).await;
    });
    Ok(())
}
