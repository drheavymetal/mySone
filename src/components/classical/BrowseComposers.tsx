import { useEffect, useMemo, useState } from "react";
import { Search } from "lucide-react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import { listClassicalTopComposers } from "../../api/classical";
import type { ComposerSummary, Era } from "../../types/classical";
import { BROWSEABLE_ERAS, eraLabel } from "../../types/classical";

import ComposerCard from "./ComposerCard";

interface BrowseComposersProps {
  onBack: () => void;
}

type EraFilter = Era | "All";

/**
 * Phase 2 BrowseComposers — full canon list, filterable by era + free
 * text search. Synchronous on the backend (snapshot lookup), so
 * filtering happens entirely client-side after the initial pull.
 *
 * Phase 7 (D-027 / D-031 / F7.0) — switched the initial fetch from 100
 * (OpenOpus canon) to 5000 (extended snapshot, ~6k composers) so the
 * full universe is in memory. Renders in pages of `INITIAL_PAGE_SIZE`
 * client-side; "Load more" appends `PAGE_INCREMENT` cards per click.
 * Search and era filter still run over the entire dataset; the
 * pagination only affects render volume, never filtering scope.
 */
const INITIAL_PAGE_SIZE = 60;
const PAGE_INCREMENT = 60;

export default function BrowseComposers({ onBack }: BrowseComposersProps) {
  const { navigateToClassicalComposer } = useNavigation();
  const [composers, setComposers] = useState<ComposerSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [eraFilter, setEraFilter] = useState<EraFilter>("All");
  const [query, setQuery] = useState("");
  // Phase 7 (F7.0) — client-side pagination cap. Reset whenever the
  // active filter or search changes so the user always sees the top N
  // matches first.
  const [renderCap, setRenderCap] = useState(INITIAL_PAGE_SIZE);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    // Phase 7 — pull the full extended universe. The backend caps at
    // the snapshot length (~6k); the IPC payload is ~2 MB but happens
    // exactly once per session.
    listClassicalTopComposers(5000)
      .then((data) => {
        if (!cancelled) {
          setComposers(data);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          console.error("[classical] BrowseComposers load failed:", err);
          setError(err instanceof Error ? err.message : String(err));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Reset the render cap whenever filters change so the user doesn't
  // have to scroll past stale "Load more" buttons.
  useEffect(() => {
    setRenderCap(INITIAL_PAGE_SIZE);
  }, [eraFilter, query]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return composers.filter((c) => {
      if (eraFilter !== "All" && c.era !== eraFilter) {
        return false;
      }
      if (!q) {
        return true;
      }
      const haystack = `${c.name} ${c.fullName ?? ""}`.toLowerCase();
      return haystack.includes(q);
    });
  }, [composers, eraFilter, query]);

  const visible = useMemo(
    () => filtered.slice(0, renderCap),
    [filtered, renderCap],
  );
  const hasMoreToShow = filtered.length > visible.length;

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-8">
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
          aria-label="Back to Hub"
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
          <h1 className="text-[28px] font-extrabold tracking-tight text-th-text-primary">
            Browse Composers
          </h1>
          <p className="mt-1 text-[13px] text-th-text-muted">
            {composers.length > 0
              ? `${composers.length.toLocaleString()} composers indexed · filter by era or search`
              : "Loading the catalog…"}
          </p>
        </header>

        {/* Search input */}
        <div className="mb-4 flex max-w-md items-center gap-2 rounded-full border border-th-border-subtle/60 bg-th-surface/40 px-4 py-2">
          <Search size={16} className="text-th-text-muted" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter by name…"
            className="flex-1 bg-transparent text-[13px] text-th-text-primary placeholder:text-th-text-muted focus:outline-none"
            aria-label="Filter composers by name"
          />
        </div>

        {/* Era filter pills */}
        <div className="mb-8 flex flex-wrap gap-2">
          <EraPill
            active={eraFilter === "All"}
            onClick={() => setEraFilter("All")}
            label="All eras"
          />
          {BROWSEABLE_ERAS.map((era) => (
            <EraPill
              key={era}
              active={eraFilter === era}
              onClick={() => setEraFilter(era)}
              label={eraLabel(era)}
            />
          ))}
        </div>

        {loading && <BrowseSkeleton />}

        {error && !loading && (
          <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-4 text-[13px] text-red-200">
            Could not load composers: {error}
          </div>
        )}

        {!loading && !error && filtered.length === 0 && (
          <div className="rounded-xl border border-th-border-subtle/60 bg-th-surface/40 p-6 text-center text-[13px] text-th-text-secondary">
            No composers match the current filters.
          </div>
        )}

        {!loading && !error && filtered.length > 0 && (
          <>
            <div className="mb-3 flex items-center justify-between text-[12px] text-th-text-muted">
              <span>
                Showing {visible.length.toLocaleString()} of{" "}
                {filtered.length.toLocaleString()}
                {filtered.length !== composers.length && (
                  <> (filtered from {composers.length.toLocaleString()})</>
                )}
              </span>
            </div>
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
              {visible.map((c) => (
                <ComposerCard
                  key={c.mbid}
                  composer={c}
                  onClick={() =>
                    navigateToClassicalComposer(c.mbid, c.fullName ?? c.name)
                  }
                />
              ))}
            </div>
            {hasMoreToShow && (
              <div className="mt-6 flex justify-center">
                <button
                  type="button"
                  onClick={() => setRenderCap((cap) => cap + PAGE_INCREMENT)}
                  className="rounded-lg border border-th-border-subtle bg-th-surface-hover px-5 py-2 text-[13px] font-medium text-th-text-primary hover:bg-th-surface-hover/80 transition-colors"
                >
                  Load more ({filtered.length - visible.length} remaining)
                </button>
              </div>
            )}
          </>
        )}
      </PageContainer>
    </div>
  );
}

interface EraPillProps {
  active: boolean;
  onClick: () => void;
  label: string;
}

function EraPill({ active, onClick, label }: EraPillProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-3 py-1 text-[12px] font-semibold transition-colors ${
        active
          ? "bg-th-accent text-black shadow"
          : "bg-th-surface/40 text-th-text-secondary hover:bg-th-surface-hover hover:text-th-text-primary"
      }`}
    >
      {label}
    </button>
  );
}

function BrowseSkeleton() {
  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
      {Array.from({ length: 18 }).map((_, idx) => (
        <div
          key={idx}
          className="flex flex-col items-center gap-2 rounded-xl bg-th-elevated p-3"
        >
          <div className="aspect-square w-full animate-pulse rounded-full bg-th-surface/60" />
          <div className="h-3 w-2/3 animate-pulse rounded bg-th-surface/50" />
          <div className="h-2 w-1/3 animate-pulse rounded bg-th-surface/40" />
        </div>
      ))}
    </div>
  );
}
