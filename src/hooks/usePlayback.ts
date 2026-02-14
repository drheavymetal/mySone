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
import type { Track, StreamInfo } from "../types";

export function usePlayback() {
  const [isPlaying, setIsPlaying] = useAtom(isPlayingAtom);
  const [currentTrack, setCurrentTrack] = useAtom(currentTrackAtom);
  const [volume, setVolumeState] = useAtom(volumeAtom);
  const [queue, setQueue] = useAtom(queueAtom);
  const [history, setHistory] = useAtom(historyAtom);
  const [streamInfo, setStreamInfo] = useAtom(streamInfoAtom);

  const currentTrackRef = useRef<Track | null>(null);

  // Keep ref in sync so callbacks always see latest value
  useEffect(() => {
    currentTrackRef.current = currentTrack;
  }, [currentTrack]);

  // NOTE: All global/init effects (restore, volume sync, persistence,
  // auto-play, keyboard shortcuts) have been moved to AppInitializer
  // to avoid running once per component that calls usePlayback().

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
