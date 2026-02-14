import { Play, Music, Loader2, Search, MoreHorizontal, User } from "lucide-react";
import { useState, useEffect, useCallback } from "react";
import { useAudioContext } from "../contexts/AudioContext";
import {
  getTidalImageUrl,
  type SearchResults,
  type Track,
  type AlbumDetail,
  type Playlist,
  type MediaItemType,
} from "../hooks/useAudio";
import TidalImage from "./TidalImage";
import MediaContextMenu from "./MediaContextMenu";
import ReusableTrackList from "./TrackList";

type SearchTab = "top" | "tracks" | "artists" | "albums" | "playlists";

const TABS: { id: SearchTab; label: string }[] = [
  { id: "top", label: "Top results" },
  { id: "tracks", label: "Tracks" },
  { id: "playlists", label: "Playlists" },
  { id: "albums", label: "Albums" },
  { id: "artists", label: "Artists" },
];

interface SearchViewProps {
  query: string;
  onBack: () => void;
}

export default function SearchView({ query, onBack }: SearchViewProps) {
  const {
    playTrack,
    setQueueTracks,
    navigateToAlbum,
    navigateToPlaylist,
    navigateToArtist,
    searchTidal,
  } = useAudioContext();

  const [results, setResults] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<SearchTab>("top");

  // Context menu state
  const [contextMenu, setContextMenu] = useState<{
    item: MediaItemType;
    position: { x: number; y: number };
  } | null>(null);

  const handleAlbumContextMenu = useCallback(
    (e: React.MouseEvent, album: AlbumDetail) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({
        item: {
          type: "album",
          id: album.id,
          title: album.title,
          cover: album.cover,
          artistName: album.artist?.name,
        },
        position: { x: e.clientX, y: e.clientY },
      });
    },
    []
  );

  const handlePlaylistContextMenu = useCallback(
    (e: React.MouseEvent, pl: Playlist) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({
        item: {
          type: "playlist",
          uuid: pl.uuid,
          title: pl.title,
          image: pl.image,
          creatorName: pl.creator?.name || (pl.creator?.id === 0 ? "TIDAL" : undefined),
        },
        position: { x: e.clientX, y: e.clientY },
      });
    },
    []
  );

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

  const handlePlayTrack = (track: Track, index: number) => {
    const allTracks = results?.tracks || [];
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
                    <ReusableTrackList
                      tracks={results.tracks.slice(0, 8)}
                      onPlay={handlePlayTrack}
                      showCover={true}
                      showArtist={true}
                      showAlbum={true}
                      context="search"
                    />
                  </section>
                )}
                {results.artists.length > 0 && (
                  <section>
                    <h2 className="text-[16px] font-bold text-white mb-3">
                      Artists
                    </h2>
                    <ArtistGrid
                      artists={results.artists.slice(0, 6)}
                      onArtistClick={(artist) =>
                        navigateToArtist(artist.id, {
                          name: artist.name,
                          picture: artist.picture,
                        })
                      }
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
                      onContextMenu={handleAlbumContextMenu}
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
                      onContextMenu={handlePlaylistContextMenu}
                    />
                  </section>
                )}
              </div>
            )}

            {/* Tracks tab */}
            {activeTab === "tracks" && results.tracks.length > 0 && (
              <ReusableTrackList
                tracks={results.tracks}
                onPlay={handlePlayTrack}
                showCover={true}
                showArtist={true}
                showAlbum={true}
                context="search"
              />
            )}

            {/* Artists tab */}
            {activeTab === "artists" && results.artists.length > 0 && (
              <ArtistGrid
                artists={results.artists}
                onArtistClick={(artist) =>
                  navigateToArtist(artist.id, {
                    name: artist.name,
                    picture: artist.picture,
                  })
                }
                large
              />
            )}

            {/* Albums tab */}
            {activeTab === "albums" && results.albums.length > 0 && (
              <AlbumGrid
                albums={results.albums}
                onAlbumClick={navigateToAlbum}
                onContextMenu={handleAlbumContextMenu}
                large
              />
            )}

            {/* Playlists tab */}
            {activeTab === "playlists" && results.playlists.length > 0 && (
              <PlaylistGrid
                playlists={results.playlists}
                onPlaylistClick={navigateToPlaylist}
                onContextMenu={handlePlaylistContextMenu}
                large
              />
            )}
          </div>
        )}

        {/* Media context menu */}
        {contextMenu && (
          <MediaContextMenu
            item={contextMenu.item}
            cursorPosition={contextMenu.position}
            onClose={() => setContextMenu(null)}
          />
        )}
      </div>
    </div>
  );
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function ArtistGrid({
  artists,
  onArtistClick,
  large = false,
}: {
  artists: { id: number; name: string; picture?: string }[];
  onArtistClick: (artist: { id: number; name: string; picture?: string }) => void;
  large?: boolean;
}) {
  return (
    <div
      className={
        large
          ? "grid grid-cols-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-5"
          : "grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5"
      }
    >
      {artists.map((artist) => (
        <div
          key={artist.id}
          onClick={() => onArtistClick(artist)}
          className="p-3 bg-[#181818] hover:bg-[#282828] rounded-md cursor-pointer group transition-[background-color] duration-300 flex flex-col items-center"
        >
          <div className="aspect-square w-full rounded-full mb-3 relative overflow-hidden shadow-lg bg-[#282828]">
            {artist.picture ? (
              <TidalImage
                src={getTidalImageUrl(artist.picture, 320)}
                alt={artist.name}
                className="w-full h-full object-cover transform group-hover:scale-105 transition-transform duration-500 ease-out"
              />
            ) : (
              <div className="w-full h-full flex items-center justify-center">
                <User size={48} className="text-[#535353]" />
              </div>
            )}
            <div className="absolute inset-0 bg-black/20 opacity-0 group-hover:opacity-100 transition-opacity duration-300 rounded-full" />
            <div className="absolute bottom-2 right-2 w-10 h-10 bg-[#00FFFF] rounded-full flex items-center justify-center shadow-xl opacity-0 group-hover:opacity-100 translate-y-2 group-hover:translate-y-0 transition-[opacity,transform] duration-300 scale-90 group-hover:scale-100">
              <Play size={20} fill="black" className="text-black ml-1" />
            </div>
          </div>
          <h4 className="font-bold text-[15px] text-white truncate w-full text-center mb-1">
            {artist.name}
          </h4>
          <p className="text-[13px] text-[#a6a6a6]">Artist</p>
        </div>
      ))}
    </div>
  );
}

function AlbumGrid({
  albums,
  onAlbumClick,
  onContextMenu,
  large = false,
}: {
  albums: AlbumDetail[];
  onAlbumClick: (
    id: number,
    info?: { title: string; cover?: string; artistName?: string }
  ) => void;
  onContextMenu?: (e: React.MouseEvent, album: AlbumDetail) => void;
  large?: boolean;
}) {
  return (
    <div
      className={
        large
          ? "grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-3 xl:grid-cols-4 gap-5"
          : "grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5"
      }
    >
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
          onContextMenu={onContextMenu ? (e) => onContextMenu(e, album) : undefined}
          className="p-3 bg-[#181818] hover:bg-[#282828] rounded-md cursor-pointer group transition-[background-color] duration-300"
        >
          <div className="aspect-square w-full rounded-md mb-3 relative overflow-hidden shadow-lg bg-[#282828]">
            <TidalImage
              src={getTidalImageUrl(album.cover, 320)}
              alt={album.title}
              className="w-full h-full transform group-hover:scale-105 transition-transform duration-500 ease-out"
            />
            <div className="absolute inset-0 bg-black/20 opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            {/* Three-dot button */}
            <button
              className="absolute top-2 right-2 w-8 h-8 bg-black/60 rounded-full flex items-center justify-center shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-200 hover:bg-black/80 z-10"
              title="More options"
              onClick={(e) => {
                e.stopPropagation();
                onContextMenu?.(e, album);
              }}
            >
              <MoreHorizontal size={16} className="text-white" />
            </button>
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
  onContextMenu,
  large = false,
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
  onContextMenu?: (e: React.MouseEvent, playlist: Playlist) => void;
  large?: boolean;
}) {
  return (
    <div
      className={
        large
          ? "grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-3 xl:grid-cols-4 gap-5"
          : "grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-5"
      }
    >
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
          onContextMenu={onContextMenu ? (e) => onContextMenu(e, pl) : undefined}
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
            {/* Three-dot button */}
            <button
              className="absolute top-2 right-2 w-8 h-8 bg-black/60 rounded-full flex items-center justify-center shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-200 hover:bg-black/80 z-10"
              title="More options"
              onClick={(e) => {
                e.stopPropagation();
                onContextMenu?.(e, pl);
              }}
            >
              <MoreHorizontal size={16} className="text-white" />
            </button>
            <div className="absolute bottom-2 right-2 w-10 h-10 bg-[#00FFFF] rounded-full flex items-center justify-center shadow-xl opacity-0 group-hover:opacity-100 translate-y-2 group-hover:translate-y-0 transition-[opacity,transform] duration-300 scale-90 group-hover:scale-100">
              <Play size={20} fill="black" className="text-black ml-1" />
            </div>
          </div>
          <h4 className="font-bold text-[15px] text-white truncate mb-1">
            {pl.title}
          </h4>
          <p className="text-[13px] text-[#a6a6a6] line-clamp-1">
            {pl.description || (pl.creator?.name ? `By ${pl.creator.name}` : pl.creator?.id === 0 ? "By TIDAL" : "Playlist")}
          </p>
          {pl.numberOfTracks != null && (
            <p className="text-[12px] text-[#666] mt-0.5">
              {pl.numberOfTracks} tracks
            </p>
          )}
        </div>
      ))}
    </div>
  );
}
