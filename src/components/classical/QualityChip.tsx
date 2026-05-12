import type { BestAvailableQuality, Recording } from "../../types/classical";

/**
 * Phase 4 (F4.1) — single source of truth for rendering a "quality
 * chip" in the Classical Hub. Used by `RecordingRow` (per-recording),
 * the WorkPage header banner ("Best available …"), and any future
 * surface (search results, library cards).
 *
 * Color palette (CLASSICAL_DESIGN.md §16.1):
 *   - Hi-Res Lossless    → th-accent (project highlight)
 *   - Lossless 16/44.1   → th-accent/70
 *   - Dolby Atmos        → purple
 *   - MQA                → amber (controversial, surfaced not promoted)
 *   - High (lossy)       → th-text-secondary subdued
 */

interface QualityChipProps {
  /** Tidal tier label as the backend exposes it. */
  tier: string;
  /** Optional refined rate. When provided we render "24/96" alongside the tier. */
  sampleRateHz?: number;
  bitDepth?: number;
  /** Optional Atmos badge — usually rendered alongside the tier. */
  hasAtmos?: boolean;
  /** Compact = drop the rate label, keep only the tier pill. */
  size?: "compact" | "default";
}

interface ChipStyle {
  label: string;
  cls: string;
}

function tierStyle(tier: string): ChipStyle | null {
  switch (tier) {
    case "HIRES_LOSSLESS": {
      return {
        label: "HI-RES",
        cls: "bg-th-accent text-black",
      };
    }
    case "LOSSLESS": {
      return {
        label: "LOSSLESS",
        cls: "bg-th-accent/70 text-black",
      };
    }
    case "MQA": {
      return {
        label: "MQA",
        cls: "bg-amber-500/80 text-black",
      };
    }
    case "HIGH": {
      return {
        label: "HIGH",
        cls: "bg-th-button-hover text-th-text-primary",
      };
    }
    default: {
      return null;
    }
  }
}

function formatRate(sampleRateHz?: number, bitDepth?: number): string {
  if (!sampleRateHz && !bitDepth) {
    return "";
  }
  const rateLabel = sampleRateHz
    ? sampleRateHz % 1000 === 0
      ? `${sampleRateHz / 1000}`
      : (sampleRateHz / 1000).toFixed(1)
    : "";
  if (bitDepth && rateLabel) {
    return `${bitDepth}/${rateLabel}`;
  }
  if (rateLabel) {
    return `${rateLabel}k`;
  }
  if (bitDepth) {
    return `${bitDepth}-bit`;
  }
  return "";
}

/**
 * Render a quality chip from raw fields. Returns `null` for unknown
 * tiers (so an empty `audioQualityTags` recording renders as nothing —
 * the caller decides whether to surface a "not on Tidal" placeholder).
 */
export default function QualityChip({
  tier,
  sampleRateHz,
  bitDepth,
  hasAtmos,
  size = "default",
}: QualityChipProps) {
  const style = tierStyle(tier);
  if (!style && !hasAtmos) {
    return null;
  }

  const rate = size === "default" ? formatRate(sampleRateHz, bitDepth) : "";

  return (
    <div className="flex items-center gap-1">
      {rate && (
        <span className="font-mono text-[10px] tracking-tight text-th-text-secondary">
          {rate}
        </span>
      )}
      {style && (
        <span
          className={`rounded px-1.5 py-0.5 text-[9px] font-black uppercase tracking-wider leading-none ${style.cls}`}
        >
          {style.label}
        </span>
      )}
      {hasAtmos && (
        <span
          className="rounded px-1.5 py-0.5 text-[9px] font-black uppercase tracking-wider leading-none bg-purple-500/80 text-white"
          title="Dolby Atmos available"
        >
          ATMOS
        </span>
      )}
    </div>
  );
}

/**
 * Convenience: pick the right tier for a recording. Returns the
 * highest-priority tier present in `audioQualityTags`. Mirrors the
 * backend ordering in `quality.rs::primary_tier`.
 */
export function primaryTierOf(rec: Recording): string {
  const tags = rec.audioQualityTags;
  if (tags.includes("HIRES_LOSSLESS")) {
    return "HIRES_LOSSLESS";
  }
  if (tags.includes("LOSSLESS")) {
    return "LOSSLESS";
  }
  if (tags.includes("MQA")) {
    return "MQA";
  }
  if (tags.includes("HIGH")) {
    return "HIGH";
  }
  return "";
}

/** Whether a recording has the DOLBY_ATMOS audio mode. */
export function hasAtmosMode(rec: Recording): boolean {
  return rec.audioModes.includes("DOLBY_ATMOS");
}

/**
 * Render the chip directly from a `BestAvailableQuality` shape. Used
 * by the WorkPage header banner.
 */
export function BestAvailableChip({
  best,
  size = "default",
}: {
  best: BestAvailableQuality;
  size?: "compact" | "default";
}) {
  return (
    <QualityChip
      tier={best.tier}
      sampleRateHz={best.sampleRateHz}
      bitDepth={best.bitDepth}
      hasAtmos={best.hasAtmos}
      size={size}
    />
  );
}
