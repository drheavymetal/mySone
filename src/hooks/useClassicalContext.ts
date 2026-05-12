import { useEffect, useState } from "react";
import { useAtomValue } from "jotai";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { currentTrackAtom } from "../atoms/playback";
import {
  getClassicalWork,
  getCurrentClassicalWorkMbid,
  resolveClassicalMovement,
} from "../api/classical";
import type {
  ClassicalWorkResolvedPayload,
  MovementContext,
  Work,
} from "../types/classical";

/**
 * Phase 3 (F3.1 / F3.2): aggregate hook that hydrates the work-aware
 * pieces of the player bar:
 *
 *   - `workMbid`: parent Work MBID for the currently playing track
 *     (resolved via the `classical:work-resolved` event, with a
 *     single +5 s polled fallback for the cold-mount race).
 *   - `work`: the full Work entity, hydrated lazily once the MBID is
 *     known. Source of `composerName` and `workTitle` for the
 *     persistent header.
 *   - `movement`: which movement of `work` the current track maps to,
 *     with `attaccaTo` exposed for the optional "Attacca →" hint.
 *
 * All fields independently nullable. The consumer renders only what's
 * present — no skeletons, no flicker. State resets on every track id
 * change.
 *
 * This hook does NOT touch audio routing, volume, or any playback
 * state — it's a read-only consumer of the catalog + scrobble events.
 */
export interface ClassicalPlaybackContext {
  workMbid: string | null;
  work: Work | null;
  movement: MovementContext | null;
}

const FALLBACK_POLL_DELAY_MS = 5_000;

export function useClassicalContext(): ClassicalPlaybackContext {
  const currentTrack = useAtomValue(currentTrackAtom);
  const [workMbid, setWorkMbid] = useState<string | null>(null);
  const [work, setWork] = useState<Work | null>(null);
  const [movement, setMovement] = useState<MovementContext | null>(null);

  // Reset on track change so the chip never lingers on the wrong track.
  useEffect(() => {
    setWorkMbid(null);
    setWork(null);
    setMovement(null);
  }, [currentTrack?.id]);

  // Primary path: subscribe to scrobble manager's resolution event.
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
        console.debug("[classical-context] event listen failed:", err);
      });
    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [currentTrack?.id]);

  // Fallback path — single delayed poll, no busy waiting.
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
        console.debug("[classical-context] fallback poll failed:", err);
      }
    }, FALLBACK_POLL_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [currentTrack?.id, workMbid]);

  // Hydrate Work entity once we know the MBID.
  useEffect(() => {
    if (!workMbid) {
      return;
    }
    let cancelled = false;
    getClassicalWork(workMbid)
      .then((w) => {
        if (!cancelled) {
          setWork(w);
        }
      })
      .catch((err: unknown) => {
        console.debug("[classical-context] fetch work failed:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [workMbid]);

  // Resolve movement once both work + currentTrack are present.
  useEffect(() => {
    if (!work || !currentTrack) {
      setMovement(null);
      return;
    }
    if (work.movements.length === 0) {
      // Single-movement piece — no indicator.
      setMovement(null);
      return;
    }
    let cancelled = false;
    resolveClassicalMovement(
      work.mbid,
      currentTrack.title,
      currentTrack.trackNumber,
    )
      .then((ctx) => {
        if (!cancelled) {
          setMovement(ctx);
        }
      })
      .catch((err: unknown) => {
        console.debug("[classical-context] resolve movement failed:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [work, currentTrack?.id, currentTrack?.title, currentTrack?.trackNumber]);

  return { workMbid, work, movement };
}
