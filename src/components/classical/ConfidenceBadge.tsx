import type { MatchConfidence } from "../../types/classical";

interface ConfidenceBadgeProps {
  confidence: MatchConfidence;
  /** When confidence is `TextSearchInferred`, the canonical query that
   *  produced the hit. Surfaced through `title` so a hover reveals it
   *  without crowding the row. */
  query?: string;
  score?: number;
}

/**
 * Per-row confidence indicator for the cascade matcher (D-010 + D-037).
 *
 *   IsrcBound           → green dot, "ISRC" label
 *   TextSearchInferred  → amber dot, "Inferred" label, tooltip with query
 *   TidalDirectInferred → orange dot, "Tidal direct" label, tooltip with
 *                         query (D-037, bug 3 fix — work-level fallback)
 *   NotFound            → grey dot, "Not on Tidal" label, no play button
 *
 * The badge is purely informational: actions (play / favorite) are
 * disabled by `RecordingRow` based on the same flag.
 */
export default function ConfidenceBadge({
  confidence,
  query,
  score,
}: ConfidenceBadgeProps) {
  if (confidence === "IsrcBound") {
    return (
      <span
        className="inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-emerald-300/90"
        aria-label="ISRC matched (high confidence)"
      >
        <span
          aria-hidden="true"
          className="inline-block h-2 w-2 rounded-full bg-emerald-400 shadow-[0_0_8px_rgba(74,222,128,0.6)]"
        />
        ISRC
      </span>
    );
  }

  if (confidence === "TextSearchInferred") {
    const tooltip = score !== undefined && query
      ? `Inferred match — query: "${query}" (score ${score.toFixed(2)})`
      : query
        ? `Inferred match — query: "${query}"`
        : "Inferred match";
    return (
      <span
        className="inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-amber-300/90"
        title={tooltip}
        aria-label={tooltip}
      >
        <span
          aria-hidden="true"
          className="inline-block h-2 w-2 rounded-full bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.55)]"
        />
        Inferred
      </span>
    );
  }

  if (confidence === "TidalDirectInferred") {
    // D-037 (bug 3 fix) — work-level fallback. MB had no recordings or
    // none matched; we ran a Tidal-only search and got a plausible hit.
    // Confidence is lower than TextSearchInferred (no artist anchor),
    // so the badge is visually softer (orange vs amber) and the tooltip
    // is more explicit about the path taken.
    const tooltip = score !== undefined && query
      ? `Tidal direct match (work-level fallback) — query: "${query}" (score ${score.toFixed(2)})`
      : query
        ? `Tidal direct match (work-level fallback) — query: "${query}"`
        : "Tidal direct match (work-level fallback)";
    return (
      <span
        className="inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-orange-300/90"
        title={tooltip}
        aria-label={tooltip}
      >
        <span
          aria-hidden="true"
          className="inline-block h-2 w-2 rounded-full bg-orange-400 shadow-[0_0_8px_rgba(251,146,60,0.55)]"
        />
        Tidal direct
      </span>
    );
  }

  return (
    <span
      className="inline-flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-th-text-secondary/70"
      title="Not available on Tidal — info only"
      aria-label="Not available on Tidal"
    >
      <span
        aria-hidden="true"
        className="inline-block h-2 w-2 rounded-full bg-th-text-secondary/40"
      />
      Not on Tidal
    </span>
  );
}
