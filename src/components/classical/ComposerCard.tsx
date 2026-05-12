import { User } from "lucide-react";

import type { ComposerSummary } from "../../types/classical";

import EraBadge from "./EraBadge";

interface ComposerCardProps {
  composer: ComposerSummary;
  onClick: () => void;
  /** Optional override of width — by default the card stretches to fill
   *  the parent grid cell. Use "card-scroll-item" or fixed pixel widths
   *  for horizontal scroll rows. */
  widthClass?: string;
}

function lifeSpan(c: ComposerSummary): string {
  const parts: string[] = [];
  if (c.birthYear) {
    parts.push(String(c.birthYear));
  }
  if (c.deathYear) {
    parts.push(String(c.deathYear));
  } else if (c.birthYear) {
    parts.push("—");
  }
  return parts.join("–");
}

/**
 * Composer card used in the Hub featured grid and the BrowseComposers
 * list. Matches the visual weight of `MediaCard` so the Hub feels
 * native to SONE without copying TIDAL's circular artist treatment.
 */
export default function ComposerCard({
  composer,
  onClick,
  widthClass,
}: ComposerCardProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`group flex flex-col items-stretch gap-2 rounded-xl bg-th-elevated p-3 text-left transition-[background-color,transform] duration-200 hover:-translate-y-0.5 hover:bg-th-surface-hover ${
        widthClass ?? ""
      }`}
      aria-label={`Open composer page: ${composer.fullName ?? composer.name}`}
    >
      {/* Portrait */}
      <div className="relative aspect-square w-full overflow-hidden rounded-full bg-th-surface-hover shadow-md">
        {composer.portraitUrl ? (
          <img
            src={composer.portraitUrl}
            alt=""
            className="h-full w-full object-cover transition-transform duration-500 ease-out group-hover:scale-105"
            loading="lazy"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = "none";
            }}
          />
        ) : (
          <div className="flex h-full w-full items-center justify-center bg-gradient-to-br from-th-button to-th-surface">
            <User size={36} className="text-th-text-faint" />
          </div>
        )}
      </div>

      {/* Name + life */}
      <div className="px-1 pt-1">
        <h3 className="truncate text-center text-[14px] font-bold text-th-text-primary">
          {composer.fullName ?? composer.name}
        </h3>
        <p className="mt-0.5 text-center font-mono text-[11px] text-th-text-muted">
          {lifeSpan(composer)}
        </p>
      </div>

      {/* Era */}
      <div className="flex justify-center">
        <EraBadge era={composer.era} />
      </div>
    </button>
  );
}
