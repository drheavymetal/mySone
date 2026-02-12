import { Play, Clock, Music, Loader2, Search } from "lucide-react";
import { useState, useEffect } from "react";
import { useAudioContext } from "../contexts/AudioContext";
import {
  getTidalImageUrl,
  type SearchResults,
  type Track,
  type AlbumDetail,
  type Playlist,
} from "../hooks/useAudio";
import TidalImage from "./TidalImage";

type SearchTab = "top" | "tracks" | "albums" | "playlists";

const TABS: { id: SearchTab; label: string }[] = [
  { id: "top", label: "Top results" },
  { id: "tracks", label: "Tracks" },
  { id: "albums", label: "Albums" },
  { id: "playlists", label: "Playlists" },
];

function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

interface SearchViewProps {
  query: string;
  onBack: () => void;
}

export default function SearchView({ query, onBack }: SearchViewProps) {
  const {
    playTrack,
    setQueueTracks,
    currentTrack,
    isPlaying,
    navigateToAlbum,
    navigateToPlaylist,
    searchTidal,
  } = useAudioContext();

  const [results, setResults] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<SearchTab>("top");

  useEffect(() => {
    if (!query.trim()) return;

    let active = true;
    setLoading(true);
    setError(null);

    searchTidal(query.trim(), 50)
      .then((r) => {
        if (active) setResults(r);
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
  }, [query]);

  const handlePlayTrack = (track: Track, allTracks: Track[], index: number) => {
    setQueueTracks(allTracks.slice(index + 1));
    playTrack(track);
  };

  if (loading) {
    return (
      <div className="flex-1 bg-linear-to-b from-[#1a1a1a] to-[#121212] flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <Loader2 size={28} className="animate-spin text-[#00FFFF]" />
          <p className="text-[#a6a6a6] text-sm">Searching for "{query}"...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex-1 bg-linear-to-b from-[#1a1a1a] to-[#121212] flex items-center justify-center">
        <div className="flex flex-col items-center gap-4 text-center px-8">
          <Search size={48} className="text-[#535353]" />
          <p className="text-white font-semibold text-lg">Search failed</p>
          <p className="text-[#a6a6a6] text-sm max-w-md">{error}</p>
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

  const noResults =
    results &&
    results.tracks.length === 0 &&
    results.albums.length === 0 &&
    results.artists.length === 0 &&
    results.playlists.length === 0;

  return (
    <div className="flex-1 bg-linear-to-b from-[#1a1a1a] to-[#121212] min-h-full">
      <div className="px-6 py-6">
        {/* Tab bar */}
        <div className="pb-6 flex items-center gap-2">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`px-4 py-1.5 rounded-full text-[13px] font-medium transition-colors duration-150 ${
                activeTab === tab.id
                  ? "bg-white text-black"
                  : "bg-white/7 text-[#e0e0e0] hover:bg-white/12"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {noResults && (
          <div className="flex flex-col items-center justify-center py-20 text-[#535353]">
            <Search size={48} className="mb-4" />
            <p className="text-white font-semibold text-lg mb-1">
              No results found
            </p>
            <p className="text-sm">Try a different search term</p>
          </div>
        )}

        {results && !noResults && (
          <div className="pb-8">
            {/* Top Results */}
            {activeTab === "top" && (
              <div className="flex flex-col gap-8">
                {results.tracks.length > 0 && (
                  <section>
                    <h2 className="text-[16px] font-bold text-white mb-3">
                      Tracks
                    </h2>
                    <TrackList
                      tracks={results.tracks.slice(0, 8)}
                      currentTrack={currentTrack}
                      isPlaying={isPlaying}
                      onPlay={handlePlayTrack}
                    />
                  </section>
                )}
                {results.albums.length > 0 && (
                  <section>
                    <h2 className="text-[16px] font-bold text-white mb-3">
                      Albums
                    </h2>
                    <AlbumGrid
                      albums={results.albums.slice(0, 6)}
                      onAlbumClick={navigateToAlbum}
                    />
                  </section>
                )}
                {results.playlists.length > 0 && (
                  <section>
                    <h2 className="text-[16px] font-bold text-white mb-3">
                      Playlists
                    </h2>
                    <PlaylistGrid
                      playlists={results.playlists.slice(0, 6)}
                      onPlaylistClick={navigateToPlaylist}
                    />
                  </section>
                )}
              </div>
            )}

            {/* Tracks tab */}
            {activeTab === "tracks" && results.tracks.length > 0 && (
              <TrackList
                tracks={results.tracks}
                currentTrack={currentTrack}
                isPlaying={isPlaying}
                onPlay={handlePlayTrack}
              />
            )}

            {/* Albums tab */}
            {activeTab === "albums" && results.albums.length > 0 && (
              <AlbumGrid
                albums={results.albums}
                onAlbumClick={navigateToAlbum}
              />
            )}

            {/* Playlists tab */}
            {activeTab === "playlists" && results.playlists.length > 0 && (
              <PlaylistGrid
                playlists={results.playlists}
                onPlaylistClick={navigateToPlaylist}
              />
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function TrackList({
  tracks,
  currentTrack,
  isPlaying,
  onPlay,
}: {
  tracks: Track[];
  currentTrack: Track | null;
  isPlaying: boolean;
  onPlay: (track: Track, allTracks: Track[], index: number) => void;
}) {
  return (
    <div className="flex flex-col">
      {/* Header row */}
      <div className="grid grid-cols-[36px_1fr_minmax(120px,1fr)_72px] gap-4 px-4 py-2 text-[12px] text-[#a6a6a6] uppercase tracking-widest border-b border-[#2a2a2a] mb-1">
        <span className="text-right">#</span>
        <span>Title</span>
        <span>Album</span>
        <span className="flex justify-end">
          <Clock size={15} />
        </span>
      </div>

      {tracks.map((track, i) => {
        const isActive = currentTrack?.id === track.id;
        const playing = isActive && isPlaying;
        return (
          <div
            key={`${track.id}-${i}`}
            onClick={() => onPlay(track, tracks, i)}
            className={`grid grid-cols-[36px_1fr_minmax(120px,1fr)_72px] gap-4 px-4 py-2.5 rounded-md cursor-pointer group transition-colors ${
              isActive ? "bg-[#ffffff0a]" : "hover:bg-[#ffffff08]"
            }`}
          >
            <div className="flex items-center justify-end">
              {playing ? (
                <div className="flex items-center gap-[3px]">
                  <span className="w-[3px] h-3 bg-[#00FFFF] rounded-full animate-pulse" />
                  <span
                    className="w-[3px] h-4 bg-[#00FFFF] rounded-full animate-pulse"
                    style={{ animationDelay: "0.15s" }}
                  />
                  <span
                    className="w-[3px] h-2.5 bg-[#00FFFF] rounded-full animate-pulse"
                    style={{ animationDelay: "0.3s" }}
                  />
                </div>
              ) : (
                <>
                  <span
                    className={`text-[15px] tabular-nums group-hover:hidden ${
                      isActive ? "text-[#00FFFF]" : "text-[#a6a6a6]"
                    }`}
                  >
                    {i + 1}
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
              <div className="w-10 h-10 rounded bg-[#282828] overflow-hidden shrink-0">
                <TidalImage
                  src={getTidalImageUrl(track.album?.cover, 80)}
                  alt={track.title}
                  className="w-full h-full"
                />
              </div>
              <div className="flex flex-col justify-center min-w-0">
                <span
                  className={`text-[15px] font-medium truncate leading-snug ${
                    isActive ? "text-[#00FFFF]" : "text-white"
                  }`}
                >
                  {track.title}
                </span>
                <span className="text-[13px] text-[#a6a6a6] truncate leading-snug">
                  {track.artist?.name || "Unknown Artist"}
                </span>
              </div>
            </div>
            <div className="flex items-center min-w-0">
              <span className="text-[14px] text-[#a6a6a6] truncate">
                {track.album?.title || ""}
              </span>
            </div>
            <div className="flex items-center justify-end text-[14px] text-[#a6a6a6] tabular-nums">
              {formatDuration(track.duration)}
            </div>
          </div>
        );
      })}
    </div>
  );
}

function AlbumGrid({
  albums,
  onAlbumClick,
}: {
  albums: AlbumDetail[];
  onAlbumClick: (
    id: number,
    info?: { title: string; cover?: string; artistName?: string }
  ) => void;
}) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5">
      {albums.map((album) => (
        <div
          key={album.id}
          onClick={() =>
            onAlbumClick(album.id, {
              title: album.title,
              cover: album.cover,
              artistName: album.artist?.name,
            })
          }
          className="p-3 bg-[#181818] hover:bg-[#282828] rounded-md cursor-pointer group transition-[background-color] duration-300"
        >
          <div className="aspect-square w-full rounded-md mb-3 relative overflow-hidden shadow-lg bg-[#282828]">
            <TidalImage
              src={getTidalImageUrl(album.cover, 320)}
              alt={album.title}
              className="w-full h-full transform group-hover:scale-105 transition-transform duration-500 ease-out"
            />
            <div className="absolute inset-0 bg-black/20 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            <div className="absolute bottom-2 right-2 w-10 h-10 bg-[#00FFFF] rounded-full flex items-center justify-center shadow-xl opacity-0 group-hover:opacity-100 translate-y-2 group-hover:translate-y-0 transition-[opacity,transform] duration-300 scale-90 group-hover:scale-100">
              <Play size={20} fill="black" className="text-black ml-1" />
            </div>
          </div>
          <h4 className="font-bold text-[15px] text-white truncate mb-1">
            {album.title}
          </h4>
          <p className="text-[13px] text-[#a6a6a6] truncate">
            {album.artist?.name || "Unknown Artist"}
            {album.releaseDate && (
              <span> &middot; {new Date(album.releaseDate).getFullYear()}</span>
            )}
          </p>
        </div>
      ))}
    </div>
  );
}

function PlaylistGrid({
  playlists,
  onPlaylistClick,
}: {
  playlists: Playlist[];
  onPlaylistClick: (
    id: string,
    info?: {
      title: string;
      image?: string;
      description?: string;
      creatorName?: string;
      numberOfTracks?: number;
    }
  ) => void;
}) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5">
      {playlists.map((pl) => (
        <div
          key={pl.uuid}
          onClick={() =>
            onPlaylistClick(pl.uuid, {
              title: pl.title,
              image: pl.image,
              description: pl.description,
              creatorName: pl.creator?.name || (pl.creator?.id === 0 ? "TIDAL" : undefined),
              numberOfTracks: pl.numberOfTracks,
            })
          }
          className="p-3 bg-[#181818] hover:bg-[#282828] rounded-md cursor-pointer group transition-[background-color] duration-300"
        >
          <div className="aspect-square w-full rounded-md mb-3 relative overflow-hidden shadow-lg bg-[#282828]">
            {pl.image ? (
              <TidalImage
                src={getTidalImageUrl(pl.image, 320)}
                alt={pl.title}
                type="playlist"
                className="w-full h-full transform group-hover:scale-105 transition-transform duration-500 ease-out"
              />
            ) : (
              <div className="w-full h-full flex items-center justify-center">
                <Music size={32} className="text-[#535353]" />
              </div>
            )}
            <div className="absolute inset-0 bg-black/20 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            <div className="absolute bottom-2 right-2 w-10 h-10 bg-[#00FFFF] rounded-full flex items-center justify-center shadow-xl opacity-0 group-hover:opacity-100 translate-y-2 group-hover:translate-y-0 transition-[opacity,transform] duration-300 scale-90 group-hover:scale-100">
              <Play size={20} fill="black" className="text-black ml-1" />
            </div>
          </div>
          <h4 className="font-bold text-[15px] text-white truncate mb-1">
            {pl.title}
          </h4>
          <p className="text-[13px] text-[#a6a6a6] line-clamp-2">
            {pl.description || (pl.creator?.name ? `By ${pl.creator.name}` : pl.creator?.id === 0 ? "By TIDAL" : "Playlist")}
          </p>
        </div>
      ))}
    </div>
  );
}
