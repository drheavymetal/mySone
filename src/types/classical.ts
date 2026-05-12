// Domain types for the Classical Hub. Mirrors `src-tauri/src/classical/types.rs`.
//
// These shapes are produced by the Rust backend (serde camelCase) and
// consumed by React components verbatim. Do not add UI-only fields here
// — keep the file aligned with the Rust source so divergence is loud.

export type Era =
  | "Medieval"
  | "Renaissance"
  | "Baroque"
  | "Classical"
  | "EarlyRomantic"
  | "Romantic"
  | "LateRomantic"
  | "TwentiethCentury"
  | "PostWar"
  | "Contemporary"
  | "Unknown";

export type WorkType =
  | "Symphony"
  | "Concerto"
  | "Sonata"
  | "StringQuartet"
  | "Opera"
  | "Cantata"
  | "Mass"
  | "Lieder"
  | "Suite"
  | "Etude"
  | "Other";

export type Genre =
  | "Orchestral"
  | "Concerto"
  | "Chamber"
  | "SoloInstrumental"
  | "Vocal"
  | "Choral"
  | "Opera"
  | "Sacred"
  | "Stage"
  | "Film"
  | "Other";

/**
 * Phase 9 (D-039 + D-040) — taxonomy used by ComposerPage to group
 * works into 9+2 buckets. Mirrors `WorkBucket` in the Rust backend.
 * Order of the union mirrors the canonical presentation order
 * (Stage → Choral & sacred → Vocal → Symphonies → Concertos →
 * Orchestral → Chamber → Keyboard → Solo instrumental → Film & theatre
 * → Other).
 */
export type WorkBucket =
  | "Stage"
  | "ChoralSacred"
  | "Vocal"
  | "Symphonies"
  | "Concertos"
  | "Orchestral"
  | "Chamber"
  | "Keyboard"
  | "SoloInstrumental"
  | "FilmTheatre"
  | "Other";

/** Localised label for a `WorkBucket`. The frontend uses this for
 *  section headers and chips when the backend payload doesn't carry
 *  a pre-resolved label. */
export function workBucketLabel(bucket: WorkBucket, locale: "en" | "es" = "en"): string {
  if (locale === "es") {
    switch (bucket) {
      case "Stage":
        return "Obras escénicas";
      case "ChoralSacred":
        return "Coral y sacro";
      case "Vocal":
        return "Vocal";
      case "Symphonies":
        return "Sinfonías";
      case "Concertos":
        return "Conciertos";
      case "Orchestral":
        return "Orquestal";
      case "Chamber":
        return "Cámara";
      case "Keyboard":
        return "Teclado";
      case "SoloInstrumental":
        return "Instrumento solo";
      case "FilmTheatre":
        return "Cine y teatro";
      case "Other":
      default:
        return "Otros";
    }
  }
  switch (bucket) {
    case "Stage":
      return "Stage works";
    case "ChoralSacred":
      return "Choral & sacred";
    case "Vocal":
      return "Vocal";
    case "Symphonies":
      return "Symphonies";
    case "Concertos":
      return "Concertos";
    case "Orchestral":
      return "Orchestral";
    case "Chamber":
      return "Chamber";
    case "Keyboard":
      return "Keyboard";
    case "SoloInstrumental":
      return "Solo instrumental";
    case "FilmTheatre":
      return "Film & theatre";
    case "Other":
    default:
      return "Other";
  }
}

export const ALL_WORK_BUCKETS: ReadonlyArray<WorkBucket> = [
  "Stage",
  "ChoralSacred",
  "Vocal",
  "Symphonies",
  "Concertos",
  "Orchestral",
  "Chamber",
  "Keyboard",
  "SoloInstrumental",
  "FilmTheatre",
  "Other",
];

/**
 * Match confidence tiers for the cascade matching (D-010 + D-037).
 *
 * - `IsrcBound`: deterministic. MB exposed an ISRC and Tidal accepted it.
 * - `TextSearchInferred`: heuristic. MB browse returned a recording,
 *   Tidal text search resolved it, score ≥ 0.6.
 * - `TidalDirectInferred` (D-037, bug 3 fix): MB had no recordings for
 *   the work, OR none crossed the per-recording threshold. Backend ran
 *   a work-level Tidal search (composer + work title, no artist
 *   constraint) and the top result crossed 0.55. Lower confidence than
 *   `TextSearchInferred`; UI surfaces the query used at hover.
 * - `NotFound`: info-only row, no play button.
 */
export type MatchConfidence =
  | "IsrcBound"
  | "TextSearchInferred"
  | "TidalDirectInferred"
  | "NotFound";

/**
 * D-038 (bug 4 fix) — kind discriminator on `SoneError` JSON serialised by
 * the backend. The frontend can render specific copy for transient
 * failures (retry CTA visible, no permanent "service down" framing).
 */
export type SoneErrorKind =
  | "Api"
  | "Parse"
  | "Network"
  | "NetworkTransient"
  | "NotAuthenticated"
  | "NotConfigured"
  | "Io"
  | "Audio"
  | "Crypto"
  | "Scrobble";

/**
 * Best-effort detector for transient `SoneError` payloads. Tauri
 * serialises `SoneError` as `{ kind, message }` (see `error.rs`'s
 * `#[serde(tag = "kind", content = "message")]`). We don't rely on the
 * exact shape — we accept any object with a `kind` field that signals
 * transient.
 */
export function isTransientSoneError(err: unknown): boolean {
  if (err === null || typeof err !== "object") {
    return false;
  }
  const k = (err as { kind?: unknown }).kind;
  return k === "NetworkTransient";
}

export interface PerformerCredit {
  mbid?: string;
  name: string;
  /** 'person' | 'group' | 'orchestra' | 'choir' | 'ensemble' */
  kind: string;
}

export interface PerformerCreditWithRole {
  mbid?: string;
  name: string;
  kind: string;
  /** Localized role label: "violin", "soprano", "piano", ... */
  role: string;
  instrumentMbid?: string;
}

export interface CatalogueNumber {
  /** 'BWV' | 'K' | 'D' | 'RV' | 'Hob' | 'HWV' | 'Op' | 'Other' */
  system: string;
  number: string;
  /** Display string the UI renders verbatim. */
  display: string;
}

export interface LifeEvent {
  year?: number;
  date?: string;
  place?: string;
}

export interface Composer {
  mbid: string;
  qid?: string;
  openOpusId?: string;
  name: string;
  fullName?: string;
  birth?: LifeEvent;
  death?: LifeEvent;
  era: Era;
  portraitUrl?: string;
  bioShort?: string;
  bioLong?: string;
  bioSourceUrl?: string;
  /** Phase 5 (D-020) — editorial blurb from the curated snapshot. */
  editorNote?: string;
  /** Phase 6 (D-022) — Wikidata-backed list of related composers.
   *  Empty when the composer has no QID, no genre overlap, or WD
   *  failed. The frontend renders the section as "Related composers". */
  relatedComposers?: RelatedComposer[];
}

/** Phase 6 (D-022) — one entry in the "Related composers" sidebar of
 *  the ComposerPage. The MBID is best-effort (Wikidata's P434 link)
 *  — when empty the UI renders the chip as info-only (no nav). */
export interface RelatedComposer {
  qid: string;
  /** May be empty when Wikidata had no P434 (MB) link. */
  mbid?: string;
  name: string;
  /** Genre QIDs shared with the seed composer (e.g. "Q9730" for opera). */
  sharedGenres?: string[];
  birthYear?: number;
  portraitUrl?: string;
}

export interface Movement {
  mbid: string;
  index: number;
  title: string;
  durationApproxSecs?: number;
  attaccaTo?: number;
}

export interface Recording {
  mbid: string;
  workMbid: string;
  title?: string;
  conductor?: PerformerCredit;
  orchestras: PerformerCredit[];
  soloists: PerformerCreditWithRole[];
  ensemble?: PerformerCredit;
  choir?: PerformerCredit;
  recordingYear?: number;
  recordingDate?: string;
  venue?: string;
  label?: string;
  isrcs: string[];
  artistCredits: string[];
  coverUrl?: string;
  tidalTrackId?: number;
  tidalAlbumId?: number;
  audioQualityTags: string[];
  audioModes: string[];
  durationSecs?: number;
  /** Phase 4 — refined sample rate from Tidal manifest metadata. */
  sampleRateHz?: number;
  /** Phase 4 — refined bit depth (16, 24, ...) from Tidal manifest metadata. */
  bitDepth?: number;
  /** Phase 4 — pre-computed numeric score for sort/filter (D-018). 0 means no playable match. */
  qualityScore: number;
  /** Phase 5 (D-020 + D-021) — true when this row is the (snapshot or user) Editor's Choice. */
  isEditorsChoice?: boolean;
  /** Phase 5 — short rationale shown in the Editor's Choice tooltip. */
  editorNote?: string;
  matchConfidence: MatchConfidence;
  matchQuery?: string;
  matchScore?: number;
}

/**
 * Phase 4 (D-018) — work-level quality summary derived from the
 * recordings list. Mirrors `BestAvailableQuality` in the Rust backend.
 */
export interface BestAvailableQuality {
  /** "HIRES_LOSSLESS" | "LOSSLESS" | "DOLBY_ATMOS" | "MQA" | "HIGH" */
  tier: string;
  sampleRateHz?: number;
  bitDepth?: number;
  /** True when at least one recording has DOLBY_ATMOS in `audioModes`. */
  hasAtmos: boolean;
}

export interface Work {
  mbid: string;
  qid?: string;
  title: string;
  composerMbid?: string;
  composerName?: string;
  alternativeTitles: string[];
  catalogueNumber?: CatalogueNumber;
  key?: string;
  genre?: Genre;
  workType?: WorkType;
  /** Phase 9 (D-040) — presentation bucket cached on the Work. */
  bucket?: WorkBucket;
  compositionYear?: number;
  premiereYear?: number;
  durationApproxSecs?: number;
  movements: Movement[];
  description?: string;
  descriptionSourceUrl?: string;
  recordings: Recording[];
  recordingCount: number;
  /** Phase 4 — best quality across the recordings list, when known. */
  bestAvailableQuality?: BestAvailableQuality;
  /** Phase 5 (D-020) — 1-3 sentence editorial blurb from the snapshot. */
  editorNote?: string;
  /**
   * Phase 7 (D-030) — `true` when the cascade ISRC + Tidal text-search
   * produced zero playable recordings on the last fresh fetch. The
   * frontend renders a "No recordings on Tidal yet" banner with a
   * Re-check CTA. Cached for 7d; expires on its own and can be force-
   * refreshed via the `recheckClassicalWorkTidal` command.
   */
  tidalUnavailable?: boolean;
}

// ---------------------------------------------------------------------------
// Phase 2 — browse summaries
// ---------------------------------------------------------------------------

/**
 * Lightweight composer projection for grid/list views (Hub landing,
 * BrowseComposers, BrowsePeriods drill-down). Mirrors `ComposerSummary`
 * in the Rust backend.
 */
export interface ComposerSummary {
  mbid: string;
  openOpusId?: string;
  name: string;
  fullName?: string;
  birthYear?: number;
  deathYear?: number;
  era: Era;
  portraitUrl?: string;
  popular: boolean;
}

/**
 * Lightweight work projection for the Composer page sections. Mirrors
 * `WorkSummary` in the Rust backend. Click → navigate to WorkPage with
 * `mbid`; full Work entity hydrates from `getClassicalWork` there.
 */
export interface WorkSummary {
  mbid: string;
  title: string;
  composerMbid?: string;
  composerName?: string;
  catalogueNumber?: CatalogueNumber;
  key?: string;
  workType?: WorkType;
  genre?: Genre;
  /** Phase 9 (D-040) — bucket cached on every summary so list views
   *  don't recompute. May be `undefined` on legacy cached payloads. */
  bucket?: WorkBucket;
  compositionYear?: number;
  popular: boolean;
}

// ---------------------------------------------------------------------------
// Phase 9 (B9.3 / B9.4 / D-040 / D-041) — bucketed composer payloads
// ---------------------------------------------------------------------------

/** Phase 9 (B9.3) — full bucketed view of a composer's catalogue. */
export interface ComposerBuckets {
  composerMbid: string;
  buckets: BucketSummary[];
  totalWorks: number;
  mbTotal: number;
  canonicalWorksLoaded: number;
}

/** Phase 9 (B9.3) — one bucket inside a `ComposerBuckets` response. */
export interface BucketSummary {
  bucket: WorkBucket;
  labelEn: string;
  labelEs: string;
  totalCount: number;
  topWorks: WorkSummary[];
  subBuckets?: SubBucketSummary[];
}

/** Phase 9 (B9.3) — sub-bucket inside a parent bucket. */
export interface SubBucketSummary {
  /** "Piano" | "Violin" | "Cello" | "Quartets" | … */
  label: string;
  count: number;
}

/** Phase 9 (B9.4) — drill-down response for a single bucket. */
export interface BucketWorksPage {
  works: WorkSummary[];
  total: number;
  offset: number;
  hasMore: boolean;
}

// ---------------------------------------------------------------------------
// Phase 9 (B9.6 / D-044) — extended editorial notes ("About this work")
// ---------------------------------------------------------------------------

/** Phase 9 (D-044) — single source attribution. */
export interface ExtendedSource {
  /** "wikipedia" | "wikidata" | "editor" | (other) */
  kind: string;
  url?: string;
  qid?: string;
  name?: string;
  license?: string;
}

/** Phase 9 (D-044) — five sub-sections rendered by `AboutThisWork`.
 *  Each field is markdown-light: `_italic_`, `**bold**`, `[label](url)`. */
export interface ExtendedNoteBody {
  origin?: string;
  premiere?: string;
  highlights?: string;
  context?: string;
  notableRecordingsEssay?: string;
}

/** Phase 9 (D-044) — full extended note for a work, locale-resolved. */
export interface ExtendedNote {
  /** Resolved locale ("en" or the requested locale when a translation existed). */
  language: string;
  body: ExtendedNoteBody;
  sources: ExtendedSource[];
}

/**
 * Phase 7 (D-029) — paginated response for the "All works of a composer"
 * list. Mirrors `ComposerWorksPage` in the Rust backend. Used by the
 * ComposerPage's expandable "All works" section, which loads pages of
 * 100 on demand.
 *
 * Phase 8.9 (D-047 / A4) adds `nextOffset`. The frontend used to pass
 * `works.length` (post movement-filter) which silently overlapped with
 * the previous MB browse window. `nextOffset` is the MB-pre-filter
 * cursor — frontend forwards it verbatim on the next request so paging
 * advances strictly through MB's catalogue.
 */
export interface ComposerWorksPage {
  works: WorkSummary[];
  /** Total works available for this composer (MB report, pre-filter). */
  total: number;
  offset: number;
  hasMore: boolean;
  /** D-047 — MB-pre-filter cursor for the next page. */
  nextOffset: number;
}

/**
 * The 9 era buckets the Hub uses for `BrowsePeriods`. `Unknown` is
 * intentionally NOT in this list — it's an empty state, not a browse axis.
 */
export const BROWSEABLE_ERAS: ReadonlyArray<Era> = [
  "Medieval",
  "Renaissance",
  "Baroque",
  "Classical",
  "EarlyRomantic",
  "Romantic",
  "LateRomantic",
  "TwentiethCentury",
  "PostWar",
  "Contemporary",
];

/** Human-readable label for an Era. Used by chips/badges. */
export function eraLabel(era: Era): string {
  switch (era) {
    case "Medieval":
      return "Medieval";
    case "Renaissance":
      return "Renaissance";
    case "Baroque":
      return "Baroque";
    case "Classical":
      return "Classical";
    case "EarlyRomantic":
      return "Early Romantic";
    case "Romantic":
      return "Romantic";
    case "LateRomantic":
      return "Late Romantic";
    case "TwentiethCentury":
      return "20th Century";
    case "PostWar":
      return "Post-War";
    case "Contemporary":
      return "Contemporary";
    case "Unknown":
    default:
      return "Unknown era";
  }
}

/** Approximate year span for an era. Used by BrowsePeriods cards. */
export function eraYearSpan(era: Era): string {
  switch (era) {
    case "Medieval":
      return "—1399";
    case "Renaissance":
      return "1400–1599";
    case "Baroque":
      return "1600–1749";
    case "Classical":
      return "1750–1799";
    case "EarlyRomantic":
      return "1800–1849";
    case "Romantic":
      return "1850–1899";
    case "LateRomantic":
      return "1850–1910";
    case "TwentiethCentury":
      return "1900–1929";
    case "PostWar":
      return "1930–1959";
    case "Contemporary":
      return "1960–today";
    case "Unknown":
    default:
      return "";
  }
}

/** Human-readable label for a Genre. */
export function genreLabel(genre: Genre): string {
  switch (genre) {
    case "Orchestral":
      return "Orchestral";
    case "Concerto":
      return "Concerto";
    case "Chamber":
      return "Chamber";
    case "SoloInstrumental":
      return "Solo Instrumental";
    case "Vocal":
      return "Vocal";
    case "Choral":
      return "Choral";
    case "Opera":
      return "Opera";
    case "Sacred":
      return "Sacred";
    case "Stage":
      return "Stage";
    case "Film":
      return "Film";
    case "Other":
    default:
      return "Other";
  }
}

// ---------------------------------------------------------------------------
// Phase 3 — movement context
// ---------------------------------------------------------------------------

/**
 * How the backend resolved the current movement match. Surfaced to the
 * UI as a tooltip / debug attribute. Useful when QA-ing why the player
 * shows "II / IV" for a particular track.
 */
export type ResolutionMethod = "romanPrefix" | "titleSubstring" | "albumPosition";

/**
 * Resolved movement of a Work that the current track belongs to. Backed
 * by `src-tauri/src/classical/movement.rs::MovementContext`. The player
 * uses this to render `II / IV` + the optional `Attacca →` chip.
 */
export interface MovementContext {
  /** 1-based, always within `1..=total`. */
  index: number;
  total: number;
  title: string;
  /** Present when this movement attaccas into another. */
  attaccaTo?: number;
  method: ResolutionMethod;
}

/**
 * Payload of the `classical:work-resolved` Tauri event emitted by the
 * scrobble manager once it resolves the parent Work MBID for the
 * currently playing track. Frontend listens via `listen("classical:work-resolved", ...)`.
 */
export interface ClassicalWorkResolvedPayload {
  /** Tidal track id, when the player knew it at lookup time. */
  trackId?: number;
  /** Recording MBID resolved by MusicBrainz (always set when emitted). */
  recordingMbid: string;
  /** Parent Work MBID resolved via `WorkMbidResolver`. */
  workMbid: string;
}

// ---------------------------------------------------------------------------
// Phase 5 — search + editorial + listening guides
// ---------------------------------------------------------------------------

/** Token recognised by the classical search tokenizer (D-019). Mirrors
 *  `classical::search::Token` in the Rust backend. */
export type SearchToken =
  | { kind: "Catalogue"; value: { system: string; number: string; display: string } }
  | { kind: "Year"; value: number }
  | { kind: "Key"; value: string }
  | { kind: "Composer"; value: { surname: string; mbid: string } }
  | { kind: "Keyword"; value: string };

/** Resolved search plan returned alongside the hits so the UI can render
 *  "Detected: composer:Beethoven · year:1962" chips. */
export interface SearchPlan {
  composerMbid?: string;
  composerName?: string;
  catalogue?: { system: string; number: string; display: string };
  year?: number;
  key?: string;
  keywords: string;
  tokens: SearchToken[];
}

/** A single search result row. Click → navigate to WorkPage. */
export interface SearchHit {
  workMbid: string;
  title: string;
  composerName?: string;
  composerMbid?: string;
  catalogueDisplay?: string;
  /** [0, 1], higher is better. UI sorts by this. */
  score: number;
  /** "snapshot" | "mb-lucene" | "composer-list" — surfaced for debug/UX. */
  source: string;
}

export interface SearchResults {
  plan: SearchPlan;
  hits: SearchHit[];
}

/** Editorial pick rendered on the Hub home grid (D-020). */
export interface EditorsChoice {
  conductor?: string;
  performer: string;
  year?: number;
  label?: string;
  note?: string;
}

export interface EditorialPick {
  composerMbid: string;
  composerName: string;
  titleCanonical: string;
  catalogue?: string;
  editorsChoice: EditorsChoice;
}

/** A single line of a community listening-guide LRC file. */
export interface LrcLine {
  /** Absolute position in milliseconds. `undefined` for untimed headers. */
  tsMs?: number;
  text: string;
}

export interface LrcGuide {
  workMbid: string;
  lines: LrcLine[];
}

// ---------------------------------------------------------------------------
// Phase 6 — personalisation + Wikidata + browse-by-conductor
// ---------------------------------------------------------------------------

/** One row of the "Top classical works" leaderboard. */
export interface TopClassicalWork {
  workMbid: string;
  plays: number;
  listenedSecs: number;
  sampleTitle?: string;
  sampleArtist?: string;
  sampleAlbum?: string;
  distinctRecordings: number;
}

/** One row of the "Top classical composers" leaderboard. */
export interface TopClassicalComposer {
  composerMbid: string;
  plays: number;
  listenedSecs: number;
  distinctWorks: number;
  sampleName?: string;
}

/** Aggregate footprint of classical listening for the given window. */
export interface ClassicalOverview {
  totalPlays: number;
  totalListenedSecs: number;
  distinctWorks: number;
  distinctComposers: number;
  distinctRecordings: number;
  sinceUnix: number;
}

/** A recently-played classical session, grouped by `workMbid`. */
export interface RecentClassicalSession {
  workMbid: string;
  lastStartedAt: number;
  firstStartedAt: number;
  plays: number;
  listenedSecs: number;
  sampleTitle?: string;
  sampleArtist?: string;
  sampleAlbum?: string;
  distinctRecordings: number;
}

/** A row of the recording-comparison view: same work, different versions. */
export interface RecordingComparisonRow {
  recordingMbid: string;
  plays: number;
  listenedSecs: number;
  completedCount: number;
  sampleArtist?: string;
  sampleAlbum?: string;
  lastStartedAt: number;
}

/** Saved-favorite row in the Hub Library tab. */
export interface ClassicalFavorite {
  id: number;
  /** "work" | "recording" | "composer" | "performer" */
  kind: string;
  mbid: string;
  displayName: string;
  addedAt: number;
}

/** Browse-by-conductor result — flat entries plus a grouped view. */
export interface ArtistDiscography {
  artistMbid: string;
  total: number;
  entries: DiscographyEntry[];
  groups: DiscographyGroup[];
}

export interface DiscographyEntry {
  recordingMbid: string;
  title: string;
  artistCredit: string;
  workMbid?: string;
  releaseYear?: number;
  lengthSecs?: number;
  isrcs?: string[];
}

export interface DiscographyGroup {
  /** `undefined` → the synthetic "no parent work" bucket. */
  workMbid?: string;
  count: number;
  /** Indices into `ArtistDiscography.entries`. */
  indices: number[];
}

/** Human-readable label for a WorkType. */
export function workTypeLabel(t: WorkType): string {
  switch (t) {
    case "Symphony":
      return "Symphony";
    case "Concerto":
      return "Concerto";
    case "Sonata":
      return "Sonata";
    case "StringQuartet":
      return "String Quartet";
    case "Opera":
      return "Opera";
    case "Cantata":
      return "Cantata";
    case "Mass":
      return "Mass";
    case "Lieder":
      return "Lieder";
    case "Suite":
      return "Suite";
    case "Etude":
      return "Etude";
    case "Other":
    default:
      return "Other";
  }
}
