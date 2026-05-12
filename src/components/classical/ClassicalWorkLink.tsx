import { useEffect, useState } from "react";
import { useAtomValue } from "jotai";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { currentTrackAtom } from "../../atoms/playback";
import {
  getCurrentClassicalWorkMbid,
  getClassicalWork,
} from "../../api/classical";
import type { ClassicalWorkResolvedPayload } from "../../types/classical";
import { useNavigation } from "../../hooks/useNavigation";

/**
 * Phase 3 (F3.0): we prefer the `classical:work-resolved` Tauri event
 * (push) over polling. We still fall back to a single delayed poll at
 * +5 s to handle the corner case where the event was emitted before
 * this component finished mounting (race on first launch / quick
 * track changes). After that one fallback we go quiet — no busy
 * waiting.
 */
const FALLBACK_POLL_DELAY_MS = 5_000;

/**
 * "View work" affordance for the player bar. Two paths to discover the
 * parent-Work MBID for the currently playing track:
 *
 *   1. **Event subscription** (primary) — `classical:work-resolved` is
 *      emitted by `ScrobbleManager::on_track_started` once the
 *      MusicBrainz lookup chain lands. This is the fast path: one
 *      event, instant render.
 *   2. **Single delayed poll** (fallback) — at +5 s after the track
 *      starts we ask the backend snapshot once. Catches the race where
 *      the event fires before our listener is wired up (rare, but
 *      possible on app cold-start with autoplay).
 *
 * State resets on every track change. Hidden until both `workMbid` AND
 * `workTitle` land — we never show a hollow button.
 */
export default function ClassicalWorkLink() {
  const currentTrack = useAtomValue(currentTrackAtom);
  const { navigateToClassicalWork } = useNavigation();
  const [workMbid, setWorkMbid] = useState<string | null>(null);
  const [workTitle, setWorkTitle] = useState<string | null>(null);

  // Reset on track change.
  useEffect(() => {
    setWorkMbid(null);
    setWorkTitle(null);
  }, [currentTrack?.id]);

  // Primary path: event subscription. Lives for the lifetime of the
  // component (the player bar is mounted continuously); we filter
  // payloads by trackId so cross-track stale events don't bleed in.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;
    listen<ClassicalWorkResolvedPayload>(
      "classical:work-resolved",
      (event) => {
        if (cancelled) {
          return;
        }
        const payload = event.payload;
        // Track-id guard: when the backend knows the trackId at lookup
        // time it sends it. If our currentTrack changed between event
        // emit and delivery, drop the stale payload.
        if (
          payload.trackId !== undefined &&
          payload.trackId !== null &&
          currentTrack?.id !== undefined &&
          payload.trackId !== currentTrack.id
        ) {
          return;
        }
        setWorkMbid(payload.workMbid);
      },
    )
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((err) => {
        console.debug("[classical] event listen failed:", err);
      });
    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [currentTrack?.id]);

  // Fallback path: a single delayed poll after the track has had time
  // to land. Cancels if the event-driven path or another track win
  // first.
  useEffect(() => {
    if (!currentTrack) {
      return;
    }
    if (workMbid) {
      return;
    }
    let cancelled = false;
    const timer = setTimeout(async () => {
      if (cancelled) {
        return;
      }
      try {
        const mbid = await getCurrentClassicalWorkMbid();
        if (!cancelled && mbid) {
          setWorkMbid(mbid);
        }
      } catch (err) {
        console.debug("[classical] fallback poll failed:", err);
      }
    }, FALLBACK_POLL_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [currentTrack?.id, workMbid]);

  // Once we have the MBID, hydrate the title for a meaningful label.
  useEffect(() => {
    if (!workMbid) {
      return;
    }
    let cancelled = false;
    getClassicalWork(workMbid)
      .then((work) => {
        if (!cancelled) {
          setWorkTitle(work.title);
        }
      })
      .catch((err: unknown) => {
        console.debug("[classical] fetch work for label failed:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [workMbid]);

  if (!workMbid) {
    return null;
  }

  return (
    <button
      type="button"
      onClick={() => navigateToClassicalWork(workMbid, workTitle ?? undefined)}
      title={workTitle ? `View work: ${workTitle}` : "View work"}
      aria-label={
        workTitle ? `View classical work: ${workTitle}` : "View classical work"
      }
      className="ml-2 inline-flex items-center gap-1 rounded-md border border-th-accent/40 bg-th-accent/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-th-accent transition-colors hover:bg-th-accent/25"
    >
      <span aria-hidden="true">♪</span>
      View work
    </button>
  );
}
