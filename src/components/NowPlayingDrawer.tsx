import {
  X,
  ListMusic,
  Sparkles,
  Mic2,
  Users,
  Music,
  Loader2,
  Plus,
} from "lucide-react";
import { useState, useEffect } from "react";
import { useAudioContext } from "../contexts/AudioContext";
import {
  getTidalImageUrl,
  type Track,
  type Lyrics,
  type Credit,
} from "../hooks/useAudio";
import TidalImage from "./TidalImage";

type TabId = "queue" | "suggested" | "lyrics" | "credits";

const TABS: { id: TabId; label: string; icon: typeof ListMusic }[] = [
  { id: "queue", label: "Play queue", icon: ListMusic },
  { id: "suggested", label: "Suggested tracks", icon: Sparkles },
  { id: "lyrics", label: "Lyrics", icon: Mic2 },
  { id: "credits", label: "Credits", icon: Users },
];

// ─── Queue Tab ───────────────────────────────────────────────────────────────

function QueueTab() {
  const {
    currentTrack,
    queue,
    history,
    isPlaying,
    playTrack,
    setQueueTracks,
    removeFromQueue,
  } = useAudioContext();

  return (
    <div className="flex flex-col gap-6">
      {/* History */}
      {history.length > 0 && (
        <section>
          <h3 className="text-[13px] font-bold text-[#a6a6a6] uppercase tracking-wider mb-3">
            History
          </h3>
          <div className="flex flex-col gap-0.5">
            {history
              .slice(-10)
              .reverse()
              .map((track, i) => (
                <TrackRow
                  key={`hist-${track.id}-${i}`}
                  track={track}
                  isActive={false}
                  isPlaying={false}
                  dimmed
                  onClick={() => playTrack(track)}
                />
              ))}
          </div>
        </section>
      )}

      {/* Now Playing */}
      {currentTrack && (
        <section>
          <h3 className="text-[13px] font-bold text-[#a6a6a6] uppercase tracking-wider mb-3">
            Now playing
          </h3>
          <TrackRow
            track={currentTrack}
            isActive
            isPlaying={isPlaying}
            onClick={() => {}}
          />
        </section>
      )}

      {/* Next Up */}
      {queue.length > 0 && (
        <section>
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-[13px] font-bold text-[#a6a6a6] uppercase tracking-wider">
              Next up
            </h3>
            <button
              onClick={() => setQueueTracks([])}
              className="text-[11px] text-[#a6a6a6] hover:text-white transition-colors"
            >
              Clear
            </button>
          </div>
          <div className="flex flex-col gap-0.5">
            {queue.map((track, i) => (
              <TrackRow
                key={`queue-${track.id}-${i}`}
                track={track}
                isActive={false}
                isPlaying={false}
                onClick={() => {
                  // Play this track and set remaining as new queue
                  const remaining = queue.slice(i + 1);
                  setQueueTracks(remaining);
                  playTrack(track);
                }}
                onRemove={() => removeFromQueue(i)}
              />
            ))}
          </div>
        </section>
      )}

      {queue.length === 0 && !currentTrack && (
        <div className="flex flex-col items-center justify-center py-16 text-[#535353]">
          <Music size={40} className="mb-3" />
          <p className="text-sm">Queue is empty</p>
        </div>
      )}
    </div>
  );
}

// ─── Suggested Tracks Tab ────────────────────────────────────────────────────

function SuggestedTab() {
  const { currentTrack, getTrackRadio, playTrack, addToQueue } =
    useAudioContext();
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentTrack) return;

    let active = true;
    setLoading(true);
    setError(null);

    getTrackRadio(currentTrack.id, 20)
      .then((result) => {
        if (active) setTracks(result);
      })
      .catch((err) => {
        if (active) setError(String(err));
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [currentTrack?.id]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 size={24} className="animate-spin text-[#00FFFF]" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-[#535353]">
        <Sparkles size={40} className="mb-3" />
        <p className="text-sm">Suggested tracks not available</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0.5">
      {tracks.map((track, i) => (
        <TrackRow
          key={`sug-${track.id}-${i}`}
          track={track}
          isActive={currentTrack?.id === track.id}
          isPlaying={false}
          onClick={() => playTrack(track)}
          onAdd={() => addToQueue(track)}
        />
      ))}
    </div>
  );
}

// ─── Lyrics Tab ──────────────────────────────────────────────────────────────

function LyricsTab() {
  const { currentTrack, getTrackLyrics } = useAudioContext();
  const [lyrics, setLyrics] = useState<Lyrics | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentTrack) return;

    let active = true;
    setLoading(true);
    setError(null);
    setLyrics(null);

    getTrackLyrics(currentTrack.id)
      .then((result) => {
        if (active) setLyrics(result);
      })
      .catch((err) => {
        if (active) setError(String(err));
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [currentTrack?.id]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 size={24} className="animate-spin text-[#00FFFF]" />
      </div>
    );
  }

  if (error || !lyrics?.lyrics) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-[#535353]">
        <Mic2 size={40} className="mb-3" />
        <p className="text-sm">No lyrics available for this track</p>
      </div>
    );
  }

  return (
    <div
      className="whitespace-pre-wrap text-[16px] leading-relaxed text-[#e0e0e0] pr-4"
      dir={lyrics.isRightToLeft ? "rtl" : "ltr"}
    >
      {lyrics.lyrics}
      {lyrics.lyricsProvider && (
        <p className="mt-8 text-[11px] text-[#535353]">
          Lyrics provided by {lyrics.lyricsProvider}
        </p>
      )}
    </div>
  );
}

// ─── Credits Tab ─────────────────────────────────────────────────────────────

function CreditsTab() {
  const { currentTrack, getTrackCredits } = useAudioContext();
  const [credits, setCredits] = useState<Credit[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentTrack) return;

    let active = true;
    setLoading(true);
    setError(null);
    setCredits([]);

    getTrackCredits(currentTrack.id)
      .then((result) => {
        if (active) setCredits(result);
      })
      .catch((err) => {
        if (active) setError(String(err));
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [currentTrack?.id]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 size={24} className="animate-spin text-[#00FFFF]" />
      </div>
    );
  }

  if (error || credits.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-[#535353]">
        <Users size={40} className="mb-3" />
        <p className="text-sm">No credits available for this track</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      {credits.map((credit, i) => (
        <div key={`${credit.creditType}-${i}`}>
          <h4 className="text-[12px] font-bold text-[#a6a6a6] uppercase tracking-wider mb-2">
            {credit.creditType}
          </h4>
          <div className="flex flex-col gap-1">
            {credit.contributors.map((c, j) => (
              <span
                key={`${c.name}-${j}`}
                className="text-[14px] text-white/90"
              >
                {c.name}
              </span>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

// ─── Shared Track Row ────────────────────────────────────────────────────────

function TrackRow({
  track,
  isActive,
  isPlaying,
  dimmed,
  onClick,
  onRemove,
  onAdd,
}: {
  track: Track;
  isActive: boolean;
  isPlaying: boolean;
  dimmed?: boolean;
  onClick: () => void;
  onRemove?: () => void;
  onAdd?: () => void;
}) {
  return (
    <div
      onClick={onClick}
      className={`flex items-center gap-3 px-3 py-2 rounded-md cursor-pointer group transition-colors ${
        isActive ? "bg-white/[0.08]" : "hover:bg-white/[0.05]"
      } ${dimmed ? "opacity-50" : ""}`}
    >
      <div className="w-10 h-10 rounded bg-[#282828] overflow-hidden flex-shrink-0 relative">
        <TidalImage
          src={getTidalImageUrl(track.album?.cover, 80)}
          alt={track.title}
          className="w-full h-full"
        />
        {isActive && isPlaying && (
          <div className="absolute inset-0 bg-black/40 flex items-center justify-center">
            <div className="flex items-center gap-[2px]">
              <span className="w-[2px] h-2.5 bg-[#00FFFF] rounded-full animate-pulse" />
              <span
                className="w-[2px] h-3.5 bg-[#00FFFF] rounded-full animate-pulse"
                style={{ animationDelay: "0.15s" }}
              />
              <span
                className="w-[2px] h-2 bg-[#00FFFF] rounded-full animate-pulse"
                style={{ animationDelay: "0.3s" }}
              />
            </div>
          </div>
        )}
      </div>
      <div className="flex-1 min-w-0">
        <p
          className={`text-[13px] font-medium truncate ${
            isActive ? "text-[#00FFFF]" : "text-white"
          }`}
        >
          {track.title}
        </p>
        <p className="text-[11px] text-[#a6a6a6] truncate">
          {track.artist?.name || "Unknown Artist"}
        </p>
      </div>
      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
        {onAdd && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onAdd();
            }}
            className="w-7 h-7 rounded-full flex items-center justify-center text-[#a6a6a6] hover:text-white hover:bg-white/10 transition-all"
            title="Add to queue"
          >
            <Plus size={14} />
          </button>
        )}
        {onRemove && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onRemove();
            }}
            className="w-7 h-7 rounded-full flex items-center justify-center text-[#a6a6a6] hover:text-white hover:bg-white/10 transition-all"
            title="Remove"
          >
            <X size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

// ─── Main Drawer ─────────────────────────────────────────────────────────────

export default function NowPlayingDrawer() {
  const { currentTrack, drawerOpen, setDrawerOpen } = useAudioContext();
  const [activeTab, setActiveTab] = useState<TabId>("queue");

  // Close on Escape
  useEffect(() => {
    if (!drawerOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") setDrawerOpen(false);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [drawerOpen, setDrawerOpen]);

  if (!drawerOpen || !currentTrack) return null;

  return (
    <div className="fixed inset-0 z-40 flex flex-col">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={() => setDrawerOpen(false)}
      />

      {/* Drawer content - above backdrop, below player bar (z-50) */}
      <div className="relative z-10 flex-1 flex overflow-hidden bg-[#121212] animate-slideUp">
        {/* Left: Album Art */}
        <div className="w-[45%] min-w-[300px] max-w-[500px] flex flex-col items-center justify-center p-10 gap-6">
          <div className="w-full max-w-[360px] aspect-square rounded-lg overflow-hidden shadow-2xl shadow-black/60">
            <TidalImage
              src={getTidalImageUrl(currentTrack.album?.cover, 640)}
              alt={currentTrack.album?.title || currentTrack.title}
              className="w-full h-full"
            />
          </div>
          <div className="text-center w-full max-w-[360px]">
            <h2 className="text-[22px] font-bold text-white truncate">
              {currentTrack.title}
            </h2>
            <p className="text-[15px] text-[#a6a6a6] truncate mt-1">
              {currentTrack.artist?.name || "Unknown Artist"}
            </p>
          </div>
        </div>

        {/* Right: Tabs */}
        <div className="flex-1 flex flex-col min-w-0 border-l border-white/[0.06]">
          {/* Close button */}
          <div className="flex items-center justify-between px-6 pt-5 pb-2">
            <div className="flex items-center gap-1">
              {TABS.map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex items-center gap-2 px-4 py-2 rounded-full text-[13px] font-medium transition-all ${
                    activeTab === tab.id
                      ? "bg-white/[0.12] text-white"
                      : "text-[#a6a6a6] hover:text-white hover:bg-white/[0.05]"
                  }`}
                >
                  <tab.icon size={14} />
                  {tab.label}
                </button>
              ))}
            </div>
            <button
              onClick={() => setDrawerOpen(false)}
              className="w-8 h-8 rounded-full flex items-center justify-center text-[#a6a6a6] hover:text-white hover:bg-white/[0.08] transition-all flex-shrink-0 ml-2"
            >
              <X size={18} />
            </button>
          </div>

          {/* Tab content */}
          <div className="flex-1 overflow-y-auto px-6 py-4 scrollbar-thin scrollbar-thumb-[#333] scrollbar-track-transparent">
            {activeTab === "queue" && <QueueTab />}
            {activeTab === "suggested" && <SuggestedTab />}
            {activeTab === "lyrics" && <LyricsTab />}
            {activeTab === "credits" && <CreditsTab />}
          </div>
        </div>
      </div>
    </div>
  );
}
