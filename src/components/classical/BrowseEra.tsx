import { useEffect, useMemo, useState } from "react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import { listClassicalComposersByEra } from "../../api/classical";
import type { ComposerSummary, Era } from "../../types/classical";
import { eraLabel, eraYearSpan } from "../../types/classical";

import ComposerCard from "./ComposerCard";
import EraBadge from "./EraBadge";

interface BrowseEraProps {
  era: string;
  onBack: () => void;
}

/** Recognised era literals — keeps the URL parser strict. */
const VALID_ERAS = new Set<string>([
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
  "Unknown",
]);

/**
 * Phase 2 era drill-down. Reached from BrowsePeriods via
 * `classical://era/{era}`. Lists every composer in the snapshot whose
 * era bucket matches; click any composer → ComposerPage.
 */
export default function BrowseEra({ era, onBack }: BrowseEraProps) {
  const { navigateToClassicalComposer } = useNavigation();
  const [composers, setComposers] = useState<ComposerSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const validEra = useMemo<Era | null>(() => {
    return VALID_ERAS.has(era) ? (era as Era) : null;
  }, [era]);

  useEffect(() => {
    if (!validEra) {
      setError(`Unknown era: ${era}`);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    listClassicalComposersByEra(validEra)
      .then((data) => {
        if (!cancelled) {
          setComposers(data);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          console.error("[classical] BrowseEra load failed:", err);
          setError(err instanceof Error ? err.message : String(err));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [validEra, era]);

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-8">
        <button
          type="button"
          onClick={onBack}
          className="mb-6 inline-flex items-center gap-1 text-[13px] font-medium text-th-text-secondary hover:text-th-text-primary transition-colors"
          aria-label="Back to Browse Periods"
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

        <header className="mb-8">
          <div className="flex items-center gap-3">
            {validEra && <EraBadge era={validEra} showLabel={false} />}
            <h1 className="text-[28px] font-extrabold tracking-tight text-th-text-primary">
              {validEra ? eraLabel(validEra) : era}
            </h1>
            <span className="font-mono text-[13px] text-th-text-muted">
              {validEra ? eraYearSpan(validEra) : ""}
            </span>
          </div>
          <p className="mt-1 text-[13px] text-th-text-muted">
            {!loading && composers.length > 0
              ? `${composers.length} composers in this era`
              : "Composers in this era"}
          </p>
        </header>

        {loading && <BrowseSkeleton />}

        {error && !loading && (
          <div className="rounded-xl border border-red-500/40 bg-red-500/10 p-4 text-[13px] text-red-200">
            {error}
          </div>
        )}

        {!loading && !error && composers.length === 0 && (
          <div className="rounded-xl border border-th-border-subtle/60 bg-th-surface/40 p-6 text-center text-[13px] text-th-text-secondary">
            No composers in this era yet — the snapshot covers the canon top.
            Phase 6 will widen coverage.
          </div>
        )}

        {!loading && !error && composers.length > 0 && (
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
            {composers.map((c) => (
              <ComposerCard
                key={c.mbid}
                composer={c}
                onClick={() =>
                  navigateToClassicalComposer(c.mbid, c.fullName ?? c.name)
                }
              />
            ))}
          </div>
        )}
      </PageContainer>
    </div>
  );
}

function BrowseSkeleton() {
  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
      {Array.from({ length: 12 }).map((_, idx) => (
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
