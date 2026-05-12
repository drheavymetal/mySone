import type { Movement } from "../../types/classical";

interface MovementListProps {
  movements: Movement[];
}

function formatDuration(secs?: number): string {
  if (!secs || secs <= 0) {
    return "";
  }
  const m = Math.floor(secs / 60);
  const s = Math.round(secs % 60);
  if (s === 0) {
    return `${m} min`;
  }
  return `${m}m ${s}s`;
}

const ROMAN = [
  "I",
  "II",
  "III",
  "IV",
  "V",
  "VI",
  "VII",
  "VIII",
  "IX",
  "X",
  "XI",
  "XII",
];

function romanIndex(n: number): string {
  if (n >= 1 && n <= ROMAN.length) {
    return ROMAN[n - 1];
  }
  return String(n);
}

/**
 * Plain list of movements with roman index + duration. No interaction
 * yet; Phase 3 wires per-movement playback once the player is
 * work-aware. Shown only when MB has child-work data for the work.
 */
export default function MovementList({ movements }: MovementListProps) {
  if (movements.length === 0) {
    return null;
  }

  return (
    <section
      className="rounded-2xl border border-th-border-subtle bg-th-surface/40 p-5"
      aria-label="Movements"
    >
      <h2 className="mb-3 text-[13px] font-bold uppercase tracking-[0.18em] text-th-text-secondary">
        Movements
      </h2>
      <ol className="space-y-1">
        {movements.map((m, idx) => {
          const dur = formatDuration(m.durationApproxSecs);
          const display = idx + 1;
          return (
            <li
              key={m.mbid}
              className="flex items-baseline gap-3 py-1 text-[14px] text-th-text-primary/90"
            >
              <span className="w-10 shrink-0 font-mono text-[12px] font-semibold text-th-text-secondary">
                {romanIndex(display)}.
              </span>
              <span className="flex-1 truncate">{m.title}</span>
              {dur && (
                <span className="shrink-0 font-mono text-[12px] text-th-text-secondary/80">
                  {dur}
                </span>
              )}
            </li>
          );
        })}
      </ol>
    </section>
  );
}
