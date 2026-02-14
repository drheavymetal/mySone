import { useEffect, useCallback, useRef } from "react";
import { useAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import {
  isPlayingAtom,
  currentTrackAtom,
  volumeAtom,
  queueAtom,
  historyAtom,
  streamInfoAtom,
} from "../atoms/playback";
import type { Track, StreamInfo, PlaybackSnapshot } from "../types";

const PLAYBACK_STATE_KEY = "tide-vibe.playback-state.v1";

export function usePlayback() {
  const [isPlaying, setIsPlaying] = useAtom(isPlayingAtom);
  const [currentTrack, setCurrentTrack] = useAtom(currentTrackAtom);
  const [volume, setVolumeState] = useAtom(volumeAtom);
  const [queue, setQueue] = useAtom(queueAtom);
  const [history, setHistory] = useAtom(historyAtom);
  const [streamInfo, setStreamInfo] = useAtom(streamInfoAtom);

  const currentTrackRef = useRef<Track | null>(null);
  const hasRestoredPlaybackRef = useRef(false);
  const playbackPersistReady = useRef(false);
  // Track whether volume was synced to backend after restore.
  const volumeSyncedRef = useRef(false);

  // Keep ref in sync so callbacks always see latest value
  useEffect(() => {
    currentTrackRef.current = currentTrack;
  }, [currentTrack]);

  // Restore last playback session (track + queue + history)
  // Volume is handled by atomWithStorage automatically.
  useEffect(() => {
    try {
      const raw = localStorage.getItem(PLAYBACK_STATE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<PlaybackSnapshot>;

        if (parsed.currentTrack && typeof parsed.currentTrack.id === "number") {
          setCurrentTrack(parsed.currentTrack as Track);
        }

        if (Array.isArray(parsed.queue)) {
          setQueue(
            parsed.queue.filter(
              (track): track is Track => !!track && typeof track.id === "number"
            )
          );
        }

        if (Array.isArray(parsed.history)) {
          setHistory(
            parsed.history.filter(
              (track): track is Track => !!track && typeof track.id === "number"
            )
          );
        }
      }
    } catch (err) {
      console.error("Failed to restore playback state:", err);
    } finally {
      hasRestoredPlaybackRef.current = true;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Sync restored volume to backend once on mount.
  // `volumeAtom` is restored from localStorage by atomWithStorage synchronously,
  // but we still need to push it to the Tauri backend.
  useEffect(() => {
    if (!volumeSyncedRef.current) {
      volumeSyncedRef.current = true;
      invoke("set_volume", { level: volume }).catch((err) => {
        console.error("Failed to apply restored volume:", err);
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Persist now-playing state across app relaunches.
  useEffect(() => {
    if (!hasRestoredPlaybackRef.current) return;

    if (!playbackPersistReady.current) {
      playbackPersistReady.current = true;
      return;
    }

    const snapshot: PlaybackSnapshot = { currentTrack, queue, history };

    try {
      localStorage.setItem(PLAYBACK_STATE_KEY, JSON.stringify(snapshot));
    } catch (err) {
      console.error("Failed to persist playback state:", err);
    }
  }, [currentTrack, queue, history]);

  // Auto-play next track when current finishes
  useEffect(() => {
    if (!isPlaying || !currentTrack) return;

    const checkInterval = setInterval(async () => {
      try {
        const isFinished = await invoke<boolean>("is_track_finished");
        if (isFinished && queue.length > 0) {
          playNext();
        }
      } catch (err) {
        console.error("Failed to check track status:", err);
      }
    }, 1000);

    return () => clearInterval(checkInterval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isPlaying, currentTrack, queue]);

  const playTrack = useCallback(
    async (track: Track) => {
      try {
        // Push current track to history before switching
        if (currentTrackRef.current) {
          setHistory((h) => [...h, currentTrackRef.current!]);
        }
        const info = await invoke<StreamInfo>("play_tidal_track", {
          trackId: track.id,
        });
        setStreamInfo(info);
        setCurrentTrack(track);
        setIsPlaying(true);
      } catch (error: any) {
        console.error("Failed to play track:", error);
      }
    },
    [setHistory, setStreamInfo, setCurrentTrack, setIsPlaying]
  );

  const pauseTrack = useCallback(async () => {
    try {
      await invoke("pause_track");
      setIsPlaying(false);
    } catch (error) {
      console.error("Failed to pause track:", error);
    }
  }, [setIsPlaying]);

  const resumeTrack = useCallback(async () => {
    try {
      const track = currentTrackRef.current;
      if (!track) return;

      const isFinished = await invoke<boolean>("is_track_finished");
      if (isFinished) {
        const info = await invoke<StreamInfo>("play_tidal_track", {
          trackId: track.id,
        });
        setStreamInfo(info);
      } else {
        await invoke("resume_track");
      }
      setIsPlaying(true);
    } catch (error) {
      console.error("Failed to resume track:", error);
    }
  }, [setStreamInfo, setIsPlaying]);

  const setVolume = useCallback(
    async (level: number) => {
      setVolumeState(level);
      try {
        await invoke("set_volume", { level });
      } catch (error) {
        console.error("Failed to set volume:", error);
      }
    },
    [setVolumeState]
  );

  const getPlaybackPosition = useCallback(async (): Promise<number> => {
    try {
      return await invoke<number>("get_playback_position");
    } catch (error) {
      console.error("Failed to get playback position:", error);
      return 0;
    }
  }, []);

  const seekTo = useCallback(async (positionSecs: number) => {
    try {
      await invoke("seek_track", { positionSecs });
    } catch (error) {
      console.error("Failed to seek:", error);
    }
  }, []);

  const addToQueue = useCallback(
    (track: Track) => {
      setQueue((prev) => [...prev, track]);
    },
    [setQueue]
  );

  const playNextInQueue = useCallback(
    (track: Track) => {
      setQueue((prev) => [track, ...prev]);
    },
    [setQueue]
  );

  const setQueueTracks = useCallback(
    (tracks: Track[]) => {
      setQueue(tracks);
    },
    [setQueue]
  );

  const removeFromQueue = useCallback(
    (index: number) => {
      setQueue((prev) => prev.filter((_, i) => i !== index));
    },
    [setQueue]
  );

  const playNext = useCallback(async () => {
    if (queue.length > 0) {
      const [nextTrack, ...rest] = queue;
      setQueue(rest);
      await playTrack(nextTrack);
    } else {
      setIsPlaying(false);
    }
  }, [queue, setQueue, playTrack, setIsPlaying]);

  const playPrevious = useCallback(async () => {
    // If more than 3 seconds in, restart the current track
    try {
      const pos = await getPlaybackPosition();
      if (pos > 3) {
        await seekTo(0);
        return;
      }
    } catch {
      // ignore position errors
    }

    // Go to previous track from history
    if (history.length > 0) {
      const newHistory = [...history];
      const prevTrack = newHistory.pop()!;
      setHistory(newHistory);

      // Put current track back at front of queue
      if (currentTrackRef.current) {
        const curr = currentTrackRef.current;
        setQueue((prev) => [curr, ...prev]);
      }

      // Play previous track directly (playTrack would push to history again)
      try {
        const info = await invoke<StreamInfo>("play_tidal_track", {
          trackId: prevTrack.id,
        });
        setStreamInfo(info);
        setCurrentTrack(prevTrack);
        setIsPlaying(true);
      } catch (error: any) {
        console.error("Failed to play previous track:", error);
      }
    } else if (currentTrackRef.current) {
      // No history, just restart current track
      await seekTo(0);
    }
  }, [history, setHistory, setQueue, setStreamInfo, setCurrentTrack, setIsPlaying, getPlaybackPosition, seekTo]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        return;
      }

      switch (e.code) {
        case "Space":
          e.preventDefault();
          if (isPlaying) {
            pauseTrack();
          } else {
            resumeTrack();
          }
          break;
        case "ArrowLeft":
          e.preventDefault();
          playPrevious();
          break;
        case "ArrowRight":
          e.preventDefault();
          playNext();
          break;
        case "ArrowUp":
          e.preventDefault();
          setVolume(Math.min(1.0, volume + 0.1));
          break;
        case "ArrowDown":
          e.preventDefault();
          setVolume(Math.max(0.0, volume - 0.1));
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isPlaying, volume, playNext, playPrevious, pauseTrack, resumeTrack, setVolume]);

  return {
    isPlaying,
    currentTrack,
    volume,
    queue,
    history,
    streamInfo,
    playTrack,
    pauseTrack,
    resumeTrack,
    setVolume,
    seekTo,
    getPlaybackPosition,
    addToQueue,
    playNextInQueue,
    setQueueTracks,
    removeFromQueue,
    playNext,
    playPrevious,
  };
}
