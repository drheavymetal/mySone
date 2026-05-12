import type { Recording } from "../../types/classical";
import { hasAtmosMode, primaryTierOf } from "./QualityChip";

/**
 * Phase 4 (F4.2) — filter chips that narrow the recordings list. Pure
 * UI; the parent (`WorkPage`) owns the state and applies filtering via
 * `applyRecordingFilters` below.
 */

export interface RecordingFilterState {
  hiResOnly: boolean;
  atmosOnly: boolean;
  minSampleRateK: 0 | 96 | 192;
  excludeMqa: boolean;
  /** When > 0, only show recordings with `recordingYear >= minYear`. */
  minYear: number;
}

export const DEFAULT_FILTERS: RecordingFilterState = {
  hiResOnly: false,
  atmosOnly: false,
  minSampleRateK: 0,
  excludeMqa: false,
  minYear: 0,
};

interface RecordingFiltersProps {
  state: RecordingFilterState;
  onChange: (next: RecordingFilterState) => void;
  /** Earliest year present across the recordings — drives the year slider. */
  earliestYear: number;
}

interface ChipProps {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function Chip({ active, onClick, children }: ChipProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full border px-3 py-1 text-[12px] font-medium transition-colors ${
        active
          ? "border-th-accent bg-th-accent/15 text-th-text-primary"
          : "border-th-border-subtle/60 text-th-text-secondary hover:border-th-accent/40 hover:text-th-text-primary"
      }`}
    >
      {children}
    </button>
  );
}

export default function RecordingFilters({
  state,
  onChange,
  earliestYear,
}: RecordingFiltersProps) {
  const setField = <K extends keyof RecordingFilterState>(
    key: K,
    value: RecordingFilterState[K],
  ) => {
    onChange({ ...state, [key]: value });
  };

  const cycleSampleRate = () => {
    const next: RecordingFilterState["minSampleRateK"] =
      state.minSampleRateK === 0 ? 96 : state.minSampleRateK === 96 ? 192 : 0;
    setField("minSampleRateK", next);
  };

  const sampleRateLabel =
    state.minSampleRateK === 0
      ? "Sample rate"
      : `≥ ${state.minSampleRateK} kHz`;

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Chip
        active={state.hiResOnly}
        onClick={() => setField("hiResOnly", !state.hiResOnly)}
      >
        Hi-Res only
      </Chip>
      <Chip
        active={state.atmosOnly}
        onClick={() => setField("atmosOnly", !state.atmosOnly)}
      >
        Atmos
      </Chip>
      <Chip active={state.minSampleRateK > 0} onClick={cycleSampleRate}>
        {sampleRateLabel}
      </Chip>
      <Chip
        active={state.excludeMqa}
        onClick={() => setField("excludeMqa", !state.excludeMqa)}
      >
        Sin MQA
      </Chip>
      {earliestYear > 0 && (
        <YearChip
          minYear={state.minYear}
          earliestYear={earliestYear}
          onChange={(y) => setField("minYear", y)}
        />
      )}
    </div>
  );
}

interface YearChipProps {
  minYear: number;
  earliestYear: number;
  onChange: (y: number) => void;
}

function YearChip({ minYear, earliestYear, onChange }: YearChipProps) {
  const presets: Array<{ year: number; label: string }> = [
    { year: 0, label: "Year" },
    { year: 1990, label: "≥ 1990" },
    { year: 2000, label: "≥ 2000" },
    { year: 2010, label: "≥ 2010" },
    { year: 2020, label: "≥ 2020" },
  ].filter((p) => p.year === 0 || p.year >= earliestYear);

  const cycle = () => {
    const idx = presets.findIndex((p) => p.year === minYear);
    const next = presets[(idx + 1) % presets.length];
    onChange(next.year);
  };
  const current = presets.find((p) => p.year === minYear) ?? presets[0];
  return (
    <Chip active={minYear > 0} onClick={cycle}>
      {current.label}
    </Chip>
  );
}

/**
 * Apply a filter state to the recordings list. Pure: returns a new
 * array, does not mutate input. Order is preserved — the caller layers
 * `applyRecordingSort` on top of this.
 */
export function applyRecordingFilters(
  recordings: Recording[],
  filters: RecordingFilterState,
): Recording[] {
  return recordings.filter((rec) => {
    if (filters.hiResOnly) {
      const tier = primaryTierOf(rec);
      if (tier !== "HIRES_LOSSLESS") {
        return false;
      }
    }
    if (filters.atmosOnly && !hasAtmosMode(rec)) {
      return false;
    }
    if (filters.minSampleRateK > 0) {
      const sr = rec.sampleRateHz ?? 0;
      if (sr < filters.minSampleRateK * 1000) {
        return false;
      }
    }
    if (filters.excludeMqa && rec.audioQualityTags.includes("MQA")) {
      return false;
    }
    if (filters.minYear > 0) {
      const y = rec.recordingYear ?? 0;
      if (y < filters.minYear) {
        return false;
      }
    }
    return true;
  });
}
