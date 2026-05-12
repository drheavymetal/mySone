import type { Recording } from "../../types/classical";

/**
 * Phase 4 (F4.2) — sort dropdown for the WorkPage recordings list.
 * Pure UI; the parent owns the state and passes it to
 * `applyRecordingSort` below.
 */

export type RecordingSortKey =
  | "popularity"
  | "yearDesc"
  | "yearAsc"
  | "qualityDesc"
  | "conductor";

export const SORT_OPTIONS: Array<{ key: RecordingSortKey; label: string }> = [
  { key: "popularity", label: "Popularity" },
  { key: "yearDesc", label: "Year (newest first)" },
  { key: "yearAsc", label: "Year (oldest first)" },
  { key: "qualityDesc", label: "Audio quality (best first)" },
  { key: "conductor", label: "Conductor A–Z" },
];

interface RecordingSortProps {
  value: RecordingSortKey;
  onChange: (next: RecordingSortKey) => void;
}

export default function RecordingSort({ value, onChange }: RecordingSortProps) {
  return (
    <label className="flex items-center gap-2 text-[12px] text-th-text-secondary">
      <span className="font-medium uppercase tracking-wider text-[10px]">
        Sort
      </span>
      <select
        value={value}
        onChange={(e) => {
          onChange(e.target.value as RecordingSortKey);
        }}
        className="rounded-full border border-th-border-subtle/60 bg-th-surface/50 px-3 py-1 text-[12px] text-th-text-primary hover:border-th-accent/40 focus:border-th-accent focus:outline-none"
      >
        {SORT_OPTIONS.map((opt) => (
          <option key={opt.key} value={opt.key}>
            {opt.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function conductorKey(rec: Recording): string {
  return (rec.conductor?.name ?? rec.artistCredits[0] ?? "").toLowerCase();
}

/**
 * Apply a sort key to a recordings array. Returns a new array, leaving
 * the input untouched. Stable for ties so adjacent equally-scored rows
 * keep their input order (typically MB popularity proxy).
 *
 * `popularity` is implemented as "input order" because that's what MB's
 * recording browse already orders by (release count proxy). Phase 5
 * may add a true `popularityScore` field — until then we treat the
 * input ordering as authoritative.
 */
export function applyRecordingSort(
  recordings: Recording[],
  key: RecordingSortKey,
): Recording[] {
  if (recordings.length <= 1 || key === "popularity") {
    return recordings;
  }

  const indexed = recordings.map((rec, idx) => ({ rec, idx }));
  const compare = (
    a: { rec: Recording; idx: number },
    b: { rec: Recording; idx: number },
  ): number => {
    switch (key) {
      case "yearDesc": {
        const aY = a.rec.recordingYear ?? -Infinity;
        const bY = b.rec.recordingYear ?? -Infinity;
        if (aY !== bY) {
          return bY - aY;
        }
        return a.idx - b.idx;
      }
      case "yearAsc": {
        const aY = a.rec.recordingYear ?? Infinity;
        const bY = b.rec.recordingYear ?? Infinity;
        if (aY !== bY) {
          return aY - bY;
        }
        return a.idx - b.idx;
      }
      case "qualityDesc": {
        if (a.rec.qualityScore !== b.rec.qualityScore) {
          return b.rec.qualityScore - a.rec.qualityScore;
        }
        return a.idx - b.idx;
      }
      case "conductor": {
        const cmp = conductorKey(a.rec).localeCompare(conductorKey(b.rec));
        if (cmp !== 0) {
          return cmp;
        }
        return a.idx - b.idx;
      }
      default: {
        return a.idx - b.idx;
      }
    }
  };

  indexed.sort(compare);
  return indexed.map((x) => x.rec);
}
