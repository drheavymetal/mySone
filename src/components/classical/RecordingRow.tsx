import { useState, useCallback } from "react";
import { Star } from "lucide-react";

import { getTidalImageUrl, type Track } from "../../types";
import { getTrack } from "../../api/tidal";
import { usePlaybackActions } from "../../hooks/usePlaybackActions";
import { useNavigation } from "../../hooks/useNavigation";
import { useToast } from "../../contexts/ToastContext";
import {
  setClassicalEditorsChoice,
  clearClassicalEditorsChoice,
} from "../../api/classical";
import type { PerformerCredit, Recording } from "../../types/classical";

import ConfidenceBadge from "./ConfidenceBadge";
import QualityChip, { hasAtmosMode, primaryTierOf } from "./QualityChip";

interface RecordingRowProps {
  recording: Recording;
  workTitle: string;
  /** Phase 5 — fired when the user toggles Editor's Choice override. */
  onEditorialChange?: () => void;
}

function primaryArtists(recording: Recording): string {
  if (recording.conductor) {
    const orchestra = recording.orchestras[0]?.name;
    return orchestra
      ? `${recording.conductor.name} · ${orchestra}`
      : recording.conductor.name;
  }
  if (recording.orchestras.length > 0) {
    return recording.orchestras.map((o) => o.name).join(" · ");
  }
  if (recording.artistCredits.length > 0) {
    return recording.artistCredits.slice(0, 2).join(" · ");
  }
  return "Unknown performer";
}

function formatYear(recording: Recording): string {
  if (recording.recordingYear) {
    return String(recording.recordingYear);
  }
  return "";
}

function formatDuration(secs?: number): string {
  if (!secs || secs <= 0) {
    return "";
  }
  const m = Math.floor(secs / 60);
  const s = Math.round(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}


/**
 * Single recording row in the Work page list. Shows cover, primary
 * artist (conductor + orchestra), year, label, quality badges, and the
 * D-010 confidence indicator. Clicking the play button hydrates the
 * Tidal track via `getTrack(id)` and hands it to the existing
 * playback pipeline — audio routing untouched.
 */
export default function RecordingRow({
  recording,
  workTitle,
  onEditorialChange,
}: RecordingRowProps) {
  const { playTrack } = usePlaybackActions();
  const { navigateToClassicalArtist } = useNavigation();
  const { showToast } = useToast();
  const [busy, setBusy] = useState(false);
  const [updatingChoice, setUpdatingChoice] = useState(false);

  const playable =
    recording.matchConfidence !== "NotFound" &&
    recording.tidalTrackId !== undefined &&
    recording.tidalTrackId !== null;

  const handlePlay = useCallback(async () => {
    if (!playable || recording.tidalTrackId === undefined) {
      return;
    }
    setBusy(true);
    try {
      // Hydrate full Track from Tidal so the rest of the player has
      // every field it expects (mediaMetadata, audio modes, etc).
      const track: Track = await getTrack(recording.tidalTrackId);
      const ok = await playTrack(track, { chosenByUser: true });
      if (!ok) {
        showToast("Could not start playback", "error");
      }
    } catch (err) {
      console.error("[classical] play failed:", err);
      showToast("Could not start playback", "error");
    } finally {
      setBusy(false);
    }
  }, [playTrack, playable, recording.tidalTrackId, showToast]);

  const cover = recording.coverUrl
    ? getTidalImageUrl(recording.coverUrl, 160)
    : null;

  const ariaLabel = `Play ${workTitle} — ${primaryArtists(recording)}`;
  const year = formatYear(recording);
  const duration = formatDuration(recording.durationSecs);
  const isEditorsChoice = recording.isEditorsChoice ?? false;

  // Phase 5 (D-021): toggle the user override. If the row already is
  // the Editor's Choice we clear; otherwise we set this row as the pick.
  const handleToggleEditorsChoice = useCallback(async () => {
    if (updatingChoice || !recording.workMbid) {
      return;
    }
    setUpdatingChoice(true);
    try {
      if (isEditorsChoice) {
        await clearClassicalEditorsChoice(recording.workMbid);
        showToast("Editor's Choice override cleared", "info");
      } else {
        await setClassicalEditorsChoice(
          recording.workMbid,
          recording.mbid,
          undefined,
        );
        showToast("Marked as Editor's Choice", "info");
      }
      onEditorialChange?.();
    } catch (err: unknown) {
      console.error("[classical] editorial toggle failed:", err);
      showToast("Could not update Editor's Choice", "error");
    } finally {
      setUpdatingChoice(false);
    }
  }, [
    isEditorsChoice,
    onEditorialChange,
    recording.mbid,
    recording.workMbid,
    showToast,
    updatingChoice,
  ]);

  return (
    <li
      className={`group flex items-center gap-4 rounded-xl border bg-th-surface/40 px-4 py-3 transition-colors hover:border-th-accent/40 hover:bg-th-surface/70 ${
        isEditorsChoice
          ? "border-th-accent/50 bg-th-accent/5"
          : "border-th-border-subtle/60"
      } ${playable ? "" : "opacity-70"}`}
    >
      {/* Editor's Choice toggle */}
      <button
        type="button"
        onClick={handleToggleEditorsChoice}
        disabled={updatingChoice}
        className={`shrink-0 flex h-7 w-7 items-center justify-center rounded-full transition-colors ${
          isEditorsChoice
            ? "text-th-accent"
            : "text-th-text-faint hover:text-th-text-secondary"
        } disabled:opacity-50`}
        aria-label={
          isEditorsChoice
            ? "Clear Editor's Choice"
            : "Mark as Editor's Choice"
        }
        title={
          isEditorsChoice
            ? recording.editorNote ?? "Editor's Choice"
            : "Mark as Editor's Choice"
        }
      >
        <Star
          size={16}
          fill={isEditorsChoice ? "currentColor" : "none"}
        />
      </button>

      {/* Cover */}
      <div className="h-12 w-12 shrink-0 overflow-hidden rounded-md bg-th-base/80">
        {cover ? (
          <img
            src={cover}
            alt=""
            className="h-full w-full object-cover"
            loading="lazy"
          />
        ) : (
          <div className="h-full w-full bg-gradient-to-br from-th-surface to-th-base" />
        )}
      </div>

      {/* Body */}
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="truncate text-[14px] font-semibold text-th-text-primary">
            <ArtistLinks
              recording={recording}
              onArtistClick={(mbid, name) =>
                navigateToClassicalArtist(mbid, name)
              }
            />
          </span>
          {year && (
            <span className="shrink-0 font-mono text-[12px] text-th-text-secondary">
              {year}
            </span>
          )}
        </div>
        <div className="mt-1 flex items-center gap-2 text-[12px] text-th-text-secondary">
          {recording.label && (
            <span className="truncate">{recording.label}</span>
          )}
          {recording.label && (
            <span aria-hidden="true" className="text-th-text-secondary/40">
              ·
            </span>
          )}
          <ConfidenceBadge
            confidence={recording.matchConfidence}
            query={recording.matchQuery}
            score={recording.matchScore}
          />
          {isEditorsChoice && recording.editorNote && (
            <>
              <span aria-hidden="true" className="text-th-text-secondary/40">
                ·
              </span>
              <span className="truncate text-th-accent" title={recording.editorNote}>
                Editor's Choice
              </span>
            </>
          )}
        </div>
      </div>

      {/* Quality + duration */}
      <div className="hidden flex-col items-end gap-1.5 sm:flex">
        <QualityChip
          tier={primaryTierOf(recording)}
          sampleRateHz={recording.sampleRateHz}
          bitDepth={recording.bitDepth}
          hasAtmos={hasAtmosMode(recording)}
        />
        {duration && (
          <span className="font-mono text-[11px] text-th-text-secondary">
            {duration}
          </span>
        )}
      </div>

      {/* Play */}
      <button
        type="button"
        onClick={handlePlay}
        disabled={!playable || busy}
        aria-label={ariaLabel}
        className={`shrink-0 flex h-9 w-9 items-center justify-center rounded-full transition-colors ${
          playable
            ? "bg-th-accent text-black hover:scale-105 disabled:opacity-60"
            : "bg-th-base/60 text-th-text-secondary/50 cursor-not-allowed"
        }`}
      >
        {busy ? (
          <span
            aria-hidden="true"
            className="h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent"
          />
        ) : (
          <svg
            aria-hidden="true"
            viewBox="0 0 24 24"
            fill="currentColor"
            className="h-4 w-4 translate-x-[1px]"
          >
            <path d="M8 5v14l11-7L8 5z" />
          </svg>
        )}
      </button>
    </li>
  );
}

interface ArtistLinksProps {
  recording: Recording;
  onArtistClick: (mbid: string, name: string) => void;
}

/**
 * Phase 6 (D-022) — render the recording's primary credits as
 * navigable links when MusicBrainz gave us an artist MBID. Falls back
 * to the flat text rendering when MBIDs aren't present (typical for
 * Tidal-text-search-inferred recordings or non-classical credits).
 */
function ArtistLinks({ recording, onArtistClick }: ArtistLinksProps) {
  const credits: PerformerCredit[] = [];
  if (recording.conductor) {
    credits.push(recording.conductor);
  }
  if (recording.orchestras.length > 0) {
    credits.push(recording.orchestras[0]);
  }
  if (credits.length === 0) {
    // No structured credits — fall back to the flat artistCredits text.
    return <>{primaryArtists(recording)}</>;
  }
  return (
    <span>
      {credits.map((c, idx) => (
        <span key={`${c.name}:${idx}`}>
          {idx > 0 && (
            <span aria-hidden="true" className="mx-1 text-th-text-secondary/40">
              ·
            </span>
          )}
          {c.mbid ? (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onArtistClick(c.mbid!, c.name);
              }}
              className="rounded text-th-text-primary hover:text-th-accent transition-colors hover:underline"
              title={`Browse ${c.name}'s discography`}
            >
              {c.name}
            </button>
          ) : (
            <span>{c.name}</span>
          )}
        </span>
      ))}
    </span>
  );
}
