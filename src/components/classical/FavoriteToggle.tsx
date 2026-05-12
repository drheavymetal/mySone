import { useEffect, useState } from "react";
import { Heart } from "lucide-react";

import {
  addClassicalFavorite,
  isClassicalFavorite,
  removeClassicalFavorite,
} from "../../api/classical";
import type { ClassicalFavorite } from "../../types/classical";

interface FavoriteToggleProps {
  kind: ClassicalFavorite["kind"];
  mbid: string;
  displayName: string;
  /** Optional UI label next to the heart, e.g. "Save". When omitted
   *  the button renders as a circular icon-only control. */
  label?: string;
  className?: string;
}

/**
 * Phase 6 (F6.5) — toggle a classical entity's saved state. The
 * component owns its own state lookup (`isClassicalFavorite`) so any
 * page can drop it in without threading state from a parent.
 *
 * Pure UI: never touches audio, never opens a stream — the only
 * round-trips are stats DB CRUD calls.
 */
export default function FavoriteToggle({
  kind,
  mbid,
  displayName,
  label,
  className,
}: FavoriteToggleProps) {
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    isClassicalFavorite(kind, mbid)
      .then((v) => {
        if (!cancelled) {
          setSaved(v);
        }
      })
      .catch((err: unknown) => {
        console.error("[favorite-toggle] check failed:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [kind, mbid]);

  const onClick = async () => {
    if (busy || !mbid) {
      return;
    }
    setBusy(true);
    try {
      if (saved) {
        await removeClassicalFavorite(kind, mbid);
        setSaved(false);
      } else {
        await addClassicalFavorite(kind, mbid, displayName);
        setSaved(true);
      }
    } catch (err: unknown) {
      console.error("[favorite-toggle] toggle failed:", err);
    } finally {
      setBusy(false);
    }
  };

  const tooltip = saved ? "Remove from your library" : "Save to your library";
  const labelText = label ?? (saved ? "Saved" : "Save");
  const heartProps = saved
    ? { fill: "currentColor", className: "text-th-accent" }
    : { className: "text-th-text-secondary" };
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={busy}
      title={tooltip}
      aria-pressed={saved}
      aria-label={tooltip}
      className={
        className ??
        `inline-flex items-center gap-1.5 rounded-full border border-th-border-subtle/60 px-3 py-1 text-[12px] font-medium transition-colors hover:border-th-accent/40 hover:text-th-text-primary disabled:opacity-50 ${
          saved
            ? "bg-th-accent/15 text-th-accent"
            : "bg-th-surface/40 text-th-text-secondary"
        }`
      }
    >
      <Heart size={14} {...heartProps} />
      {label !== "" && <span>{labelText}</span>}
    </button>
  );
}
