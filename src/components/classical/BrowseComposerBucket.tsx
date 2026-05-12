import { useEffect, useMemo, useState } from "react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import {
  getClassicalComposer,
  listClassicalWorksInBucket,
} from "../../api/classical";
import type {
  Composer,
  WorkBucket,
  WorkSummary,
  SubBucketSummary,
} from "../../types/classical";
import { listClassicalComposerBuckets } from "../../api/classical";
import { workBucketLabel } from "../../types/classical";

import SubBucketChips from "./SubBucketChips";
import WorkSummaryCard from "./WorkSummaryCard";

/**
 * Phase 9 (F9.3 / D-040 / D-043) — drill-down page for a single
 * bucket of a composer's catalogue. Reachable via
 * `classical://composer/{mbid}/bucket/{bucket}` (a "View all" click
 * inside `BucketSection`).
 *
 * Loads the bucket's full work list, exposes:
 *   • Sub-bucket filter chips (powered by the parent bucket's
 *     server-computed sub-bucket palette).
 *   • Sort modes: Catalog (default) / Date / Alphabetical. Catalog is
 *     the editorial default when most works carry catalogue numbers
 *     (Bach BWV, Mozart K., Beethoven Op.); other composers fall to
 *     Alphabetical implicitly via the catalog sort key.
 *   • Optional secondary filter "with Editor's Choice" (client-side,
 *     against the snapshot via `popular` proxy — V1 approximation).
 *
 * Pagination is internal: the backend returns up to 50 per call;
 * "Load more" advances `offset`. Sub-bucket / sort changes reset to
 * offset=0.
 */

interface BrowseComposerBucketProps {
  composerMbid: string;
  bucket: WorkBucket;
  onBack: () => void;
}

type SortMode = "Catalog" | "Date" | "Alphabetical";

const SORT_MODES: SortMode[] = ["Catalog", "Date", "Alphabetical"];

export default function BrowseComposerBucket({
  composerMbid,
  bucket,
  onBack,
}: BrowseComposerBucketProps) {
  const { navigateToClassicalWork } = useNavigation();
  const [composer, setComposer] = useState<Composer | null>(null);
  const [subBuckets, setSubBuckets] = useState<SubBucketSummary[]>([]);
  const [activeSub, setActiveSub] = useState<string | null>(null);
  const [sort, setSort] = useState<SortMode>("Catalog");
  const [works, setWorks] = useState<WorkSummary[]>([]);
  const [total, setTotal] = useState<number>(0);
  const [hasMore, setHasMore] = useState<boolean>(false);
  const [offset, setOffset] = useState<number>(0);
  const [loading, setLoading] = useState<boolean>(true);
  const [loadingMore, setLoadingMore] = useState<boolean>(false);

  // Composer + bucket palette load on first mount.
  useEffect(() => {
    let cancelled = false;
    setComposer(null);
    setSubBuckets([]);
    Promise.all([
      getClassicalComposer(composerMbid).catch((e: unknown) => {
        console.warn("[classical] composer fetch (bucket page):", e);
        return null;
      }),
      listClassicalComposerBuckets(composerMbid).catch((e: unknown) => {
        console.warn("[classical] composer-buckets fetch (bucket page):", e);
        return null;
      }),
    ]).then(([comp, all]) => {
      if (cancelled) {
        return;
      }
      setComposer(comp);
      const here = all?.buckets.find((b) => b.bucket === bucket);
      setSubBuckets(here?.subBuckets ?? []);
    });
    return () => {
      cancelled = true;
    };
  }, [composerMbid, bucket]);

  // Works load whenever filter/sort changes.
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setWorks([]);
    setOffset(0);

    listClassicalWorksInBucket(composerMbid, bucket, {
      subBucket: activeSub,
      sort,
      offset: 0,
      limit: 50,
    })
      .then((page) => {
        if (cancelled) {
          return;
        }
        setWorks(page.works);
        setTotal(page.total);
        setHasMore(page.hasMore);
        setOffset(page.works.length);
        setLoading(false);
      })
      .catch((e: unknown) => {
        if (cancelled) {
          return;
        }
        console.error("[classical] bucket works fetch failed:", e);
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [composerMbid, bucket, activeSub, sort]);

  const loadMore = async () => {
    if (loadingMore || !hasMore) {
      return;
    }
    setLoadingMore(true);
    try {
      const page = await listClassicalWorksInBucket(composerMbid, bucket, {
        subBucket: activeSub,
        sort,
        offset,
        limit: 50,
      });
      setWorks((prev) => {
        const seen = new Set(prev.map((w) => w.mbid));
        const merged = [...prev];
        for (const w of page.works) {
          if (!seen.has(w.mbid)) {
            merged.push(w);
            seen.add(w.mbid);
          }
        }
        return merged;
      });
      setHasMore(page.hasMore);
      setOffset(offset + page.works.length);
    } catch (e: unknown) {
      console.error("[classical] bucket load-more failed:", e);
    } finally {
      setLoadingMore(false);
    }
  };

  const composerName = composer?.fullName ?? composer?.name ?? "Composer";
  const bucketLabel = useMemo(() => workBucketLabel(bucket), [bucket]);

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-8">
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

        <header className="mb-6">
          <p className="text-[12px] font-bold uppercase tracking-widest text-th-text-secondary">
            {composerName}
          </p>
          <h1 className="mt-1 text-[36px] font-extrabold leading-tight tracking-tight text-th-text-primary">
            {bucketLabel}{" "}
            <span className="ml-1 text-[18px] font-normal text-th-text-muted">
              ({total})
            </span>
          </h1>
        </header>

        <div className="mb-6 flex flex-wrap items-center justify-between gap-4">
          {subBuckets.length > 1 ? (
            <SubBucketChips
              subBuckets={subBuckets}
              active={activeSub}
              onChange={setActiveSub}
            />
          ) : (
            <div />
          )}
          <div className="flex items-center gap-2 text-[12px]">
            <span className="text-th-text-muted">Sort</span>
            <select
              value={sort}
              onChange={(e) => setSort(e.target.value as SortMode)}
              className="rounded-md border border-th-border-subtle bg-th-elevated px-2 py-1 text-th-text-primary"
              aria-label="Sort works"
            >
              {SORT_MODES.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </div>
        </div>

        {loading && (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 md:grid-cols-6">
            {Array.from({ length: 12 }).map((_, idx) => (
              <div
                key={idx}
                className="aspect-square animate-pulse rounded-md bg-th-surface/40"
              />
            ))}
          </div>
        )}

        {!loading && works.length === 0 && (
          <div className="rounded-xl border border-th-border-subtle/60 bg-th-surface/40 p-6 text-center text-[13px] text-th-text-secondary">
            No works in this bucket{activeSub ? ` (${activeSub})` : ""}.
          </div>
        )}

        {!loading && works.length > 0 && (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
            {works.map((w) => (
              <WorkSummaryCard
                key={w.mbid}
                work={w}
                onClick={() => navigateToClassicalWork(w.mbid, w.title)}
              />
            ))}
          </div>
        )}

        {hasMore && !loading && (
          <div className="mt-6 flex justify-center">
            <button
              type="button"
              onClick={loadMore}
              disabled={loadingMore}
              className="rounded-lg border border-th-border-subtle bg-th-surface-hover px-6 py-2 text-[13px] font-medium text-th-text-primary hover:bg-th-surface-hover/80 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {loadingMore ? "Loading…" : "Load more"}
            </button>
          </div>
        )}
      </PageContainer>
    </div>
  );
}
