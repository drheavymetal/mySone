import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAtomValue } from "jotai";
import {
  currentTrackAtom,
  isPlayingAtom,
  queueAtom,
  manualQueueAtom,
} from "../atoms/playback";
import { getTidalImageUrl, getTrackDisplayTitle, type Track } from "../types";
import { usePlaybackActions } from "./usePlaybackActions";

interface ShareCmdEvent {
  action: "play" | "pause" | "toggle" | "next" | "prev" | "playTrack";
  trackId?: number | null;
}

/**
 * Two-way sync between SONE state and the Listening Share landing.
 *
 * Push direction: every track / queue / play-state change is pushed to the
 * backend (debounced) so the public landing page can render the same
 * "now playing + up next" view.
 *
 * Receive direction: the backend forwards listener-issued transport
 * commands as `share-cmd` Tauri events. We dispatch them through the
 * existing `usePlaybackActions` hook so behaviour matches the local UI
 * exactly (queue advancement, autoplay rules, scrobble lifecycle, etc.).
 */
export function useShareSync() {
  const track = useAtomValue(currentTrackAtom);
  const isPlaying = useAtomValue(isPlayingAtom);
  const queue = useAtomValue(queueAtom);
  const manualQueue = useAtomValue(manualQueueAtom);
  const actions = usePlaybackActions();
  const debounceRef = useRef<number | undefined>(undefined);

  // ─── Push state ─────────────────────────────────────────────────────
  useEffect(() => {
    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => {
      const now = track
        ? {
            title: getTrackDisplayTitle(track),
            artist:
              track.artists?.map((a) => a.name).join(", ") ||
              track.artist?.name ||
              "",
            album: track.album?.title ?? "",
            coverUrl: track.album?.cover
              ? getTidalImageUrl(track.album.cover, 640)
              : "",
            durationSecs: track.duration ?? 0,
            positionSecs: 0,
          }
        : null;

      const upcoming: Track[] = [...manualQueue, ...queue].slice(0, 30);
      const queuePayload = upcoming.map((t) => ({
        trackId: t.id,
        title: getTrackDisplayTitle(t),
        artist:
          t.artists?.map((a) => a.name).join(", ") || t.artist?.name || "",
        coverUrl: t.album?.cover ? getTidalImageUrl(t.album.cover, 160) : "",
      }));

      invoke("share_set_state", {
        nowState: {
          now,
          queue: queuePayload,
          isPlaying,
        },
      }).catch(() => {});
    }, 250);

    return () => {
      if (debounceRef.current) window.clearTimeout(debounceRef.current);
    };
  }, [track, isPlaying, queue, manualQueue]);

  // ─── Listen for remote commands ─────────────────────────────────────
  useEffect(() => {
    let unsub: (() => void) | null = null;
    listen<ShareCmdEvent>("share-cmd", async (event) => {
      const p = event.payload;
      try {
        switch (p.action) {
          case "play":
            await actions.resumeTrack();
            break;
          case "pause":
            await actions.pauseTrack();
            break;
          case "toggle":
            if (isPlaying) await actions.pauseTrack();
            else await actions.resumeTrack();
            break;
          case "next":
            await actions.playNext();
            break;
          case "prev":
            await actions.playPrevious();
            break;
          case "playTrack": {
            if (!p.trackId) return;
            const all: Track[] = [...manualQueue, ...queue];
            const target = all.find((t) => t.id === p.trackId);
            if (target) await actions.playTrack(target, { chosenByUser: true });
            break;
          }
        }
      } catch (e) {
        console.warn("[share-cmd] failed:", e);
      }
    }).then((u) => {
      unsub = u;
    });
    return () => {
      if (unsub) unsub();
    };
  }, [actions, isPlaying, queue, manualQueue]);
}
