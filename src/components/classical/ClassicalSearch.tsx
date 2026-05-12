import { useEffect, useMemo, useRef, useState } from "react";
import { Search, Star } from "lucide-react";

import PageContainer from "../PageContainer";
import { useNavigation } from "../../hooks/useNavigation";
import { searchClassical } from "../../api/classical";
import type {
  SearchHit,
  SearchPlan,
  SearchResults,
  SearchToken,
} from "../../types/classical";

interface ClassicalSearchProps {
  initialQuery?: string;
  onBack: () => void;
}

/**
 * Phase 5 (F5.1) — Classical search UI. Uses the tokenized + planned
 * backend search (D-019). Renders detected tokens as chips so the user
 * can see what the system understood.
 *
 * Read-only: this component never touches audio routing. Clicking a
 * result navigates to the WorkPage which then drives playback through
 * the existing `usePlaybackActions` path.
 */
export default function ClassicalSearch({
  initialQuery,
  onBack,
}: ClassicalSearchProps) {
  const { navigateToClassicalWork } = useNavigation();
  const [query, setQuery] = useState(initialQuery ?? "");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Auto-run on mount if we received an initial query (deep link from
  // the Hub home → "search this composer" affordance).
  useEffect(() => {
    if (initialQuery && initialQuery.trim().length > 0) {
      void runSearch(initialQuery);
    }
    inputRef.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const runSearch = async (q: string) => {
    const trimmed = q.trim();
    if (trimmed.length === 0) {
      setResults(null);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const r = await searchClassical(trimmed, 20);
      setResults(r);
    } catch (err: unknown) {
      console.error("[classical] search failed:", err);
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      setResults(null);
    } finally {
      setLoading(false);
    }
  };

  const handleChange = (next: string) => {
    setQuery(next);
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    debounceRef.current = setTimeout(() => {
      void runSearch(next);
    }, 350);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
    void runSearch(query);
  };

  const groupedHits = useMemo(() => {
    if (!results) {
      return { primary: [] as SearchHit[], secondary: [] as SearchHit[] };
    }
    const all = results.hits;
    if (all.length === 0) {
      return { primary: [] as SearchHit[], secondary: [] as SearchHit[] };
    }
    const top = all[0];
    const primary: SearchHit[] = [top];
    const secondary: SearchHit[] = [];
    for (const hit of all.slice(1)) {
      if (hit.workMbid === top.workMbid) {
        // Duplicate work — should not happen per backend contract,
        // but guard against it.
        continue;
      }
      // Treat hits within 0.1 of the top score as part of the "best
      // match" group; the rest go to "more results".
      if (Math.abs(top.score - hit.score) <= 0.1) {
        primary.push(hit);
      } else {
        secondary.push(hit);
      }
    }
    return { primary, secondary };
  }, [results]);

  return (
    <div className="flex-1 bg-gradient-to-b from-th-surface to-th-base min-h-full overflow-y-auto">
      <PageContainer className="px-8 py-10">
        {/* Back affordance */}
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
          Back to Hub
        </button>

        <header className="mb-6">
          <h1 className="text-[28px] font-extrabold tracking-tight text-th-text-primary">
            Classical search
          </h1>
          <p className="mt-1 text-[13px] text-th-text-muted">
            Try "Beethoven 9 Karajan 1962", "BWV 1052", or "Op. 125".
          </p>
        </header>

        <form onSubmit={handleSubmit} className="mb-6">
          <div className="relative flex items-center">
            <Search
              size={18}
              aria-hidden="true"
              className="pointer-events-none absolute left-4 text-th-text-muted"
            />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => handleChange(e.target.value)}
              placeholder="Search classical works, composers, catalogue numbers..."
              className="w-full rounded-full border border-th-border-subtle/60 bg-th-surface/50 py-3 pl-11 pr-4 text-[14px] text-th-text-primary placeholder:text-th-text-faint focus:border-th-accent/60 focus:outline-none focus:ring-1 focus:ring-th-accent/30"
              aria-label="Classical search query"
            />
          </div>
        </form>

        {/* Detected tokens */}
        {results && results.plan.tokens.length > 0 && (
          <DetectedTokens plan={results.plan} />
        )}

        {loading && <SearchSkeleton />}

        {error && !loading && (
          <div className="rounded-2xl border border-red-500/40 bg-red-500/10 p-6 text-red-200">
            <h2 className="text-[15px] font-semibold">Search failed</h2>
            <p className="mt-2 text-[13px] text-red-200/80">{error}</p>
          </div>
        )}

        {!loading && !error && results && results.hits.length === 0 && (
          <div className="rounded-2xl border border-th-border-subtle/60 bg-th-surface/40 p-6 text-center text-[13px] text-th-text-secondary">
            No matches in the catalogue snapshot. Try a different composer
            surname or catalogue number.
          </div>
        )}

        {!loading && !error && groupedHits.primary.length > 0 && (
          <section aria-labelledby="best-match-heading" className="mb-8">
            <h2
              id="best-match-heading"
              className="mb-3 text-[12px] font-bold uppercase tracking-[0.18em] text-th-text-secondary"
            >
              Best match
            </h2>
            <ul className="space-y-2">
              {groupedHits.primary.map((hit) => (
                <SearchHitRow
                  key={hit.workMbid}
                  hit={hit}
                  onSelect={() =>
                    navigateToClassicalWork(hit.workMbid, hit.title)
                  }
                />
              ))}
            </ul>
          </section>
        )}

        {!loading && !error && groupedHits.secondary.length > 0 && (
          <section aria-labelledby="more-results-heading">
            <h2
              id="more-results-heading"
              className="mb-3 text-[12px] font-bold uppercase tracking-[0.18em] text-th-text-secondary"
            >
              More results
            </h2>
            <ul className="space-y-2">
              {groupedHits.secondary.map((hit) => (
                <SearchHitRow
                  key={hit.workMbid}
                  hit={hit}
                  onSelect={() =>
                    navigateToClassicalWork(hit.workMbid, hit.title)
                  }
                />
              ))}
            </ul>
          </section>
        )}
      </PageContainer>
    </div>
  );
}

interface DetectedTokensProps {
  plan: SearchPlan;
}

function DetectedTokens({ plan }: DetectedTokensProps) {
  const chips: Array<{ label: string; value: string }> = [];
  if (plan.composerName) {
    chips.push({ label: "composer", value: plan.composerName });
  }
  if (plan.catalogue) {
    chips.push({ label: "catalogue", value: plan.catalogue.display });
  }
  if (plan.year) {
    chips.push({ label: "year", value: String(plan.year) });
  }
  if (plan.key) {
    chips.push({ label: "key", value: plan.key });
  }
  if (plan.keywords.length > 0) {
    // Per-token chips from the kept tokens, in order.
    for (const tok of plan.tokens) {
      if (isKeywordToken(tok)) {
        chips.push({ label: "keyword", value: tok.value });
      }
    }
  }
  if (chips.length === 0) {
    return null;
  }
  return (
    <div className="mb-6 flex flex-wrap items-center gap-2 text-[12px]">
      <span className="font-bold uppercase tracking-wider text-th-text-secondary">
        Detected
      </span>
      {chips.map((c, idx) => (
        <span
          key={`${c.label}:${c.value}:${idx}`}
          className="rounded-full border border-th-border-subtle/60 bg-th-surface/50 px-2.5 py-0.5 text-th-text-secondary"
          title={c.label}
        >
          <span className="text-th-text-faint">{c.label}:</span>{" "}
          <span className="text-th-text-primary">{c.value}</span>
        </span>
      ))}
    </div>
  );
}

function isKeywordToken(
  token: SearchToken,
): token is Extract<SearchToken, { kind: "Keyword" }> {
  return token.kind === "Keyword";
}

interface SearchHitRowProps {
  hit: SearchHit;
  onSelect: () => void;
}

function SearchHitRow({ hit, onSelect }: SearchHitRowProps) {
  const subtitleParts: string[] = [];
  if (hit.composerName) {
    subtitleParts.push(hit.composerName);
  }
  if (hit.catalogueDisplay) {
    subtitleParts.push(hit.catalogueDisplay);
  }
  return (
    <li>
      <button
        type="button"
        onClick={onSelect}
        className="flex w-full items-center gap-3 rounded-xl border border-th-border-subtle/60 bg-th-surface/40 px-4 py-3 text-left transition-colors hover:border-th-accent/40 hover:bg-th-surface/70"
      >
        <Star
          size={16}
          aria-hidden="true"
          className="shrink-0 text-th-text-muted"
        />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[14px] font-semibold text-th-text-primary">
            {hit.title}
          </p>
          {subtitleParts.length > 0 && (
            <p className="mt-0.5 truncate text-[12px] text-th-text-secondary">
              {subtitleParts.join(" · ")}
            </p>
          )}
        </div>
        <span
          className="font-mono text-[11px] text-th-text-faint"
          title={`Score: ${hit.score.toFixed(2)} · ${hit.source}`}
        >
          {hit.score.toFixed(2)}
        </span>
      </button>
    </li>
  );
}

function SearchSkeleton() {
  return (
    <div className="space-y-2">
      {Array.from({ length: 5 }).map((_, idx) => (
        <div
          key={idx}
          className="h-14 animate-pulse rounded-xl bg-th-surface/40"
        />
      ))}
    </div>
  );
}
