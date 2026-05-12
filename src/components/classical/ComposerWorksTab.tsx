import { useEffect, useMemo, useState } from "react";

import { listClassicalComposerBuckets } from "../../api/classical";
import { useNavigation } from "../../hooks/useNavigation";
import type {
  BucketSummary,
  ComposerBuckets,
  WorkSummary,
} from "../../types/classical";

import BucketSection from "./BucketSection";
import WorkSummaryCard from "./WorkSummaryCard";

/**
 * Phase 9 (F9.2 / D-040 / D-043) — Works tab content. Drives the new
 * composer page renderer end-to-end:
 *
 *   ┌─ Tab: Works ────────────────────────────────────────────┐
 *   │  ── Essentials ───────────────  (cherry-picked, 4-8)    │
 *   │  ── Symphonies (9) ─────────────────  [View all (9)]    │
 *   │  ── Concertos (7) ──────────────────  [View all (7)]    │
 *   │     Filter: [All] [Piano (5)] [Violin (1)] [Triple (1)] │
 *   │  ...                                                     │
 *   └──────────────────────────────────────────────────────────┘
 *
 * Essentials are cherry-picked from `popular=true` works across all
 * buckets — same as the Phase 7 layout, kept above the bucket grid
 * so the user has a fast entry point even before scanning the
 * taxonomy.
 *
 * Empty buckets do NOT render. `Other` and `FilmTheatre` render at
 * the bottom and stay collapsed (`<details>`) when their count is
 * non-zero.
 */

interface ComposerWorksTabProps {
  composerMbid: string;
}

export default function ComposerWorksTab({ composerMbid }: ComposerWorksTabProps) {
  const { navigateToClassicalWork } = useNavigation();
  const [data, setData] = useState<ComposerBuckets | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setData(null);

    listClassicalComposerBuckets(composerMbid)
      .then((d) => {
        if (cancelled) {
          return;
        }
        setData(d);
        setLoading(false);
      })
      .catch((e: unknown) => {
        if (cancelled) {
          return;
        }
        const msg = e instanceof Error ? e.message : String(e);
        console.error("[classical] composer-buckets fetch failed:", e);
        setError(msg);
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [composerMbid]);

  const essentials = useMemo(() => {
    if (!data) {
      return [];
    }
    const all: WorkSummary[] = [];
    for (const bucket of data.buckets) {
      for (const w of bucket.topWorks) {
        if (w.popular && all.findIndex((x) => x.mbid === w.mbid) === -1) {
          all.push(w);
        }
      }
    }
    return all.slice(0, 8);
  }, [data]);

  if (loading) {
    return (
      <div className="space-y-10">
        {Array.from({ length: 4 }).map((_, idx) => (
          <div key={idx} className="space-y-3">
            <div className="h-4 w-40 animate-pulse rounded bg-th-surface/50" />
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 md:grid-cols-6">
              {Array.from({ length: 6 }).map((_, j) => (
                <div
                  key={j}
                  className="aspect-square animate-pulse rounded-md bg-th-surface/40"
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-6 text-red-200">
        <h3 className="text-[14px] font-semibold">Could not load works</h3>
        <p className="mt-2 text-[13px] text-red-200/80">{error}</p>
      </div>
    );
  }

  if (!data || data.buckets.length === 0) {
    return (
      <div className="rounded-xl border border-th-border-subtle/60 bg-th-surface/40 p-6 text-center text-[13px] text-th-text-secondary">
        No works available in MusicBrainz for this composer yet.
      </div>
    );
  }

  // Split: visible buckets render in canonical order; trailing
  // condition-only buckets (FilmTheatre, Other) collapse into details.
  const primary: BucketSummary[] = [];
  const trailing: BucketSummary[] = [];
  for (const b of data.buckets) {
    if (b.bucket === "FilmTheatre" || b.bucket === "Other") {
      trailing.push(b);
    } else {
      primary.push(b);
    }
  }

  return (
    <div className="space-y-12">
      {/* Essentials — cherry-picked across buckets. */}
      {essentials.length > 0 && (
        <section>
          <h2 className="mb-4 text-[15px] font-bold uppercase tracking-[0.18em] text-th-text-secondary">
            Essentials
          </h2>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-4">
            {essentials.map((w) => (
              <WorkSummaryCard
                key={w.mbid}
                work={w}
                onClick={() => navigateToClassicalWork(w.mbid, w.title)}
              />
            ))}
          </div>
        </section>
      )}

      {/* Primary buckets (Stage → SoloInstrumental). */}
      {primary.map((bucket) => (
        <BucketSection
          key={bucket.bucket}
          composerMbid={composerMbid}
          bucket={bucket}
        />
      ))}

      {/* Trailing buckets — Other / FilmTheatre, collapsed by default. */}
      {trailing.map((bucket) => (
        <details
          key={bucket.bucket}
          className="rounded-xl border border-th-border-subtle/40 bg-th-surface/30 p-4 [&_summary]:cursor-pointer"
        >
          <summary className="flex items-center justify-between text-[14px] font-semibold text-th-text-secondary">
            <span>
              {bucket.labelEn}{" "}
              <span className="font-normal text-th-text-muted">
                ({bucket.totalCount})
              </span>
            </span>
            <span className="text-[11px] text-th-text-muted">expand</span>
          </summary>
          <div className="mt-4">
            <BucketSection
              composerMbid={composerMbid}
              bucket={bucket}
              hideHeader
            />
          </div>
        </details>
      ))}
    </div>
  );
}
