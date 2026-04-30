import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAtomValue } from "jotai";
import { currentTrackAtom } from "../atoms/playback";
import { parseLyrics, type ParsedLyrics } from "../lib/lyrics";

interface TidalLyricsRaw {
  trackId?: number;
  lyricsProvider?: string;
  lyrics?: string;
  subtitles?: string;
  isRightToLeft?: boolean;
}

interface State {
  loading: boolean;
  parsed: ParsedLyrics | null;
  error: string | null;
  rtl: boolean;
}

const empty: State = { loading: false, parsed: null, error: null, rtl: false };

/**
 * Fetches lyrics from TIDAL whenever the current track changes and parses
 * them into synced + plain forms. Cancels in-flight requests when track
 * flips so a slow fetch can't clobber a newer track's lyrics.
 */
export function useLyrics(): State {
  const track = useAtomValue(currentTrackAtom);
  const [state, setState] = useState<State>(empty);

  useEffect(() => {
    if (!track) {
      setState(empty);
      return;
    }
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, error: null }));
    invoke<TidalLyricsRaw>("get_track_lyrics", { trackId: track.id })
      .then((raw) => {
        if (cancelled) return;
        const parsed = parseLyrics({
          subtitles: raw.subtitles,
          lyrics: raw.lyrics,
        });
        setState({
          loading: false,
          parsed,
          error: null,
          rtl: !!raw.isRightToLeft,
        });
      })
      .catch((e) => {
        if (cancelled) return;
        setState({
          loading: false,
          parsed: null,
          error: typeof e === "string" ? e : (e?.message ?? "no lyrics"),
          rtl: false,
        });
      });
    return () => {
      cancelled = true;
    };
  }, [track?.id]);

  return state;
}
