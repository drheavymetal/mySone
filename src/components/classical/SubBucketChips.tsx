import type { SubBucketSummary } from "../../types/classical";

/**
 * Phase 9 (F9.2 / D-040 / D-043) — sub-bucket filter chips. Shown
 * above a `BucketSection` grid when the parent bucket exposed a
 * sub-bucket palette server-side. Click toggles the active filter;
 * a second click on the active chip clears it.
 *
 * The "All" pseudo-chip is rendered first so users always have an
 * obvious way back. Counts come pre-computed from the backend.
 */

interface SubBucketChipsProps {
  subBuckets: SubBucketSummary[];
  active: string | null;
  onChange: (next: string | null) => void;
}

export default function SubBucketChips({
  subBuckets,
  active,
  onChange,
}: SubBucketChipsProps) {
  const total = subBuckets.reduce((sum, s) => sum + s.count, 0);
  return (
    <div
      className="mb-4 flex flex-wrap items-center gap-2"
      role="group"
      aria-label="Sub-bucket filters"
    >
      <Chip
        label="All"
        count={total}
        active={active === null}
        onClick={() => onChange(null)}
      />
      {subBuckets.map((s) => (
        <Chip
          key={s.label}
          label={s.label}
          count={s.count}
          active={active === s.label}
          onClick={() => onChange(active === s.label ? null : s.label)}
        />
      ))}
    </div>
  );
}

interface ChipProps {
  label: string;
  count: number;
  active: boolean;
  onClick: () => void;
}

function Chip({ label, count, active, onClick }: ChipProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full border px-3 py-1 text-[12px] font-medium transition-colors ${
        active
          ? "border-th-accent bg-th-accent/15 text-th-text-primary"
          : "border-th-border-subtle/60 bg-th-elevated text-th-text-secondary hover:border-th-accent/40 hover:text-th-text-primary"
      }`}
      aria-pressed={active}
    >
      {label}
      <span className="ml-1.5 text-[11px] tabular-nums opacity-70">
        ({count})
      </span>
    </button>
  );
}
