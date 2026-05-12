import { useMemo, useState } from "react";

import { useNavigation } from "../../hooks/useNavigation";
import type { BucketSummary, WorkSummary } from "../../types/classical";

import WorkSummaryCard from "./WorkSummaryCard";
import SubBucketChips from "./SubBucketChips";

/**
 * Phase 9 (F9.2 / D-040 / D-043) — single bucket section inside the
 * Works tab. Renders:
 *
 *   • Header with `{label} ({count})` and a "View all" pill when
 *     `totalCount > 12`.
 *   • Optional sub-bucket filter chips when the parent bucket
 *     produced sub-buckets server-side. Filtering is client-side over
 *     `topWorks` — drill-down into the full set goes through
 *     `BrowseComposerBucket`.
 *   • A 12-card grid (the cap is enforced server-side; we render
 *     whatever `topWorks` carries).
 */

interface BucketSectionProps {
  composerMbid: string;
  bucket: BucketSummary;
  /** Used by `<details>`-collapsed trailing buckets (Other,
   *  FilmTheatre) to skip the header — the surrounding `<summary>`
   *  already renders one. */
  hideHeader?: boolean;
}

/** Map a `WorkSummary` to its sub-bucket label using the same
 *  heuristics as the backend's `sub_bucket_for_work`. Kept duplicated
 *  here because the backend doesn't ship the per-work sub-bucket as
 *  a field on `WorkSummary` — we recompute on demand for the chip
 *  filter. The chip *counts* are authoritative (came from the
 *  server); per-work labels here are best-effort, used only for
 *  client-side filtering of the visible 12. */
function clientSubBucketFor(bucket: BucketSummary["bucket"], w: WorkSummary): string {
  const lower = w.title.toLowerCase();
  switch (bucket) {
    case "Concertos": {
      if (lower.includes("piano concerto") || lower.includes("for piano and orchestra")) {
        return "Piano";
      }
      if (lower.includes("violin concerto") || lower.includes("for violin and orchestra")) {
        return "Violin";
      }
      if (lower.includes("cello concerto") || lower.includes("for cello and orchestra")) {
        return "Cello";
      }
      return "Other";
    }
    case "Chamber": {
      if (lower.includes("string quartet") || lower.includes("quartet")) {
        return "Quartets";
      }
      if (lower.includes("trio")) {
        return "Trios";
      }
      if (lower.includes("quintet")) {
        return "Quintets";
      }
      if (lower.includes("sonata")) {
        return "Sonatas";
      }
      return "Other";
    }
    case "Keyboard": {
      if (lower.includes("sonata")) {
        return "Sonatas";
      }
      if (lower.includes("variation")) {
        return "Variations";
      }
      if (lower.includes("étude") || lower.includes("etude")) {
        return "Études";
      }
      if (
        lower.includes("nocturne") ||
        lower.includes("mazurka") ||
        lower.includes("polonaise") ||
        lower.includes("ballade") ||
        lower.includes("impromptu") ||
        lower.includes("prelude") ||
        lower.includes("fugue")
      ) {
        return "Character pieces";
      }
      return "Other";
    }
    case "SoloInstrumental": {
      if (lower.includes("violin") || lower.includes("partita")) {
        return "Violin";
      }
      if (lower.includes("cello")) {
        return "Cello";
      }
      return "Other";
    }
    case "ChoralSacred": {
      if (lower.startsWith("mass") || lower.includes(" mass ") || lower.includes("missa")) {
        return "Mass";
      }
      if (lower.includes("requiem")) {
        return "Requiem";
      }
      if (lower.includes("cantata")) {
        return "Cantata";
      }
      if (lower.includes("passion")) {
        return "Passion";
      }
      return "Other";
    }
    default:
      return "Other";
  }
}

export default function BucketSection({
  composerMbid,
  bucket,
  hideHeader,
}: BucketSectionProps) {
  const { navigateToClassicalWork, navigateToClassicalBucket } = useNavigation();
  const [activeSub, setActiveSub] = useState<string | null>(null);

  const filteredTopWorks = useMemo(() => {
    if (!activeSub || !bucket.subBuckets) {
      return bucket.topWorks;
    }
    return bucket.topWorks.filter(
      (w) => clientSubBucketFor(bucket.bucket, w) === activeSub,
    );
  }, [activeSub, bucket]);

  const showViewAll = bucket.totalCount > bucket.topWorks.length;

  return (
    <section>
      {!hideHeader && (
        <div className="mb-4 flex items-baseline justify-between">
          <h2 className="text-[18px] font-bold tracking-tight text-th-text-primary">
            {bucket.labelEn}{" "}
            <span className="ml-1 text-[13px] font-normal text-th-text-muted">
              ({bucket.totalCount})
            </span>
          </h2>
          {showViewAll && (
            <button
              type="button"
              onClick={() =>
                navigateToClassicalBucket(composerMbid, bucket.bucket)
              }
              className="text-[12px] font-semibold text-th-accent transition-opacity hover:opacity-80"
            >
              View all ({bucket.totalCount})
            </button>
          )}
        </div>
      )}

      {bucket.subBuckets && bucket.subBuckets.length > 1 && (
        <SubBucketChips
          subBuckets={bucket.subBuckets}
          active={activeSub}
          onChange={setActiveSub}
        />
      )}

      {filteredTopWorks.length === 0 && activeSub && (
        <p className="text-[12px] text-th-text-muted">
          No works visible under "{activeSub}" in this top-12. The full
          drill-down may still contain entries — click "View all".
        </p>
      )}

      {filteredTopWorks.length > 0 && (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {filteredTopWorks.map((w) => (
            <WorkSummaryCard
              key={w.mbid}
              work={w}
              onClick={() => navigateToClassicalWork(w.mbid, w.title)}
            />
          ))}
        </div>
      )}
    </section>
  );
}
