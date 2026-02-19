import { Play, Pause, Shuffle, ChevronLeft } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useAtomValue } from "jotai";
import { isPlayingAtom, currentTrackAtom } from "../atoms/playback";
import { usePlaybackActions } from "../hooks/usePlaybackActions";
import { useNavigation } from "../hooks/useNavigation";
import { getArtistTopTracksAll } from "../api/tidal";
import { getTidalImageUrl } from "../types";
import TidalImage from "./TidalImage";
import TrackContextMenu from "./TrackContextMenu";

interface ArtistTracksPageProps {
  artistId: number;
  artistName: string;
  onBack: () => void;
}

function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

export default function ArtistTracksPage({
  artistId,
  artistName,
  onBack,
}: ArtistTracksPageProps) {
  const isPlaying = useAtomValue(isPlayingAtom);
  const currentTrack = useAtomValue(currentTrackAtom);
  const { playTrack, setQueueTracks, pauseTrack, resumeTrack } =
    usePlaybackActions();
  const { navigateToAlbum } = useNavigation();

  const [tracks, setTracks] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [contextMenu, setContextMenu] = useState<{
    track: any;
    index: number;
    position: { x: number; y: number };
  } | null>(null);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await getArtistTopTracksAll(artistId);
        if (!cancelled) setTracks(data);
      } catch (err: any) {
        if (!cancelled) {
          console.error("[ArtistTracksPage] load error:", err);
          const parsed = typeof err === "string" ? (() => { try { return JSON.parse(err); } catch { return null; } })() : err;
          const msg = parsed?.message;
          if (typeof msg === "string") {
            setError(msg);
          } else if (msg && typeof msg === "object") {
            setError(`API ${msg.status}: ${typeof msg.body === "string" ? msg.body.slice(0, 200) : JSON.stringify(msg.body).slice(0, 200)}`);
          } else {
            setError(typeof err === "string" ? err : "Failed to load tracks");
          }
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    load();
    return () => { cancelled = true; };
  }, [artistId]);

  const trackIds = useMemo(
    () => new Set(tracks.map((t: any) => t.id).filter(Boolean)),
    [tracks]
  );

  const handlePlayTrack = async (track: any, index: number) => {
    try {
      setQueueTracks(tracks.slice(index + 1));
      await playTrack(track);
    } catch (err) {
      console.error("Failed to play track:", err);
    }
  };

  const handlePlayAll = async () => {
    if (tracks.length === 0) return;
    if (currentTrack && trackIds.has(currentTrack.id)) {
      if (isPlaying) await pauseTrack();
      else await resumeTrack();
      return;
    }
    try {
      setQueueTracks(tracks.slice(1));
      await playTrack(tracks[0]);
    } catch (err) {
      console.error("Failed to play all:", err);
    }
  };

  const handleShuffle = async () => {
    if (tracks.length === 0) return;
    const shuffled = [...tracks];
    for (let i = shuffled.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
    }
    try {
      setQueueTracks(shuffled.slice(1));
      await playTrack(shuffled[0]);
    } catch (err) {
      console.error("Failed to shuffle:", err);
    }
  };

  const allPlaying = !!(
    currentTrack && trackIds.has(currentTrack.id) && isPlaying
  );

  if (loading) {
    return (
      <div className="flex-1 bg-linear-to-b from-th-surface to-th-base overflow-y-auto">
        <div className="px-8 pt-6 pb-4">
          <button
            onClick={onBack}
            className="flex items-center gap-1 text-th-text-muted hover:text-white transition-colors text-sm font-medium mb-4"
          >
            <ChevronLeft size={18} />
            Back
          </button>
          <div className="h-8 w-48 bg-th-surface-hover rounded animate-pulse mb-6" />
        </div>
        <div className="px-8 flex flex-col gap-1">
          {Array.from({ length: 10 }).map((_, i) => (
            <div key={i} className="h-14 bg-th-surface-hover/50 rounded animate-pulse" />
          ))}
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex-1 bg-linear-to-b from-th-surface to-th-base flex items-center justify-center">
        <div className="flex flex-col items-center gap-4 text-center px-8">
          <p className="text-white font-semibold text-lg">
            Couldn't load tracks
          </p>
          <p className="text-th-text-muted text-sm max-w-md">{error}</p>
          <button
            onClick={onBack}
            className="mt-2 px-6 py-2 bg-white text-black rounded-full text-sm font-bold hover:scale-105 transition-transform"
          >
            Go back
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 bg-linear-to-b from-th-surface to-th-base overflow-y-auto scrollbar-thin scrollbar-thumb-th-button scrollbar-track-transparent">
      <div className="px-8 pt-6 pb-4">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-th-text-muted hover:text-white transition-colors text-sm font-medium mb-4"
        >
          <ChevronLeft size={18} />
          Back
        </button>

        <h1 className="text-[32px] font-extrabold text-white leading-tight mb-1">
          Popular tracks
        </h1>
        <p className="text-th-text-muted text-sm">{artistName}</p>
      </div>

      <div className="px-8 py-4 flex items-center gap-3">
        <button
          onClick={handlePlayAll}
          className="flex items-center gap-2 px-6 py-2.5 bg-th-accent text-black font-bold text-sm rounded-full shadow-lg hover:brightness-110 hover:scale-[1.03] transition-[transform,filter] duration-150"
        >
          {allPlaying ? (
            <Pause size={18} fill="black" className="text-black" />
          ) : (
            <Play size={18} fill="black" className="text-black" />
          )}
          {allPlaying ? "Pause" : "Play"}
        </button>
        <button
          onClick={handleShuffle}
          className="flex items-center gap-2 px-6 py-2.5 bg-th-button text-white font-bold text-sm rounded-full hover:bg-th-button-hover hover:scale-[1.03] transition-[transform,filter,background-color] duration-150"
        >
          <Shuffle size={18} />
          Shuffle
        </button>
      </div>

      <div className="px-8 pb-8">
        <div className="flex flex-col">
          {tracks.map((track, index) => {
            const trackId = track.id;
            const isActive = currentTrack?.id === trackId;
            const playing = isActive && isPlaying;
            const albumCover = track.album?.cover || track.album?.imageCover;
            const albumTitle = track.album?.title;
            const albumId = track.album?.id;
            const artistDisplay =
              track.artist?.name || track.artists?.[0]?.name;

            return (
              <div
                key={`${trackId}-${index}`}
                onClick={() => handlePlayTrack(track, index)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setContextMenu({
                    track,
                    index,
                    position: { x: e.clientX, y: e.clientY },
                  });
                }}
                className={`grid grid-cols-[36px_1fr_minmax(140px,1fr)_72px] gap-4 px-4 py-2.5 rounded-md cursor-pointer group transition-colors ${
                  isActive ? "bg-[#ffffff0a]" : "hover:bg-[#ffffff08]"
                }`}
              >
                <div className="flex items-center justify-end">
                  {playing ? (
                    <div className="flex items-end gap-[3px] h-4">
                      <span className="w-[3px] h-full bg-th-accent rounded-full playing-bar" />
                      <span
                        className="w-[3px] h-full bg-th-accent rounded-full playing-bar"
                        style={{ animationDelay: "0.2s" }}
                      />
                      <span
                        className="w-[3px] h-full bg-th-accent rounded-full playing-bar"
                        style={{ animationDelay: "0.4s" }}
                      />
                    </div>
                  ) : (
                    <>
                      <span
                        className={`text-[15px] tabular-nums group-hover:hidden ${
                          isActive ? "text-th-accent" : "text-th-text-muted"
                        }`}
                      >
                        {index + 1}
                      </span>
                      <Play
                        size={14}
                        fill="white"
                        className="text-white hidden group-hover:block"
                      />
                    </>
                  )}
                </div>

                <div className="flex items-center gap-3 min-w-0">
                  <div className="relative w-10 h-10 shrink-0 rounded bg-th-surface-hover overflow-hidden">
                    <TidalImage
                      src={getTidalImageUrl(albumCover, 160)}
                      alt={albumTitle || track.title}
                      className="w-full h-full"
                    />
                  </div>
                  <div className="flex flex-col justify-center min-w-0">
                    <span
                      className={`text-[15px] font-medium truncate leading-snug ${
                        isActive ? "text-th-accent" : "text-white"
                      }`}
                    >
                      {track.title}
                    </span>
                    {artistDisplay && (
                      <span className="text-[13px] text-th-text-muted truncate">
                        {artistDisplay}
                      </span>
                    )}
                  </div>
                </div>

                <div className="flex items-center min-w-0">
                  <span
                    className="text-[14px] text-th-text-muted truncate hover:text-white hover:underline transition-colors cursor-pointer"
                    onClick={(e) => {
                      e.stopPropagation();
                      if (albumId) {
                        navigateToAlbum(albumId, {
                          title: albumTitle,
                          cover: albumCover,
                          artistName: artistDisplay,
                        });
                      }
                    }}
                  >
                    {albumTitle || ""}
                  </span>
                </div>

                <div className="flex items-center justify-end text-[14px] text-th-text-muted tabular-nums">
                  {formatDuration(track.duration)}
                </div>
              </div>
            );
          })}
        </div>

        {tracks.length === 0 && (
          <div className="py-16 text-center">
            <p className="text-white font-semibold text-lg mb-2">
              No tracks available
            </p>
            <p className="text-th-text-muted text-sm">
              This artist doesn't have any popular tracks yet.
            </p>
          </div>
        )}
      </div>

      {contextMenu && (
        <TrackContextMenu
          track={contextMenu.track}
          index={contextMenu.index}
          cursorPosition={contextMenu.position}
          anchorRef={{ current: null }}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
