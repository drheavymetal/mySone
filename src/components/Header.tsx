import {
  ChevronLeft,
  ChevronRight,
  Search,
  X,
  Loader2,
  MoreHorizontal,
  Clock,
} from "lucide-react";
import { useState, useRef, useCallback, useEffect } from "react";
import { useAudioContext } from "../contexts/AudioContext";
import { getTidalImageUrl, type SearchResults, type Track } from "../hooks/useAudio";
import TidalImage from "./TidalImage";
import TrackContextMenu from "./TrackContextMenu";
import UserMenu from "./UserMenu";

const HISTORY_KEY = "tide-vibe-search-history";
const MAX_HISTORY = 10;

function loadHistory(): string[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed.filter((s) => typeof s === "string").slice(0, MAX_HISTORY);
    }
  } catch {}
  return [];
}

function saveHistory(history: string[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history.slice(0, MAX_HISTORY)));
  } catch {}
}

export default function Header() {
  const {
    playTrack,
    setQueueTracks,
    navigateToAlbum,
    navigateToArtist,
    navigateToSearch,
    searchTidal,
    searchSuggestions,
    currentView,
  } = useAudioContext();

  const [searchQuery, setSearchQuery] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [quickResults, setQuickResults] = useState<SearchResults | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchHistory, setSearchHistory] = useState<string[]>(loadHistory);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const suggestDebounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Track context menu state
  const [ctxTrack, setCtxTrack] = useState<Track | null>(null);
  const [ctxTrackIndex, setCtxTrackIndex] = useState(0);
  const [ctxPos, setCtxPos] = useState<{ x: number; y: number } | undefined>(undefined);
  const dotsRefs = useRef<Map<number, HTMLButtonElement>>(new Map());

  // Sync search query with current view if it's a search view
  useEffect(() => {
    if (currentView.type === "search") {
      setSearchQuery(currentView.query);
    }
  }, [currentView]);

  // Debounced quick search
  const doQuickSearch = useCallback(
    (query: string) => {
      clearTimeout(debounceRef.current);
      clearTimeout(suggestDebounceRef.current);
      if (!query.trim()) {
        setQuickResults(null);
        setSearching(false);
        setSuggestions([]);
        return;
      }
      setSearching(true);
      debounceRef.current = setTimeout(() => {
        searchTidal(query.trim(), 5)
          .then((results) => {
            setQuickResults(results);
          })
          .catch(() => {
            setQuickResults(null);
          })
          .finally(() => {
            setSearching(false);
          });
      }, 300);
      // Fetch search suggestions with shorter debounce
      suggestDebounceRef.current = setTimeout(() => {
        searchSuggestions(query.trim(), 5)
          .then((s) => setSuggestions(s))
          .catch(() => setSuggestions([]));
      }, 200);
    },
    [searchTidal, searchSuggestions]
  );

  const addToHistory = useCallback((query: string) => {
    const trimmed = query.trim();
    if (!trimmed) return;
    setSearchHistory((prev) => {
      const filtered = prev.filter((h) => h.toLowerCase() !== trimmed.toLowerCase());
      const next = [trimmed, ...filtered].slice(0, MAX_HISTORY);
      saveHistory(next);
      return next;
    });
  }, []);

  const removeFromHistory = useCallback((query: string) => {
    setSearchHistory((prev) => {
      const next = prev.filter((h) => h !== query);
      saveHistory(next);
      return next;
    });
  }, []);

  const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setSearchQuery(val);
    setSearchOpen(true);
    doQuickSearch(val);
  };

  const handleSearchKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && searchQuery.trim()) {
      setSearchOpen(false);
      addToHistory(searchQuery.trim());
      navigateToSearch(searchQuery.trim());
    } else if (e.key === "Escape") {
      setSearchOpen(false);
      searchInputRef.current?.blur();
    }
  };

  const clearSearch = () => {
    setSearchQuery("");
    setSearchOpen(false);
    setQuickResults(null);
    setSuggestions([]);
    searchInputRef.current?.focus();
  };

  // Close dropdown when clicking outside
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(e.target as Node) &&
        searchInputRef.current &&
        !searchInputRef.current.contains(e.target as Node)
      ) {
        setSearchOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const hasResults =
    quickResults &&
    (quickResults.tracks.length > 0 ||
      quickResults.albums.length > 0 ||
      quickResults.artists.length > 0);

  const getHeaderTitle = () => {
    if (currentView.type === "search") {
      return `Results for "${currentView.query}"`;
    }
    return "";
  };

  // History suggestions: filter by typed prefix
  const matchingHistory = searchQuery.trim()
    ? searchHistory.filter(
        (h) =>
          h.toLowerCase().includes(searchQuery.trim().toLowerCase()) &&
          h.toLowerCase() !== searchQuery.trim().toLowerCase()
      )
    : searchHistory;

  const showHistorySection = searchOpen && matchingHistory.length > 0;
  const showSuggestions = searchOpen && searchQuery.trim() && suggestions.length > 0;
  const showResultsSection = searchOpen && searchQuery.trim() && (searching || hasResults || quickResults);

  return (
    <div className="h-16 flex items-center justify-between px-6 bg-[#121212] z-30 shrink-0 sticky top-0">
      <div className="flex items-center gap-4 flex-1 min-w-0">
        <div className="flex items-center gap-2 shrink-0">
          <button
            onClick={() => window.history.back()}
            className="w-8 h-8 rounded-full bg-black/40 flex items-center justify-center text-[#a6a6a6] hover:text-white transition-colors disabled:opacity-50"
          >
            <ChevronLeft size={20} />
          </button>
          <button
            onClick={() => window.history.forward()}
            className="w-8 h-8 rounded-full bg-black/40 flex items-center justify-center text-[#a6a6a6] hover:text-white transition-colors disabled:opacity-50"
          >
            <ChevronRight size={20} />
          </button>
        </div>

        {/* Dynamic Title */}
        <h1 className="text-[18px] font-bold text-white truncate ml-2">
          {getHeaderTitle()}
        </h1>
      </div>

      <div className="flex items-center gap-4">
        {/* Search Input */}
        <div className="relative max-w-[360px] w-64 lg:w-80">
          <div className="flex items-center gap-2 px-3 py-2 bg-[#242424] hover:bg-[#2a2a2a] focus-within:bg-[#2a2a2a] rounded-full transition-colors group border border-transparent focus-within:border-white/10">
            <Search
              size={18}
              className="text-[#b3b3b3] group-focus-within:text-white shrink-0"
            />
            <input
              ref={searchInputRef}
              type="text"
              value={searchQuery}
              onChange={handleSearchChange}
              onKeyDown={handleSearchKeyDown}
              onFocus={() => setSearchOpen(true)}
              placeholder="Search"
              className="bg-transparent text-sm text-white placeholder-[#808080] outline-none flex-1 min-w-0"
            />
            {searchQuery && (
              <button
                onClick={clearSearch}
                className="text-[#808080] hover:text-white shrink-0"
              >
                <X size={16} />
              </button>
            )}
          </div>

          {/* Search Dropdown */}
          {searchOpen && (showHistorySection || showSuggestions || showResultsSection) && (
            <div
              ref={dropdownRef}
              className="absolute right-0 top-full mt-2 w-[420px] bg-[#1a1a1a] rounded-lg shadow-2xl shadow-black/60 border border-white/8 z-50 max-h-[70vh] overflow-y-auto scrollbar-thin scrollbar-thumb-[#333] scrollbar-track-transparent"
            >
              {/* Server-side search suggestions */}
              {showSuggestions && (
                <div className="py-1">
                  {suggestions.map((s, i) => (
                    <button
                      key={`sug-${i}`}
                      className="w-full flex items-center gap-3 px-3 py-2.5 hover:bg-white/6 transition-colors text-left"
                      onClick={() => {
                        setSearchQuery(s);
                        setSuggestions([]);
                        setSearchOpen(false);
                        addToHistory(s);
                        navigateToSearch(s);
                      }}
                    >
                      <Search size={15} className="text-[#666] shrink-0" />
                      <span className="text-[13px] text-white truncate">{s}</span>
                    </button>
                  ))}
                  {(showHistorySection || showResultsSection) && (
                    <div className="border-b border-white/6 mx-3" />
                  )}
                </div>
              )}

              {/* Search History */}
              {showHistorySection && (
                <div className="py-1">
                  {!searchQuery.trim() && (
                    <div className="px-3 pt-2 pb-1 text-[11px] text-[#666] uppercase tracking-wider font-medium">
                      Recent searches
                    </div>
                  )}
                  {matchingHistory.slice(0, 5).map((item) => (
                    <div
                      key={item}
                      className="flex items-center gap-3 px-3 py-3 hover:bg-white/6 transition-colors cursor-pointer"
                    >
                      <Clock size={15} className="text-[#666] shrink-0" />
                      <button
                        className="flex-1 text-left text-[13px] text-white truncate"
                        onClick={() => {
                          setSearchQuery(item);
                          setSearchOpen(false);
                          addToHistory(item);
                          navigateToSearch(item);
                        }}
                      >
                        {item}
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          removeFromHistory(item);
                        }}
                        className="text-[#666] hover:text-white shrink-0 p-0.5"
                        title="Remove"
                      >
                        <X size={14} />
                      </button>
                    </div>
                  ))}
                  {showResultsSection && (
                    <div className="border-b border-white/6 mx-3" />
                  )}
                </div>
              )}

              {/* Quick Results */}
              {showResultsSection && (
                <>
                  {searching && !hasResults && (
                    <div className="flex items-center justify-center py-6">
                      <Loader2 size={18} className="animate-spin text-[#00FFFF]" />
                    </div>
                  )}

                  {!searching && !hasResults && quickResults && (
                    <div className="py-6 text-center text-[13px] text-[#666]">
                      No results found
                    </div>
                  )}

                  {hasResults && (
                    <div className="py-1">
                      {/* Render sections ordered by topHitType for relevance */}
                      {(() => {
                        const topHit = quickResults!.topHitType?.toUpperCase();
                        type SectionType = "tracks" | "albums" | "artists";
                        const defaultOrder: SectionType[] = ["tracks", "albums", "artists"];
                        const order = topHit === "ARTISTS"
                          ? (["artists", "tracks", "albums"] as SectionType[])
                          : topHit === "ALBUMS"
                            ? (["albums", "tracks", "artists"] as SectionType[])
                            : defaultOrder;

                        return order.map((section) => {
                          if (section === "tracks" && quickResults!.tracks.length > 0) {
                            return quickResults!.tracks.map((track, idx) => (
                              <div
                                key={`t-${track.id}`}
                                className="flex items-center gap-3 px-3 py-3 hover:bg-white/6 transition-colors text-left group/track"
                                onContextMenu={(e) => {
                                  e.preventDefault();
                                  e.stopPropagation();
                                  setCtxPos({ x: e.clientX, y: e.clientY });
                                  setCtxTrackIndex(idx);
                                  setCtxTrack(track);
                                }}
                              >
                                <button
                                  className="flex-1 flex items-center gap-3 min-w-0"
                                  onClick={() => {
                                    setSearchOpen(false);
                                    setQueueTracks([]);
                                    playTrack(track);
                                  }}
                                >
                                  <div className="w-12 h-12 rounded bg-[#282828] overflow-hidden shrink-0">
                                    <TidalImage
                                      src={getTidalImageUrl(track.album?.cover, 80)}
                                      alt={track.title}
                                      className="w-full h-full"
                                    />
                                  </div>
                                  <div className="flex-1 min-w-0 text-left">
                                    <p className="text-[14px] text-white truncate">
                                      {track.title}
                                    </p>
                                    <p className="text-[11px] text-[#808080] truncate">
                                      Track &middot;{" "}
                                      {track.artist?.name || "Unknown Artist"}
                                    </p>
                                  </div>
                                </button>
                                <button
                                  ref={(el) => {
                                    if (el) dotsRefs.current.set(track.id, el);
                                    else dotsRefs.current.delete(track.id);
                                  }}
                                  className="p-1 rounded-full text-[#666] hover:text-white opacity-0 group-hover/track:opacity-100 transition-opacity shrink-0"
                                  title="More options"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    setCtxPos(undefined);
                                    setCtxTrackIndex(idx);
                                    setCtxTrack((prev) => (prev?.id === track.id ? null : track));
                                  }}
                                >
                                  <MoreHorizontal size={16} />
                                </button>
                                {ctxTrack?.id === track.id && (
                                  <TrackContextMenu
                                    track={track}
                                    index={ctxTrackIndex}
                                    anchorRef={{ current: dotsRefs.current.get(track.id) ?? null }}
                                    cursorPosition={ctxPos}
                                    onClose={() => setCtxTrack(null)}
                                  />
                                )}
                              </div>
                            ));
                          }
                          if (section === "albums" && quickResults!.albums.length > 0) {
                            return quickResults!.albums.map((album) => (
                              <button
                                key={`a-${album.id}`}
                                onClick={() => {
                                  setSearchOpen(false);
                                  navigateToAlbum(album.id, {
                                    title: album.title,
                                    cover: album.cover,
                                    artistName: album.artist?.name,
                                  });
                                }}
                                className="w-full flex items-center gap-3 px-3 py-3 hover:bg-white/6 transition-colors text-left"
                              >
                                <div className="w-12 h-12 rounded bg-[#282828] overflow-hidden shrink-0">
                                  <TidalImage
                                    src={getTidalImageUrl(album.cover, 80)}
                                    alt={album.title}
                                    className="w-full h-full"
                                  />
                                </div>
                                <div className="flex-1 min-w-0">
                                  <p className="text-[14px] text-white truncate">
                                    {album.title}
                                  </p>
                                  <p className="text-[11px] text-[#808080] truncate">
                                    Album &middot; {album.artist?.name || "Unknown"}
                                  </p>
                                </div>
                              </button>
                            ));
                          }
                          if (section === "artists" && quickResults!.artists.length > 0) {
                            return quickResults!.artists.map((artist) => (
                              <button
                                key={`ar-${artist.id}`}
                                onClick={() => {
                                  setSearchOpen(false);
                                  navigateToArtist(artist.id, {
                                    name: artist.name,
                                    picture: artist.picture,
                                  });
                                }}
                                className="w-full flex items-center gap-3 px-3 py-3 hover:bg-white/6 transition-colors text-left"
                              >
                                <div className="w-12 h-12 rounded-full bg-[#282828] overflow-hidden shrink-0">
                                  {artist.picture ? (
                                    <TidalImage
                                      src={getTidalImageUrl(artist.picture, 80)}
                                      alt={artist.name}
                                      className="w-full h-full object-cover"
                                    />
                                  ) : (
                                    <div className="w-full h-full flex items-center justify-center">
                                      <span className="text-[12px] font-bold text-[#666]">
                                        {artist.name.charAt(0)}
                                      </span>
                                    </div>
                                  )}
                                </div>
                                <div className="flex-1 min-w-0">
                                  <p className="text-[14px] text-white truncate">
                                    {artist.name}
                                  </p>
                                  <p className="text-[11px] text-[#808080]">Artist</p>
                                </div>
                              </button>
                            ));
                          }
                          return null;
                        });
                      })()}

                      {/* View all */}
                      <button
                        onClick={() => {
                          setSearchOpen(false);
                          addToHistory(searchQuery.trim());
                          navigateToSearch(searchQuery.trim());
                        }}
                        className="w-full py-2.5 text-center text-[12px] font-semibold text-[#00FFFF] hover:bg-white/4 border-t border-white/6 transition-colors"
                      >
                        View all results
                      </button>
                    </div>
                  )}
                </>
              )}
            </div>
          )}
        </div>

        <UserMenu />
      </div>
    </div>
  );
}
