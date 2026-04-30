import { useEffect } from "react";
import { useAtomValue } from "jotai";
import { X } from "lucide-react";
import { currentTrackAtom, isPlayingAtom } from "../atoms/playback";
import { getTidalImageUrl, getTrackDisplayTitle } from "../types";
import { TrackArtists } from "./TrackArtists";

interface Props {
  open: boolean;
  onClose: () => void;
}

/**
 * Fullscreen ambient mode — turns the screen into a slowly breathing
 * digital painting of the current album art. Background is the cover
 * blown up and heavily blurred; the cover itself sits centered at a
 * comfortable size with a gentle scale pulse. No audio reactivity in
 * v1 — just CSS-driven motion that feels alive without being busy.
 */
export default function LivePaintingMode({ open, onClose }: Props) {
  const track = useAtomValue(currentTrackAtom);
  const isPlaying = useAtomValue(isPlayingAtom);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  const cover = track?.album?.cover ?? null;
  const big = cover ? getTidalImageUrl(cover, 1280) : null;
  const med = cover ? getTidalImageUrl(cover, 640) : null;

  return (
    <div className="fixed inset-0 z-[60] bg-black overflow-hidden cursor-default">
      {/* Backdrop: scaled + blurred cover, very saturated */}
      {big ? (
        <div
          aria-hidden
          className="absolute inset-0"
          style={{
            backgroundImage: `url(${big})`,
            backgroundSize: "cover",
            backgroundPosition: "center",
            transform: "scale(1.4)",
            filter: "blur(64px) saturate(1.4)",
            opacity: 0.85,
            animation: "lp-breath-bg 11s ease-in-out infinite",
            animationPlayState: isPlaying ? "running" : "paused",
          }}
        />
      ) : (
        <div className="absolute inset-0 bg-gradient-to-br from-th-elevated to-black" />
      )}

      {/* Vignette + global darkening for legibility */}
      <div
        aria-hidden
        className="absolute inset-0 pointer-events-none"
        style={{
          background:
            "radial-gradient(ellipse at center, rgba(0,0,0,0) 30%, rgba(0,0,0,0.55) 100%)",
        }}
      />

      {/* Foreground: album art + meta */}
      <div className="absolute inset-0 flex flex-col items-center justify-center px-8 gap-8">
        {med ? (
          <div
            className="relative"
            style={{
              animation: "lp-breath-cover 7s ease-in-out infinite",
              animationPlayState: isPlaying ? "running" : "paused",
            }}
          >
            <img
              src={med}
              alt={track?.album?.title ?? track?.title ?? ""}
              className="rounded-2xl shadow-[0_30px_80px_-10px_rgba(0,0,0,0.7)] object-cover"
              style={{
                width: "min(60vh, 60vw)",
                height: "min(60vh, 60vw)",
              }}
              draggable={false}
            />
            {/* Soft outer glow tinted from the art via mix-blend */}
            <div
              aria-hidden
              className="absolute -inset-10 rounded-3xl pointer-events-none"
              style={{
                backgroundImage: `url(${med})`,
                backgroundSize: "cover",
                backgroundPosition: "center",
                filter: "blur(50px) saturate(1.4)",
                opacity: 0.55,
                zIndex: -1,
              }}
            />
          </div>
        ) : (
          <div className="w-[40vh] h-[40vh] rounded-2xl bg-white/5 flex items-center justify-center text-white/30 text-sm">
            Sin pista
          </div>
        )}

        <div className="text-center max-w-[80vw]">
          <h1 className="text-white text-[clamp(20px,3vw,38px)] font-semibold leading-tight tracking-tight drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)]">
            {track ? getTrackDisplayTitle(track) : "—"}
          </h1>
          {track ? (
            <div className="text-white/75 text-[clamp(13px,1.5vw,18px)] mt-2 drop-shadow-[0_2px_6px_rgba(0,0,0,0.6)]">
              <TrackArtists artists={track.artists} artist={track.artist} />
            </div>
          ) : null}
          {track?.album?.title ? (
            <p className="text-white/45 text-[clamp(11px,1.1vw,14px)] mt-1.5 italic">
              {track.album.title}
            </p>
          ) : null}
        </div>
      </div>

      {/* Close button (top right) */}
      <button
        onClick={onClose}
        className="absolute top-5 right-5 w-10 h-10 rounded-full flex items-center justify-center bg-black/30 hover:bg-black/50 text-white/80 hover:text-white backdrop-blur-md transition-colors"
        aria-label="Salir"
      >
        <X size={20} />
      </button>

      {/* Subtle hint at bottom */}
      <p className="absolute bottom-4 left-1/2 -translate-x-1/2 text-[10px] uppercase tracking-[0.3em] text-white/30 select-none">
        Esc para salir
      </p>

      {/* Inline keyframes — keeps the component self-contained */}
      <style>{`
        @keyframes lp-breath-bg {
          0%, 100% {
            transform: scale(1.4);
            filter: blur(64px) saturate(1.4) brightness(1);
          }
          50% {
            transform: scale(1.5);
            filter: blur(72px) saturate(1.55) brightness(1.08);
          }
        }
        @keyframes lp-breath-cover {
          0%, 100% { transform: scale(1) translateY(0); }
          50%      { transform: scale(1.025) translateY(-4px); }
        }
      `}</style>
    </div>
  );
}
