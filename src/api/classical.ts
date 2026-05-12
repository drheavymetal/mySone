import { invoke } from "@tauri-apps/api/core";

import type {
  ArtistDiscography,
  BucketWorksPage,
  ClassicalFavorite,
  ClassicalOverview,
  Composer,
  ComposerBuckets,
  ComposerSummary,
  ComposerWorksPage,
  EditorialPick,
  Era,
  ExtendedNote,
  Genre,
  LrcGuide,
  MovementContext,
  RecentClassicalSession,
  Recording,
  RecordingComparisonRow,
  RelatedComposer,
  SearchResults,
  TopClassicalComposer,
  TopClassicalWork,
  Work,
  WorkBucket,
} from "../types/classical";
import type { DiscoveryPoint, StatsWindow } from "./stats";

/**
 * Fetch a fully-fleshed Work entity by MBID. Returns title, composer,
 * description, movements, and the cascade-matched recordings list.
 *
 * Cached for 7 days at the catalog tier; SWR keeps stale data alive
 * for 30 days while a refresh happens in the background (Phase 4).
 */
export async function getClassicalWork(mbid: string): Promise<Work> {
  return invoke<Work>("get_classical_work", { mbid });
}

/**
 * Hydrate a Recording with conductor / orchestra / soloist detail.
 * Used for hover-/click-expansion within the Work page.
 */
export async function getClassicalRecording(
  mbid: string,
  workMbid: string,
): Promise<Recording> {
  return invoke<Recording>("get_classical_recording", { mbid, workMbid });
}

/** Phase 1 placeholder: fetch a Composer entity by MBID. */
export async function getClassicalComposer(mbid: string): Promise<Composer> {
  return invoke<Composer>("get_classical_composer", { mbid });
}

/**
 * Resolve the parent Work MBID for a recording. Used by the player's
 * "View work" affordance. Returns null when MB has no work-rel for
 * the recording (typical for non-classical content).
 */
export async function resolveClassicalWorkForRecording(
  recordingMbid: string,
): Promise<string | null> {
  return invoke<string | null>("resolve_classical_work_for_recording", {
    recordingMbid,
  });
}

/**
 * Snapshot the parent-Work MBID resolved for the currently playing
 * track. Returns `null` until the background MBID resolver lands.
 * The frontend polls this on a low cadence to surface the "View
 * work" button without busy-waiting.
 */
export async function getCurrentClassicalWorkMbid(): Promise<string | null> {
  return invoke<string | null>("get_current_classical_work_mbid");
}

// ---------------------------------------------------------------------------
// Phase 2 — browse
// ---------------------------------------------------------------------------

/**
 * Top-N classical composers from the embedded OpenOpus snapshot.
 * Synchronous on the backend (no I/O), so this round-trip is dominated
 * by IPC, ~5 ms typical. Used by the Hub landing's "Featured composers"
 * section and the BrowseComposers list.
 */
export async function listClassicalTopComposers(
  limit: number,
): Promise<ComposerSummary[]> {
  return invoke<ComposerSummary[]>("list_classical_top_composers", { limit });
}

/**
 * Composers in a given era (BrowsePeriods drill-down). The era literal
 * must be one of the values in `BROWSEABLE_ERAS`.
 */
export async function listClassicalComposersByEra(
  era: Era,
): Promise<ComposerSummary[]> {
  return invoke<ComposerSummary[]>("list_classical_composers_by_era", { era });
}

/**
 * Works for a composer with optional Genre filter, paginated.
 *
 * Phase 7 (D-029) — `offset` is optional (defaults to 0). The first
 * page (offset=0) drives the initial render; subsequent pages
 * (offset=100, 200, ...) power the "Load more" affordance for
 * composers with > 100 works. Cached for 7 days per
 * `(mbid, genre, offset)` triple.
 */
export async function listClassicalWorksByComposer(
  composerMbid: string,
  genre?: Genre,
  offset?: number,
): Promise<ComposerWorksPage> {
  return invoke<ComposerWorksPage>("list_classical_works_by_composer", {
    composerMbid,
    genre: genre ?? null,
    offset: offset ?? 0,
  });
}

/**
 * Phase 9 (B9.3 / D-040) — bucketed view of a composer's catalogue.
 * Returns one `BucketSummary` per non-empty bucket, sorted in canonical
 * presentation order. Each bucket carries up to 12 works in `topWorks`
 * and (when the bucket exceeds 12) a `subBuckets` palette the UI uses
 * as filter chips above the grid.
 *
 * Cold cache: multi-page MB browse, ~11s for Bach (11 pages × 1.05s).
 * Cached for 7d at `composer_buckets:v1:{mbid}`.
 */
export async function listClassicalComposerBuckets(
  composerMbid: string,
): Promise<ComposerBuckets> {
  return invoke<ComposerBuckets>("list_classical_composer_buckets", {
    composerMbid,
  });
}

/**
 * Phase 9 (B9.4) — drill-down into a single bucket of a composer.
 * Operates on the cached `bucket-full:v1:{mbid}:{bucket}` payload —
 * navigation between sub-buckets, sort modes, and pages does NOT
 * re-fetch MB once the bucket is warm.
 *
 * `subBucket` accepts the labels emitted by the backend's
 * `compute_sub_buckets` (e.g. "Piano", "Quartets", "Études"). When
 * `null`/undefined the entire bucket is returned.
 *
 * `sort` accepts "Catalog" (default) | "Date" | "Alphabetical".
 */
export async function listClassicalWorksInBucket(
  composerMbid: string,
  bucket: WorkBucket,
  options?: {
    subBucket?: string | null;
    sort?: "Catalog" | "Date" | "Alphabetical" | null;
    offset?: number;
    limit?: number;
  },
): Promise<BucketWorksPage> {
  return invoke<BucketWorksPage>("list_classical_works_in_bucket", {
    composerMbid,
    bucket,
    subBucket: options?.subBucket ?? null,
    sort: options?.sort ?? null,
    offset: options?.offset ?? 0,
    limit: options?.limit ?? 50,
  });
}

/**
 * Phase 9 (B9.6 / D-044) — fetch the extended editorial note for a
 * work. Locale resolution is server-side: missing locales fall back
 * to the entry's default language. Returns `null` when the work is
 * outside the v2 snapshot — the frontend then renders the Phase 5
 * `editorNote` and the Wikipedia summary instead.
 */
export async function getClassicalExtendedNote(
  workMbid: string,
  locale?: string,
): Promise<ExtendedNote | null> {
  return invoke<ExtendedNote | null>("get_classical_extended_note", {
    workMbid,
    locale: locale ?? null,
  });
}

/**
 * Phase 7 (F7.3) — total composers indexed in the extended snapshot.
 * Surfaced by the Hub home footer chip.
 */
export async function getClassicalExtendedTotal(): Promise<number> {
  return invoke<number>("get_classical_extended_total");
}

/**
 * Phase 7 (D-030) — re-check Tidal availability for a Work that was
 * previously marked `tidalUnavailable=true`. Drops the cache and
 * re-runs the cascade. Returns the rebuilt `Work`; the UI inspects
 * `tidalUnavailable` again to decide whether the banner stays.
 */
export async function recheckClassicalWorkTidal(workMbid: string): Promise<Work> {
  return invoke<Work>("recheck_classical_work_tidal", { workMbid });
}

// ---------------------------------------------------------------------------
// Phase 3 — movement context
// ---------------------------------------------------------------------------

/**
 * Resolve which movement of a Work the current track represents. The
 * backend handles roman-numeral parsing, attacca detection, and a
 * position-based fallback when title heuristics fail.
 *
 * Latency: warm cache hits the in-process cached `Work` (~ms), so this
 * is safe to call from the player on every track change. Cold case (no
 * cached Work yet) takes the same path as `getClassicalWork` — typically
 * already warm by the time the player asks.
 */
export async function resolveClassicalMovement(
  workMbid: string,
  trackTitle: string,
  albumPosition?: number,
): Promise<MovementContext | null> {
  return invoke<MovementContext | null>("resolve_classical_movement", {
    workMbid,
    trackTitle,
    albumPosition: albumPosition ?? null,
  });
}

// ---------------------------------------------------------------------------
// Phase 4 — quality refinement
// ---------------------------------------------------------------------------

/**
 * Phase 4 (B4.3) — force the catalog to drop its cached `Work` and the
 * per-track quality probes for this work, then rebuild. Returns the
 * freshly-rebuilt Work with refined sample-rate / bit-depth on the
 * top-N recordings.
 *
 * Use sparingly: it triggers a full MB + Tidal cascade and up to N
 * playbackinfo probes. Only the WorkPage "Refresh quality" button
 * should call this.
 */
export async function refreshClassicalWorkQualities(
  workMbid: string,
): Promise<Work> {
  return invoke<Work>("refresh_classical_work_qualities", { workMbid });
}

// ---------------------------------------------------------------------------
// Phase 5 — search + editorial + listening guides
// ---------------------------------------------------------------------------

/**
 * Phase 5 (D-019) — tokenized + planned classical search. Returns both
 * the resolved `SearchPlan` (so the UI can echo "Detected: ..." chips)
 * and the ranked hit list.
 */
export async function searchClassical(
  query: string,
  limit?: number,
): Promise<SearchResults> {
  return invoke<SearchResults>("search_classical", {
    query,
    limit: limit ?? null,
  });
}

/**
 * Phase 5 (D-020) — curated Editor's Choice picks for the Hub home grid.
 * The backend reads from the embedded `editorial.json` snapshot;
 * synchronous on the Rust side, IPC-bound on the wire.
 */
export async function listClassicalEditorialPicks(
  limit?: number,
): Promise<EditorialPick[]> {
  return invoke<EditorialPick[]>("list_classical_editorial_picks", {
    limit: limit ?? null,
  });
}

/**
 * Phase 5 (D-021) — persist a user-set Editor's Choice for a work. The
 * cached Work entry is invalidated so the next read shows the new pick.
 */
export async function setClassicalEditorsChoice(
  workMbid: string,
  recordingMbid: string,
  note?: string,
): Promise<void> {
  return invoke<void>("set_classical_editors_choice", {
    workMbid,
    recordingMbid,
    note: note ?? null,
  });
}

/** Phase 5 (D-021) — clear a user override; falls back to the snapshot pick. */
export async function clearClassicalEditorsChoice(
  workMbid: string,
): Promise<void> {
  return invoke<void>("clear_classical_editors_choice", { workMbid });
}

/**
 * Phase 5 (B5.5) — read a community-authored listening guide for a
 * work. Returns `null` when no `~/.config/sone/listening-guides/{mbid}.lrc`
 * file exists (the typical case — guides are opt-in).
 */
export async function readClassicalListeningGuide(
  workMbid: string,
): Promise<LrcGuide | null> {
  return invoke<LrcGuide | null>("read_classical_listening_guide", {
    workMbid,
  });
}

// ---------------------------------------------------------------------------
// Phase 6 — personalisation + Wikidata + browse-by-conductor
// ---------------------------------------------------------------------------

/** Phase 6 (B6.1) — your top classical works in the given window. */
export async function listTopClassicalWorks(
  window: StatsWindow,
  limit?: number,
): Promise<TopClassicalWork[]> {
  return invoke<TopClassicalWork[]>("list_top_classical_works", {
    window,
    limit: limit ?? null,
  });
}

/** Phase 6 (B6.1) — your top classical composers in the given window. */
export async function listTopClassicalComposers(
  window: StatsWindow,
  limit?: number,
): Promise<TopClassicalComposer[]> {
  return invoke<TopClassicalComposer[]>("list_top_classical_composers", {
    window,
    limit: limit ?? null,
  });
}

/** Phase 6 (B6.1) — aggregate counters for the Hub library hero. */
export async function getClassicalOverview(
  window: StatsWindow,
): Promise<ClassicalOverview> {
  return invoke<ClassicalOverview>("get_classical_overview", { window });
}

/** Phase 6 (B6.2) — discovery curve filtered to classical plays only. */
export async function getClassicalDiscoveryCurve(
  window: StatsWindow,
): Promise<DiscoveryPoint[]> {
  return invoke<DiscoveryPoint[]>("get_classical_discovery_curve", { window });
}

/** Phase 6 (B6.1) — recently-played classical sessions, grouped by work. */
export async function listRecentClassicalSessions(
  windowSecs?: number,
  limit?: number,
): Promise<RecentClassicalSession[]> {
  return invoke<RecentClassicalSession[]>("list_recent_classical_sessions", {
    windowSecs: windowSecs ?? null,
    limit: limit ?? null,
  });
}

/** Phase 6 (B6.3) — recording comparison rows for a single work. */
export async function listClassicalRecordingComparison(
  workMbid: string,
): Promise<RecordingComparisonRow[]> {
  return invoke<RecordingComparisonRow[]>(
    "list_classical_recording_comparison",
    { workMbid },
  );
}

/** Phase 6 (B6.4) — save a classical entity. Idempotent. */
export async function addClassicalFavorite(
  kind: ClassicalFavorite["kind"],
  mbid: string,
  displayName: string,
): Promise<void> {
  return invoke<void>("add_classical_favorite", { kind, mbid, displayName });
}

export async function removeClassicalFavorite(
  kind: ClassicalFavorite["kind"],
  mbid: string,
): Promise<void> {
  return invoke<void>("remove_classical_favorite", { kind, mbid });
}

export async function isClassicalFavorite(
  kind: ClassicalFavorite["kind"],
  mbid: string,
): Promise<boolean> {
  return invoke<boolean>("is_classical_favorite", { kind, mbid });
}

export async function listClassicalFavorites(
  kind: ClassicalFavorite["kind"],
  limit?: number,
): Promise<ClassicalFavorite[]> {
  return invoke<ClassicalFavorite[]>("list_classical_favorites", {
    kind,
    limit: limit ?? null,
  });
}

/** Phase 6 (D-022) — list related composers for a Composer MBID. */
export async function listClassicalRelatedComposers(
  composerMbid: string,
): Promise<RelatedComposer[]> {
  return invoke<RelatedComposer[]>("list_classical_related_composers", {
    composerMbid,
  });
}

/** Phase 6 (D-022) — discography landing for a conductor / orchestra. */
export async function getClassicalArtistDiscography(
  artistMbid: string,
  limit?: number,
): Promise<ArtistDiscography> {
  return invoke<ArtistDiscography>("get_classical_artist_discography", {
    artistMbid,
    limit: limit ?? null,
  });
}

/** Phase 6 (B6.5) — kick off a canon pre-warm in the background. */
export async function prewarmClassicalCanon(limit?: number): Promise<void> {
  return invoke<void>("prewarm_classical_canon", {
    limit: limit ?? null,
  });
}
