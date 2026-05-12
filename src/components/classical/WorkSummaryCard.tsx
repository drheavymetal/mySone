import { Music } from "lucide-react";

import type { WorkSummary } from "../../types/classical";
import { workTypeLabel } from "../../types/classical";

interface WorkSummaryCardProps {
  work: WorkSummary;
  onClick: () => void;
  widthClass?: string;
}

function buildSubtitle(work: WorkSummary): string {
  const parts: string[] = [];
  if (work.catalogueNumber) {
    parts.push(work.catalogueNumber.display);
  }
  if (work.key) {
    parts.push(work.key);
  }
  if (work.compositionYear) {
    parts.push(String(work.compositionYear));
  }
  return parts.join(" · ");
}

/**
 * Compact work card for Composer page sections + the "Essentials" rows.
 * Uses an iconic placeholder rather than cover art — Phase 2 doesn't
 * have a way to attach work-specific imagery (Phase 5 will surface
 * Editor's Choice cover when available).
 */
export default function WorkSummaryCard({
  work,
  onClick,
  widthClass,
}: WorkSummaryCardProps) {
  const subtitle = buildSubtitle(work);
  const typeBadge = work.workType ? workTypeLabel(work.workType) : null;

  return (
    <button
      type="button"
      onClick={onClick}
      className={`group flex flex-col gap-2 rounded-xl bg-th-elevated p-3 text-left transition-[background-color,transform] duration-200 hover:-translate-y-0.5 hover:bg-th-surface-hover ${
        widthClass ?? ""
      }`}
      aria-label={`Open work: ${work.title}`}
    >
      {/* Iconic placeholder block */}
      <div className="relative flex aspect-square w-full items-center justify-center overflow-hidden rounded-md bg-gradient-to-br from-th-surface to-th-base">
        <Music size={42} className="text-th-text-faint/50" />
        {work.popular && (
          <span className="absolute right-2 top-2 rounded-full bg-th-accent/90 px-1.5 py-0.5 text-[9px] font-black uppercase tracking-wider text-black">
            Popular
          </span>
        )}
        {typeBadge && (
          <span className="absolute bottom-2 left-2 rounded-md bg-black/60 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-white">
            {typeBadge}
          </span>
        )}
      </div>

      {/* Title */}
      <div className="px-1">
        <h4 className="line-clamp-2 text-[13px] font-bold text-th-text-primary">
          {work.title}
        </h4>
        {subtitle && (
          <p className="mt-1 truncate text-[11px] text-th-text-muted">
            {subtitle}
          </p>
        )}
      </div>
    </button>
  );
}
