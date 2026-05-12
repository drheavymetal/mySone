import type { Era } from "../../types/classical";
import { eraLabel } from "../../types/classical";

interface EraBadgeProps {
  era: Era;
  /** When false, shows only the colored dot. Defaults to true. */
  showLabel?: boolean;
}

const ERA_PALETTE: Record<Era, { dot: string; text: string; bg: string }> = {
  Medieval: {
    dot: "bg-amber-700",
    text: "text-amber-300",
    bg: "bg-amber-900/30",
  },
  Renaissance: {
    dot: "bg-amber-500",
    text: "text-amber-200",
    bg: "bg-amber-800/30",
  },
  Baroque: {
    dot: "bg-orange-500",
    text: "text-orange-200",
    bg: "bg-orange-900/30",
  },
  Classical: {
    dot: "bg-yellow-400",
    text: "text-yellow-100",
    bg: "bg-yellow-900/30",
  },
  EarlyRomantic: {
    dot: "bg-red-400",
    text: "text-red-200",
    bg: "bg-red-900/30",
  },
  Romantic: {
    dot: "bg-rose-500",
    text: "text-rose-200",
    bg: "bg-rose-900/30",
  },
  LateRomantic: {
    dot: "bg-fuchsia-500",
    text: "text-fuchsia-200",
    bg: "bg-fuchsia-900/30",
  },
  TwentiethCentury: {
    dot: "bg-violet-500",
    text: "text-violet-200",
    bg: "bg-violet-900/30",
  },
  PostWar: {
    dot: "bg-blue-500",
    text: "text-blue-200",
    bg: "bg-blue-900/30",
  },
  Contemporary: {
    dot: "bg-cyan-400",
    text: "text-cyan-100",
    bg: "bg-cyan-900/30",
  },
  Unknown: {
    dot: "bg-th-text-secondary/40",
    text: "text-th-text-secondary",
    bg: "bg-th-surface/40",
  },
};

/**
 * Color-coded era chip used by composer cards, work cards, and the
 * BrowsePeriods grid. Hex palette intentionally walks the time axis
 * (warm → cool) so users get a subtle chronological cue across the Hub.
 */
export default function EraBadge({ era, showLabel = true }: EraBadgeProps) {
  const palette = ERA_PALETTE[era] ?? ERA_PALETTE.Unknown;
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider ${palette.bg} ${palette.text}`}
    >
      <span
        aria-hidden="true"
        className={`inline-block h-1.5 w-1.5 rounded-full ${palette.dot}`}
      />
      {showLabel && eraLabel(era)}
    </span>
  );
}
