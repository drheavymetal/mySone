import { memo } from "react";

import { useClassicalContext } from "../../hooks/useClassicalContext";
import { useNavigation } from "../../hooks/useNavigation";

/**
 * Phase 3 (F3.1 + F3.2 + F3.3): persistent classical work header for
 * the player bar, rendered above the track title when the current
 * track is recognized as part of a known Work.
 *
 * Format:
 *
 *     Composer · Work title · II / IV · Attacca →
 *
 * Each piece is independently nullable:
 *   - Composer + Work title appear together once the Work entity is
 *     hydrated.
 *   - "II / IV" only when `movement` resolved.
 *   - "Attacca →" only when `movement.attaccaTo` is set.
 *
 * Hidden entirely until at least the Work entity is known. No
 * skeleton — we don't want a placeholder flashing in the player.
 *
 * The header is purely cosmetic. It does not touch audio routing,
 * volume, scrobble, or any other live state — it's a read-only
 * consumer of the catalog + scrobble events.
 */
const WorkHeaderLine = memo(function WorkHeaderLine() {
  const { work, movement } = useClassicalContext();
  const { navigateToClassicalWork } = useNavigation();

  if (!work) {
    return null;
  }

  const composer = work.composerName ?? null;
  const handleClick = () => {
    navigateToClassicalWork(work.mbid, work.title);
  };

  return (
    <span
      className="flex items-center gap-1.5 text-[10px] text-th-text-muted truncate min-w-0"
      title={composer ? `${composer} — ${work.title}` : work.title}
    >
      {composer && (
        <>
          <span className="font-medium text-th-text-secondary truncate">
            {composer}
          </span>
          <span aria-hidden="true" className="text-th-text-faint">
            ·
          </span>
        </>
      )}
      <button
        type="button"
        onClick={handleClick}
        className="truncate hover:underline hover:text-th-text-primary transition-colors text-left"
      >
        {work.title}
      </button>
      {movement && (
        <>
          <span aria-hidden="true" className="text-th-text-faint">
            ·
          </span>
          <span
            className="tabular-nums text-th-text-secondary whitespace-nowrap"
            title={`${movement.title} (${movement.method})`}
          >
            {toRoman(movement.index)} / {toRoman(movement.total)}
          </span>
        </>
      )}
      {movement?.attaccaTo !== undefined && (
        <span
          className="text-th-accent font-semibold whitespace-nowrap"
          title={`Attacca to movement ${toRoman(movement.attaccaTo)}`}
        >
          attacca →
        </span>
      )}
    </span>
  );
});

/**
 * Render `n` (1..=99) as upper-case roman numerals. Out-of-range
 * inputs degrade gracefully to the decimal representation — we render
 * what we got, never throw.
 */
function toRoman(n: number): string {
  if (!Number.isFinite(n) || n < 1 || n > 99) {
    return String(n);
  }
  const table: ReadonlyArray<readonly [number, string]> = [
    [90, "XC"],
    [50, "L"],
    [40, "XL"],
    [10, "X"],
    [9, "IX"],
    [5, "V"],
    [4, "IV"],
    [1, "I"],
  ];
  let remaining = Math.floor(n);
  let out = "";
  for (const [val, sym] of table) {
    while (remaining >= val) {
      out += sym;
      remaining -= val;
    }
  }
  return out;
}

export default WorkHeaderLine;
