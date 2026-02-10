import {
  Play,
  Pause,
  SkipBack,
  SkipForward,
  Repeat,
  Shuffle,
  Volume2,
  VolumeX,
  Volume1,
  Heart,
  Loader2,
  ListMusic,
  Mic2,
} from "lucide-react";
import { useAudioContext } from "../contexts/AudioContext";
import { getTidalImageUrl } from "../hooks/useAudio";
import TidalImage from "./TidalImage";
import { useState, useEffect, useRef, useCallback } from "react";

export default function PlayerBar() {
  const {
    isPlaying,
    currentTrack,
    authTokens,
    volume,
    pauseTrack,
    resumeTrack,
    setVolume,
    playNext,
    playPrevious,
    getPlaybackPosition,
    seekTo,
    isTrackFavorited,
    addFavoriteTrack,
    removeFavoriteTrack,
    toggleDrawer,
    openDrawerToTab,
    streamInfo,
  } = useAudioContext();

  const [currentTime, setCurrentTime] = useState(0);
  const [isLiked, setIsLiked] = useState(false);
  const [isLikeLoading, setIsLikeLoading] = useState(false);
  const [isLikePending, setIsLikePending] = useState(false);
  const [isShuffle, setIsShuffle] = useState(false);
  const [repeatMode, setRepeatMode] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const [dragTime, setDragTime] = useState(0);
  const [isHoveringProgress, setIsHoveringProgress] = useState(false);
  const progressRef = useRef<HTMLDivElement>(null);

  const toggleLike = useCallback(async () => {
    if (!currentTrack || isLikeLoading || isLikePending) return;

    setIsLikePending(true);
    const nextLiked = !isLiked;

    try {
      if (nextLiked) {
        await addFavoriteTrack(currentTrack.id);
      } else {
        await removeFavoriteTrack(currentTrack.id);
      }
      setIsLiked(nextLiked);
    } catch (err) {
      console.error("Failed to toggle track favorite:", err);
    } finally {
      setIsLikePending(false);
    }
  }, [
    currentTrack,
    isLikeLoading,
    isLikePending,
    isLiked,
    addFavoriteTrack,
    removeFavoriteTrack,
  ]);

  // Sync progress with backend playback position
  useEffect(() => {
    if (!isPlaying || !currentTrack || isDragging) return;

    const syncPosition = async () => {
      const pos = await getPlaybackPosition();
      setCurrentTime(pos);
    };

    syncPosition();
    const interval = setInterval(syncPosition, 500);
    return () => clearInterval(interval);
  }, [isPlaying, currentTrack, isDragging]);

  // Reset on track change
  useEffect(() => {
    setCurrentTime(0);
  }, [currentTrack?.id]);

  // Sync favorite state with backend for currently playing track
  useEffect(() => {
    if (!currentTrack) {
      setIsLiked(false);
      setIsLikeLoading(false);
      setIsLikePending(false);
      return;
    }

    if (!authTokens?.user_id) {
      setIsLikeLoading(true);
      return;
    }

    let cancelled = false;
    setIsLikeLoading(true);
    setIsLikePending(false);

    isTrackFavorited(currentTrack.id)
      .then((favorited) => {
        if (!cancelled) {
          setIsLiked(favorited);
        }
      })
      .catch((err) => {
        console.error("Failed to check current track favorite state:", err);
        if (!cancelled) {
          setIsLiked(false);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLikeLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [currentTrack?.id, authTokens?.user_id]);

  const duration = currentTrack?.duration ?? 0;
  const displayTime = isDragging ? dragTime : currentTime;
  const progress = duration > 0 ? (displayTime / duration) * 100 : 0;
  const clampedProgress = Math.min(100, Math.max(0, progress));

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  };

  // --- Scrubber drag logic ---
  // On WebKit-based engines (Linux WebKitGTK, macOS WebKit used by Tauri),
  // getBoundingClientRect() returns CSS-pixel values while clientX is in
  // viewport (zoomed) coordinates.  Detect the mismatch by comparing
  // rect.width with offsetWidth (always CSS pixels) and adjust.
  const getTimeFromClientX = useCallback(
    (clientX: number) => {
      if (!progressRef.current || !currentTrack) return 0;
      const el = progressRef.current;
      const rect = el.getBoundingClientRect();

      let adjustedX = clientX;
      const cssWidth = el.offsetWidth;
      if (cssWidth > 0 && Math.abs(rect.width / cssWidth - 1) < 0.01) {
        const zoom = parseFloat(document.documentElement.style.zoom || "1");
        if (zoom !== 1) {
          adjustedX = clientX / zoom;
        }
      }

      const pct = Math.max(
        0,
        Math.min(1, (adjustedX - rect.left) / rect.width)
      );
      return pct * currentTrack.duration;
    },
    [currentTrack]
  );

  // Register mousemove / mouseup synchronously inside the mousedown handler
  // so that even an instant click (mouseup fires before React re-renders)
  // is captured correctly.
  const handleProgressMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (!currentTrack) return;
      e.preventDefault();
      const startTime = getTimeFromClientX(e.clientX);
      setIsDragging(true);
      setDragTime(startTime);

      const onMove = (ev: MouseEvent) => {
        setDragTime(getTimeFromClientX(ev.clientX));
      };

      const onUp = async (ev: MouseEvent) => {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        const finalTime = getTimeFromClientX(ev.clientX);
        setIsDragging(false);
        setIsHoveringProgress(false);
        setCurrentTime(finalTime);
        await seekTo(finalTime);
      };

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [currentTrack, getTimeFromClientX, seekTo]
  );

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setVolume(parseFloat(e.target.value));
  };

  const VolumeIcon = volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;

  const getQualityBadge = () => {
    const quality = streamInfo?.audioQuality || currentTrack?.audioQuality;
    if (!quality) return null;

    const isMax = quality === "HI_RES_LOSSLESS" || quality === "HI_RES";
    const isHiFi = quality === "LOSSLESS";

    // Build detail string like "24-BIT 48kHz FLAC"
    const parts: string[] = [];
    if (streamInfo?.bitDepth) parts.push(`${streamInfo.bitDepth}-BIT`);
    if (streamInfo?.sampleRate) {
      const sr = streamInfo.sampleRate;
      parts.push(
        sr >= 1000
          ? `${(sr / 1000).toFixed(sr % 1000 === 0 ? 0 : 1)}kHz`
          : `${sr}Hz`
      );
    }
    if (streamInfo?.codec) parts.push(streamInfo.codec);
    const detail = parts.join(" ");

    return (
      <div className="flex items-center gap-2">
        {detail && (
          <span className="text-[10px] text-[#888] font-medium tracking-wide hidden xl:inline">
            {detail}
          </span>
        )}
        <span
          className={`px-2 py-0.5 text-[9px] font-black rounded tracking-wider leading-none ${
            isMax
              ? "bg-[#ffa726] text-black"
              : isHiFi
              ? "bg-[#1ed760] text-black"
              : "bg-[#555] text-white"
          }`}
        >
          {isMax ? "MAX" : isHiFi ? "HiFi" : "HIGH"}
        </span>
      </div>
    );
  };

  return (
    <div className="player-bar h-[90px] bg-[#181818] border-t border-white/[0.06] px-4 flex items-center justify-between relative z-50 select-none">
      {/* Left: Track Info */}
      <div className="flex items-center gap-3 w-[30%] min-w-[180px]">
        {currentTrack ? (
          <>
            <div
              onClick={toggleDrawer}
              className="w-16 h-16 rounded-md bg-[#282828] flex-shrink-0 overflow-hidden shadow-lg shadow-black/40 group cursor-pointer"
            >
              <TidalImage
                src={getTidalImageUrl(currentTrack.album?.cover, 160)}
                alt={currentTrack.album?.title || currentTrack.title}
                className="w-full h-full object-cover transform group-hover:scale-110 transition-transform duration-500"
              />
            </div>
            <div className="flex flex-col justify-center min-w-0 gap-0.5">
              <span className="text-white text-[13px] font-semibold truncate hover:underline cursor-pointer leading-tight">
                {currentTrack.title}
              </span>
              <span className="text-[#b3b3b3] text-[11px] truncate hover:text-white hover:underline cursor-pointer transition-colors duration-200">
                {currentTrack.artist?.name || "Unknown Artist"}
              </span>
            </div>
            <button
              onClick={toggleLike}
              disabled={isLikeLoading || isLikePending}
              className={`ml-1 flex-shrink-0 transition-all duration-200 active:scale-90 ${
                isLiked ? "text-[#1ed760]" : "text-[#666] hover:text-white"
              } ${
                isLikeLoading || isLikePending
                  ? "opacity-70 cursor-not-allowed"
                  : ""
              }`}
            >
              {isLikeLoading || isLikePending ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <Heart
                  size={16}
                  fill={isLiked ? "currentColor" : "none"}
                  strokeWidth={isLiked ? 0 : 2}
                />
              )}
            </button>
          </>
        ) : (
          <div className="text-[#666] text-sm">No track playing</div>
        )}
      </div>

      {/* Center: Controls + Scrubber */}
      <div className="flex flex-col items-center w-[40%] max-w-[600px] gap-1">
        {/* Transport buttons */}
        <div className="flex items-center gap-4">
          <button
            onClick={() => setIsShuffle(!isShuffle)}
            className={`w-8 h-8 flex items-center justify-center rounded-full transition-all duration-200 active:scale-90 relative ${
              isShuffle
                ? "text-[#00ffff]"
                : "text-[#b3b3b3] hover:text-white hover:bg-white/[0.07]"
            }`}
          >
            <Shuffle size={15} strokeWidth={2} />
            {isShuffle && (
              <span className="absolute -bottom-0.5 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full bg-[#00ffff]" />
            )}
          </button>
          <button
            onClick={playPrevious}
            className="w-8 h-8 flex items-center justify-center rounded-full text-[#b3b3b3] hover:text-white hover:bg-white/[0.07] transition-all duration-150 active:scale-90"
          >
            <SkipBack size={18} fill="currentColor" />
          </button>
          <button
            onClick={() => (isPlaying ? pauseTrack() : resumeTrack())}
            className="w-9 h-9 bg-white rounded-full flex items-center justify-center hover:scale-105 active:scale-95 transition-all duration-150"
          >
            {isPlaying ? (
              <Pause size={17} fill="black" className="text-black" />
            ) : (
              <Play size={17} fill="black" className="text-black ml-0.5" />
            )}
          </button>
          <button
            onClick={playNext}
            className="w-8 h-8 flex items-center justify-center rounded-full text-[#b3b3b3] hover:text-white hover:bg-white/[0.07] transition-all duration-150 active:scale-90"
          >
            <SkipForward size={18} fill="currentColor" />
          </button>
          <button
            onClick={() => setRepeatMode((repeatMode + 1) % 3)}
            className={`w-8 h-8 flex items-center justify-center rounded-full transition-all duration-200 active:scale-90 relative ${
              repeatMode > 0
                ? "text-[#00ffff]"
                : "text-[#b3b3b3] hover:text-white hover:bg-white/[0.07]"
            }`}
          >
            <Repeat size={15} strokeWidth={2} />
            {repeatMode === 2 && (
              <span className="absolute -top-0.5 -right-0.5 text-[7px] font-bold bg-[#00ffff] text-black rounded-full w-3 h-3 flex items-center justify-center leading-none">
                1
              </span>
            )}
            {repeatMode > 0 && (
              <span className="absolute -bottom-0.5 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full bg-[#00ffff]" />
            )}
          </button>
        </div>

        {/* Progress bar / scrubber */}
        <div className="w-full flex items-center gap-2 text-[#a0a0a0]">
          <span className="min-w-[40px] text-right text-[11px] tabular-nums select-none">
            {formatTime(displayTime)}
          </span>
          <div
            ref={progressRef}
            onMouseDown={handleProgressMouseDown}
            onMouseEnter={() => setIsHoveringProgress(true)}
            onMouseLeave={() => {
              if (!isDragging) setIsHoveringProgress(false);
            }}
            className="scrubber flex-1 relative cursor-pointer py-[6px]"
          >
            {/* Track background */}
            <div
              className={`relative w-full rounded-full transition-[height] duration-100 ${
                isHoveringProgress || isDragging ? "h-[5px]" : "h-[3px]"
              }`}
            >
              {/* Unfilled track */}
              <div className="absolute inset-0 bg-white/[0.12] rounded-full" />
              {/* Filled track */}
              <div
                className={`absolute h-full rounded-full transition-colors duration-100 ${
                  isHoveringProgress || isDragging
                    ? "bg-[#00ffff]"
                    : "bg-white/60"
                }`}
                style={{ width: `${clampedProgress}%` }}
              />
            </div>
            {/* Scrub handle */}
            <div
              className={`absolute top-1/2 -translate-y-1/2 w-3 h-3 bg-white rounded-full shadow-md shadow-black/50 pointer-events-none transition-opacity duration-100 ${
                isHoveringProgress || isDragging ? "opacity-100" : "opacity-0"
              }`}
              style={{
                left: `calc(${clampedProgress}% - 6px)`,
              }}
            />
          </div>
          <span className="min-w-[40px] text-[11px] tabular-nums select-none">
            {currentTrack ? formatTime(duration) : "0:00"}
          </span>
        </div>
      </div>

      {/* Right: Volume & Extras */}
      <div className="flex items-center justify-end gap-4 w-[30%] min-w-[180px]">
        {getQualityBadge()}

        <button
          onClick={() => openDrawerToTab("lyrics")}
          className="text-[#666] hover:text-white transition-colors duration-150"
          title="Lyrics"
        >
          <Mic2 size={16} strokeWidth={2} />
        </button>

        <div className="flex items-center gap-2 group/vol w-[120px]">
          <button
            onClick={() => {
              setVolume(volume > 0 ? 0 : 1);
            }}
            className="text-[#b3b3b3] hover:text-white transition-colors duration-150 flex-shrink-0"
          >
            <VolumeIcon size={16} strokeWidth={2} />
          </button>
          <div className="flex-1 relative rounded-full cursor-pointer">
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              value={volume}
              onChange={handleVolumeChange}
              className="absolute inset-0 w-full h-full opacity-0 cursor-pointer z-10"
            />
            <div className="relative h-[3px] group-hover/vol:h-[4px] transition-[height] duration-100 rounded-full">
              <div className="absolute inset-0 bg-white/[0.12] rounded-full" />
              <div
                className="absolute h-full bg-white/70 group-hover/vol:bg-[#00ffff] rounded-full transition-colors duration-100"
                style={{ width: `${volume * 100}%` }}
              />
              <div
                className="absolute top-1/2 -translate-y-1/2 w-[10px] h-[10px] bg-white rounded-full shadow-sm opacity-0 group-hover/vol:opacity-100 transition-opacity duration-100"
                style={{ left: `calc(${volume * 100}% - 5px)` }}
              />
            </div>
          </div>
        </div>

        <button
          onClick={() => openDrawerToTab("queue")}
          className="text-[#666] hover:text-white transition-colors duration-150"
          title="Play queue"
        >
          <ListMusic size={16} strokeWidth={2} />
        </button>
      </div>
    </div>
  );
}
