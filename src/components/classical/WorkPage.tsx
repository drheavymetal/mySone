import { useEffect, useMemo, useState } from "react";

import {
  getClassicalWork,
  listClassicalRecordingComparison,
  recheckClassicalWorkTidal,
  refreshClassicalWorkQualities,
} from "../../api/classical";
import {
  isTransientSoneError,
  type Recording,
  type RecordingComparisonRow,
  type Work,
} from "../../types/classical";
import { useNavigation } from "../../hooks/useNavigation";
import PageContainer from "../PageContainer";

import AboutThisWork from "./AboutThisWork";
import FavoriteToggle from "./FavoriteToggle";
import MovementList from "./MovementList";
import RecordingRow from "./RecordingRow";
import RecordingFilters, {
  DEFAULT_FILTERS,
  type RecordingFilterState,
  applyRecordingFilters,
} from "./RecordingFilters";
import RecordingSort, {
  type RecordingSortKey,
  applyRecordingSort,
} from "./RecordingSort";
import { BestAvailableChip } from "./QualityChip";
import WorkSidebar from "./WorkSidebar";

interface WorkPageProps {
  mbid: string;
  onBack: () => void;
}

function formatDuration(secs?: number): string {
  if (!secs || secs <= 0) {
    return "";
  }
  const m = Math.round(secs / 60);
  return `${m} min`;
}

function headerSubtitle(work: Work): string {
  const parts: string[] = [];
  if (work.catalogueNumber) {
    parts.push(work.catalogueNumber.display);
  }
  if (work.compositionYear) {
    parts.push(String(work.compositionYear));
  } else if (work.premiereYear) {
    parts.push(`prem. ${work.premiereYear}`);
  }
  if (work.key) {
    parts.push(work.key);
  }
  if (work.movements.length > 0) {
    parts.push(
      `${work.movements.length} ${work.movements.length === 1 ? "movement" : "movements"}`,
    );
  }
  const dur = formatDuration(work.durationApproxSecs);
  if (dur) {
    parts.push(`~${dur}`);
  }
  return parts.join(" · ");
}

function Skeleton() {
  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <div className="h-8 w-3/4 animate-pulse rounded bg-th-surface" />
        <div className="h-4 w-1/2 animate-pulse rounded bg-th-surface/60" />
        <div className="h-4 w-2/3 animate-pulse rounded bg-th-surface/40" />
      </div>
      <div className="h-32 animate-pulse rounded-2xl bg-th-surface/40" />
      <div className="space-y-2">
        {Array.from({ length: 5 }).map((_, idx) => (
          <div
            key={idx}
            className="h-16 animate-pulse rounded-xl bg-th-surface/40"
          />
        ))}
      </div>
    </div>
  );
}

/**
 * Work page — central screen of the Hub.
 *
 * Phase 9 (D-042) — re-organised into 8 canonical sections:
 *   1. Header (title + composer + catalogue + key + year + duration +
 *      movements count + recording count + best-quality badge).
 *   2. Editor's Choice banner (separate from the recordings list — D-042).
 *   3. About this work (the USP — `AboutThisWork`, lazy-loads from the
 *      v2 editorial snapshot, falls back to Phase 5 `editor_note` +
 *      Wikipedia summary).
 *   4. Listening guide (LRC reader, when available — Phase 5).
 *   5. Movements list (when `movements.length > 0`).
 *   6. Popular Recordings (top 8 by quality + popularity + EC).
 *   7. All Recordings (filters + sort + pagination, Phase 4 layout).
 *   8. Right sidebar on desktop ≥ 1280px (related, cross-version,
 *      performers — `WorkSidebar`).
 *
 * Data flow: a single Tauri call (`get_classical_work`) returns the
 * fully-resolved Work — including the cascade-matched recordings —
 * because the catalog runs the providers + matcher backend-side. The
 * frontend doesn't orchestrate provider chains. The extended note
 * loads independently inside `AboutThisWork`.
 */
export default function WorkPage({ mbid, onBack }: WorkPageProps) {
  const { navigateToClassicalCompare } = useNavigation();
  const [work, setWork] = useState<Work | null>(null);
  const [error, setError] = useState<string | null>(null);
  // D-038 (bug 4) — distinguish transient backend failures from
  // permanent ones so the UI can show a "Retry" CTA + softer copy
  // ("intermittent connectivity") instead of framing it as a hard
  // outage. The backend never caches a transient result, so a retry
  // is genuinely cheap.
  const [errorIsTransient, setErrorIsTransient] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [filters, setFilters] = useState<RecordingFilterState>(DEFAULT_FILTERS);
  const [sortKey, setSortKey] = useState<RecordingSortKey>("popularity");
  const [comparison, setComparison] = useState<RecordingComparisonRow[]>([]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setErrorIsTransient(false);
    setWork(null);
    setComparison([]);
    setFilters(DEFAULT_FILTERS);
    setSortKey("popularity");

    Promise.all([
      getClassicalWork(mbid),
      listClassicalRecordingComparison(mbid).catch((err: unknown) => {
        // Stats query failures are non-fatal; the page renders without
        // the "X versions you've played" link.
        console.warn("[classical] recording comparison failed:", err);
        return [] as RecordingComparisonRow[];
      }),
    ])
      .then(([data, cmp]) => {
        if (cancelled) {
          return;
        }
        setWork(data);
        setComparison(cmp);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) {
          return;
        }
        console.error("[classical] failed to load work:", err);
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        setErrorIsTransient(isTransientSoneError(err));
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [mbid, reloadKey]);

  // Apply filters + sort. Memoized so recording rows don't re-render on
  // every parent re-render — a Beethoven 9 page can have 60 rows.
  const visibleRecordings = useMemo(() => {
    if (!work) {
      return [];
    }
    const filtered = applyRecordingFilters(work.recordings, filters);
    return applyRecordingSort(filtered, sortKey);
  }, [work, filters, sortKey]);

  const earliestYear = useMemo(() => {
    if (!work) {
      return 0;
    }
    let earliest = Infinity;
    for (const rec of work.recordings) {
      if (rec.recordingYear && rec.recordingYear < earliest) {
        earliest = rec.recordingYear;
      }
    }
    return earliest === Infinity ? 0 : earliest;
  }, [work]);

  const handleRefreshQuality = async () => {
    if (!work || refreshing) {
      return;
    }
    setRefreshing(true);
    try {
      const fresh = await refreshClassicalWorkQualities(work.mbid);
      setWork(fresh);
    } catch (err: unknown) {
      console.error("[classical] refresh quality failed:", err);
    } finally {
      setRefreshing(false);
    }
  };

  const handleHiResShortcut = () => {
    setFilters((prev) => ({ ...prev, hiResOnly: true }));
  };

  // Phase 7 (D-030) — re-check Tidal availability for a work that came
  // back with `tidal_unavailable=true`. Drops the cache and re-runs the
  // cascade. UI shows a loading state via `refreshing`.
  const handleRecheckTidal = async () => {
    if (!work || refreshing) {
      return;
    }
    setRefreshing(true);
    try {
      const fresh = await recheckClassicalWorkTidal(work.mbid);
      setWork(fresh);
    } catch (err: unknown) {
      console.error("[classical] re-check tidal failed:", err);
    } finally {
      setRefreshing(false);
    }
  };

  // Phase 5 — when the user toggles Editor's Choice on a row, the
  // backend has invalidated the cached Work; re-fetch so the new state
  // surfaces in the rest of the list.
  const handleEditorialChange = async () => {
    if (!work) {
      return;
    }
    try {
      const fresh = await getClassicalWork(work.mbid);
      setWork(fresh);
    } catch (err: unknown) {
      console.error("[classical] reload after editorial change failed:", err);
    }
  };

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full">
      <PageContainer className="px-6 py-8">
        {/* Back affordance */}
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
          aria-label="Back"
        >
          <svg
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            className="h-4 w-4"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M15 19l-7-7 7-7"
            />
          </svg>
          Back
        </button>

        {loading && <Skeleton />}

        {error && !loading && (
          // D-038 (bug 4) — transient errors get amber styling + Retry
          // CTA. Permanent errors keep the red framing.
          <div
            className={
              errorIsTransient
                ? "rounded-2xl border border-amber-400/40 bg-amber-400/10 p-6 text-amber-100"
                : "rounded-2xl border border-red-500/40 bg-red-500/10 p-6 text-red-200"
            }
          >
            <h2 className="text-[15px] font-semibold">
              {errorIsTransient
                ? "Connection blip — couldn't reach MusicBrainz"
                : "Could not load this work"}
            </h2>
            <p
              className={
                errorIsTransient
                  ? "mt-2 text-[13px] text-amber-100/80"
                  : "mt-2 text-[13px] text-red-200/80"
              }
            >
              {errorIsTransient
                ? "This is usually a transient network issue (DNS, TLS handshake, or upstream throttling). Nothing has been cached so a retry is cheap."
                : error}
            </p>
            {errorIsTransient && (
              <p className="mt-1 text-[12px] text-amber-100/60 font-mono">
                {error}
              </p>
            )}
            {errorIsTransient && (
              <button
                type="button"
                onClick={() => setReloadKey((k) => k + 1)}
                className="mt-4 inline-flex items-center gap-2 rounded border border-amber-300/40 bg-amber-300/10 px-3 py-1.5 text-[12px] font-semibold text-amber-100 hover:bg-amber-300/20 transition-colors"
              >
                Retry
              </button>
            )}
          </div>
        )}

        {!loading && !error && work && (
          <div className="grid grid-cols-1 gap-8 xl:grid-cols-[minmax(0,1fr)_320px]">
            <div className="min-w-0 space-y-10">
              {/* (1) Header */}
              <header>
                <h1 className="text-[34px] font-extrabold tracking-tight text-th-text-primary">
                  {work.title}
                </h1>
                {work.composerName && (
                  <p className="mt-1 text-[16px] font-semibold text-th-text-secondary">
                    {work.composerName}
                  </p>
                )}
                <p className="mt-2 text-[13px] text-th-text-secondary/80">
                  {headerSubtitle(work)}
                </p>
                <div className="mt-3 flex flex-wrap items-center gap-2">
                  {work.bestAvailableQuality && (
                    <button
                      type="button"
                      onClick={handleHiResShortcut}
                      className="inline-flex items-center gap-2 rounded-full border border-th-border-subtle/60 bg-th-surface/40 px-3 py-1 text-[12px] text-th-text-secondary hover:border-th-accent/40 hover:text-th-text-primary transition-colors"
                      title="Filter to Hi-Res only"
                    >
                      <span className="font-bold uppercase tracking-wider text-[10px] text-th-text-secondary">
                        Best available
                      </span>
                      <BestAvailableChip best={work.bestAvailableQuality} />
                    </button>
                  )}
                  <FavoriteToggle
                    kind="work"
                    mbid={work.mbid}
                    displayName={work.title}
                  />
                  {comparison.length > 1 && (
                    <button
                      type="button"
                      onClick={() =>
                        navigateToClassicalCompare(work.mbid, work.title)
                      }
                      className="inline-flex items-center gap-1 rounded-full border border-th-border-subtle/60 bg-th-surface/40 px-3 py-1 text-[12px] text-th-text-secondary hover:border-th-accent/40 hover:text-th-text-primary transition-colors"
                      title="Compare your plays across recordings"
                    >
                      <span className="font-mono text-[11px] tabular-nums text-th-accent">
                        {comparison.length}
                      </span>
                      versions you've played
                    </button>
                  )}
                </div>
              </header>

              {/* (2) Editor's Choice banner — separate from the list (D-042). */}
              <EditorsChoiceBanner work={work} />

              {/* (3) About this work — USP. */}
              <AboutThisWork
                workMbid={work.mbid}
                fallbackEditorNote={work.editorNote}
                fallbackWikipediaSummary={work.description}
                fallbackWikipediaUrl={work.descriptionSourceUrl}
              />

              {/* (5) Movements — only when present. */}
              {work.movements.length > 0 && (
                <section aria-label="Movements">
                  <h2 className="mb-3 text-[15px] font-bold uppercase tracking-[0.18em] text-th-text-secondary">
                    Movements
                  </h2>
                  <MovementList movements={work.movements} />
                </section>
              )}

              {/* (6) Popular recordings — top 8 by quality+popularity. */}
              {work.recordings.length > 0 && (
                <PopularRecordingsSection
                  work={work}
                  onEditorialChange={handleEditorialChange}
                />
              )}

              {/* (7) All recordings — filters / sort / pagination. */}
              <section aria-label="All recordings">
                <div className="mb-3 flex flex-wrap items-baseline justify-between gap-3">
                  <div className="flex items-baseline gap-3">
                    <h2 className="text-[15px] font-bold uppercase tracking-[0.18em] text-th-text-secondary">
                      All recordings
                    </h2>
                    <span className="text-[12px] text-th-text-secondary/70">
                      {visibleRecordings.length} of {work.recordings.length}
                      {work.recordingCount > work.recordings.length
                        ? ` (${work.recordingCount} known)`
                        : ""}
                    </span>
                  </div>
                  <div className="flex items-center gap-3">
                    <RecordingSort value={sortKey} onChange={setSortKey} />
                    <button
                      type="button"
                      onClick={handleRefreshQuality}
                      disabled={refreshing}
                      className="rounded-full border border-th-border-subtle/60 px-3 py-1 text-[11px] font-medium text-th-text-secondary hover:border-th-accent/40 hover:text-th-text-primary disabled:opacity-50 transition-colors"
                      title="Re-probe top recordings for refined sample-rate / bit-depth"
                    >
                      {refreshing ? "Refreshing…" : "Refresh quality"}
                    </button>
                  </div>
                </div>

                {/*
                 * Phase 7 (D-030) — Tidal availability banner. Shown when
                 * the cascade returned zero playable recordings. CTA
                 * triggers a fresh cascade attempt.
                 */}
                {work.tidalUnavailable && (
                  <div className="mb-4 rounded-xl border border-amber-500/40 bg-amber-500/10 p-4 text-[13px] text-amber-100">
                    <div className="flex items-start justify-between gap-4">
                      <div className="flex flex-col gap-1">
                        <p className="font-semibold">
                          No playable recordings on Tidal yet
                        </p>
                        <p className="text-amber-100/80">
                          We searched every cascade path (ISRC + Tidal text
                          search) and didn't find a match. The result is
                          cached for 7 days. If you think Tidal recently
                          added recordings, re-check below.
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={handleRecheckTidal}
                        disabled={refreshing}
                        className="shrink-0 rounded-full border border-amber-500/40 bg-amber-500/15 px-3 py-1 text-[12px] font-medium text-amber-100 hover:bg-amber-500/25 disabled:opacity-50 transition-colors"
                      >
                        {refreshing ? "Re-checking…" : "Re-check Tidal"}
                      </button>
                    </div>
                  </div>
                )}

                {work.recordings.length > 0 && (
                  <div className="mb-3">
                    <RecordingFilters
                      state={filters}
                      onChange={setFilters}
                      earliestYear={earliestYear}
                    />
                  </div>
                )}

                {work.recordings.length === 0 ? (
                  <div className="rounded-2xl border border-th-border-subtle/60 bg-th-surface/40 p-6 text-center text-[13px] text-th-text-secondary">
                    No recordings linked to this work yet.
                  </div>
                ) : visibleRecordings.length === 0 ? (
                  <div className="rounded-2xl border border-th-border-subtle/60 bg-th-surface/40 p-6 text-center text-[13px] text-th-text-secondary">
                    No recordings match the current filters.
                  </div>
                ) : (
                  <ul className="space-y-2">
                    {visibleRecordings.map((rec) => (
                      <RecordingRow
                        key={rec.mbid}
                        recording={rec}
                        workTitle={work.title}
                        onEditorialChange={handleEditorialChange}
                      />
                    ))}
                  </ul>
                )}
              </section>
            </div>

            {/* (8) Sidebar — desktop ≥ 1280px only. */}
            <div className="hidden xl:block">
              <WorkSidebar work={work} />
            </div>
          </div>
        )}
      </PageContainer>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Editor's Choice banner (D-042) — moved out of the recordings list into
// its own prominent section. Renders nothing when no recording is marked.
// ---------------------------------------------------------------------------

interface EditorsChoiceBannerProps {
  work: Work;
}

function EditorsChoiceBanner({ work }: EditorsChoiceBannerProps) {
  const choice = work.recordings.find((r) => r.isEditorsChoice);
  if (!choice) {
    return null;
  }
  const conductor = choice.conductor?.name;
  const performer = choice.orchestras[0]?.name ?? choice.ensemble?.name;
  const year = choice.recordingYear;
  const label = choice.label;
  return (
    <section
      aria-label="Editor's Choice"
      className="rounded-2xl border border-th-accent/40 bg-gradient-to-br from-th-accent/10 to-th-accent/0 p-5"
    >
      <div className="flex flex-wrap items-baseline gap-3">
        <span className="rounded-full bg-th-accent/20 px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest text-th-accent">
          Editor's Choice
        </span>
        {choice.audioQualityTags[0] && (
          <span className="text-[11px] uppercase tracking-wider text-th-text-secondary">
            {choice.audioQualityTags[0]}
          </span>
        )}
      </div>
      <p className="mt-3 text-[16px] font-bold text-th-text-primary">
        {[conductor, performer].filter(Boolean).join(" · ")}
      </p>
      <p className="mt-1 text-[12px] text-th-text-muted tabular-nums">
        {[year, label].filter(Boolean).join(" · ")}
      </p>
      {choice.editorNote && (
        <p className="mt-3 max-w-2xl text-[14px] italic leading-relaxed text-th-text-primary/85">
          {choice.editorNote}
        </p>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// Popular recordings (F9.8) — top 8 by quality + popularity proxy + EC.
// Sub-set distinct from "All recordings"; users typically only need to
// scan the top here before diving into the filter bar below.
// ---------------------------------------------------------------------------

interface PopularRecordingsSectionProps {
  work: Work;
  onEditorialChange: () => void;
}

function PopularRecordingsSection({
  work,
  onEditorialChange,
}: PopularRecordingsSectionProps) {
  const top: Recording[] = useMemo(() => {
    return [...work.recordings]
      .sort((a, b) => {
        // Primary: editor's choice first.
        if (a.isEditorsChoice !== b.isEditorsChoice) {
          return a.isEditorsChoice ? -1 : 1;
        }
        // Secondary: quality_score desc.
        if (b.qualityScore !== a.qualityScore) {
          return b.qualityScore - a.qualityScore;
        }
        // Tertiary: ISRC-bound > inferred > direct > not-found.
        return confidenceWeight(b) - confidenceWeight(a);
      })
      .slice(0, 8);
  }, [work.recordings]);

  if (top.length === 0) {
    return null;
  }

  return (
    <section aria-label="Popular recordings">
      <h2 className="mb-3 text-[15px] font-bold uppercase tracking-[0.18em] text-th-text-secondary">
        Popular recordings
      </h2>
      <ul className="space-y-2">
        {top.map((rec) => (
          <RecordingRow
            key={rec.mbid}
            recording={rec}
            workTitle={work.title}
            onEditorialChange={onEditorialChange}
          />
        ))}
      </ul>
    </section>
  );
}

function confidenceWeight(r: Recording): number {
  switch (r.matchConfidence) {
    case "IsrcBound":
      return 3;
    case "TextSearchInferred":
      return 2;
    case "TidalDirectInferred":
      return 1;
    case "NotFound":
    default:
      return 0;
  }
}
