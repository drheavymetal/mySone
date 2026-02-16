import { invoke } from "@tauri-apps/api/core";
import type {
  AlbumDetail,
  ArtistDetail,
  Credit,
  HomePageCached,
  HomePageResponse,
  Lyrics,
  MediaItemType,
  PaginatedTracks,
  SearchResults,
  SuggestionsResponse,
  Track,
} from "../types";

// ==================== In-memory cache ====================

const store = new Map<string, { data: unknown; ts: number }>();

const TTL = {
  SHORT: 2 * 60_000,        // 2 min  — search, suggestions
  LONG: 2 * 60 * 60_000,    // 2 hrs  — playlists, favorites, mixes, page sections
  STATIC: 24 * 60 * 60_000, // 24 hrs — albums, artists, lyrics, credits, bios
};

function cached<T>(key: string, fetcher: () => Promise<T>, ttl: number): Promise<T> {
  const entry = store.get(key);
  if (entry && Date.now() - entry.ts < ttl) {
    return Promise.resolve(entry.data as T);
  }
  return fetcher().then((data) => {
    store.set(key, { data, ts: Date.now() });
    return data;
  });
}

/** Remove all cache entries whose key starts with the given prefix. */
export function invalidateCache(prefix: string): void {
  for (const key of store.keys()) {
    if (key.startsWith(prefix)) store.delete(key);
  }
}

/** Mutate a cached entry in-place. If the key exists, updater receives the data and the result replaces it. */
function mutateCache<T>(keyPrefix: string, updater: (data: T) => T): void {
  for (const [key, entry] of store.entries()) {
    if (key.startsWith(keyPrefix)) {
      store.set(key, { data: updater(entry.data as T), ts: entry.ts });
    }
  }
}

/** Optimistically prepend a track to all cached favorite-track pages. */
export function addTrackToFavoritesCache(userId: number, track: Track): void {
  mutateCache<PaginatedTracks>(`fav-tracks:${userId}:`, (page) => ({
    ...page,
    items: [track, ...page.items],
    totalNumberOfItems: page.totalNumberOfItems + 1,
  }));
}

/** Optimistically remove a track from all cached favorite-track pages. */
export function removeTrackFromFavoritesCache(userId: number, trackId: number): void {
  mutateCache<PaginatedTracks>(`fav-tracks:${userId}:`, (page) => ({
    ...page,
    items: page.items.filter((t) => t.id !== trackId),
    totalNumberOfItems: Math.max(0, page.totalNumberOfItems - 1),
  }));
}

/** Drop the entire cache (e.g. on logout). */
export function clearCache(): void {
  store.clear();
}

// ==================== Search ====================

export async function searchTidal(
  query: string,
  limit: number = 20
): Promise<SearchResults> {
  return cached(`search:${query}:${limit}`, async () => {
    try {
      return await invoke<SearchResults>("search_tidal", { query, limit });
    } catch (error: any) {
      console.error("Failed to search:", error);
      throw error;
    }
  }, TTL.SHORT);
}

export async function getSuggestions(
  query: string,
  limit: number = 10
): Promise<SuggestionsResponse> {
  return cached(`suggest:${query}:${limit}`, async () => {
    try {
      return await invoke<SuggestionsResponse>("get_suggestions", {
        query,
        limit,
      });
    } catch {
      return { textSuggestions: [], directHits: [] };
    }
  }, TTL.SHORT);
}

// ==================== Home Page ====================

export async function getHomePage(): Promise<HomePageCached> {
  return cached("home-page", () =>
    invoke<HomePageCached>("get_home_page"),
  TTL.LONG);
}

export async function refreshHomePage(): Promise<HomePageResponse> {
  return await invoke<HomePageResponse>("refresh_home_page");
}

export async function getPageSection(
  apiPath: string
): Promise<HomePageResponse> {
  return cached(`section:${apiPath}`, () =>
    invoke<HomePageResponse>("get_page_section", { apiPath }),
  TTL.LONG);
}

// ==================== Album ====================

export async function getAlbumDetail(albumId: number): Promise<AlbumDetail> {
  return cached(`album:${albumId}`, async () => {
    try {
      return await invoke<AlbumDetail>("get_album_detail", { albumId });
    } catch (error: any) {
      console.error("Failed to get album detail:", error);
      throw error;
    }
  }, TTL.STATIC);
}

export async function getAlbumTracks(
  albumId: number,
  offset: number = 0,
  limit: number = 50
): Promise<PaginatedTracks> {
  return cached(`album-tracks:${albumId}:${offset}:${limit}`, async () => {
    try {
      return await invoke<PaginatedTracks>("get_album_tracks", {
        albumId,
        offset,
        limit,
      });
    } catch (error: any) {
      console.error("Failed to get album tracks:", error);
      throw error;
    }
  }, TTL.STATIC);
}

// ==================== Artist ====================

export async function getArtistDetail(
  artistId: number
): Promise<ArtistDetail> {
  return cached(`artist:${artistId}`, () =>
    invoke<ArtistDetail>("get_artist_detail", { artistId }),
  TTL.STATIC);
}

export async function getArtistTopTracks(
  artistId: number,
  limit: number = 20
): Promise<Track[]> {
  return cached(`artist-tracks:${artistId}:${limit}`, () =>
    invoke<Track[]>("get_artist_top_tracks", { artistId, limit }),
  TTL.STATIC);
}

export async function getArtistAlbums(
  artistId: number,
  limit: number = 20
): Promise<AlbumDetail[]> {
  return cached(`artist-albums:${artistId}:${limit}`, () =>
    invoke<AlbumDetail[]>("get_artist_albums", { artistId, limit }),
  TTL.STATIC);
}

export async function getArtistBio(artistId: number): Promise<string> {
  return cached(`artist-bio:${artistId}`, () =>
    invoke<string>("get_artist_bio", { artistId }),
  TTL.STATIC);
}

// ==================== Playlist / Mix ====================

export async function getPlaylistTracks(
  playlistId: string
): Promise<Track[]> {
  return cached(`playlist:${playlistId}`, async () => {
    try {
      const tracks = await invoke<Track[]>("get_playlist_tracks", {
        playlistId: playlistId,
      });
      return tracks || [];
    } catch (error: any) {
      console.error("Failed to get playlist tracks:", error);
      throw error;
    }
  }, TTL.LONG);
}

export async function getPlaylistTracksPage(
  playlistId: string,
  offset: number = 0,
  limit: number = 50
): Promise<PaginatedTracks> {
  return cached(`playlist-page:${playlistId}:${offset}:${limit}`, async () => {
    try {
      return await invoke<PaginatedTracks>("get_playlist_tracks_page", {
        playlistId,
        offset,
        limit,
      });
    } catch (error: any) {
      console.error("Failed to get playlist tracks page:", error);
      throw error;
    }
  }, TTL.LONG);
}

export async function getMixItems(mixId: string): Promise<Track[]> {
  return cached(`mix:${mixId}`, () =>
    invoke<Track[]>("get_mix_items", { mixId }),
  TTL.LONG);
}

/** Fetch all tracks from a media item (album / playlist / mix) */
export async function fetchMediaTracks(
  item: MediaItemType
): Promise<Track[]> {
  switch (item.type) {
    case "album": {
      const result = await getAlbumTracks(item.id, 0, 200);
      return result.items;
    }
    case "playlist": {
      return await getPlaylistTracks(item.uuid);
    }
    case "mix": {
      return await getMixItems(item.mixId);
    }
  }
}

// ==================== Track metadata ====================

export async function getTrackLyrics(trackId: number): Promise<Lyrics> {
  return cached(`lyrics:${trackId}`, async () => {
    try {
      return await invoke<Lyrics>("get_track_lyrics", { trackId });
    } catch (error: any) {
      console.error("Failed to get lyrics:", error);
      throw error;
    }
  }, TTL.STATIC);
}

export async function getTrackCredits(trackId: number): Promise<Credit[]> {
  return cached(`credits:${trackId}`, async () => {
    try {
      return await invoke<Credit[]>("get_track_credits", { trackId });
    } catch (error: any) {
      console.error("Failed to get credits:", error);
      throw error;
    }
  }, TTL.STATIC);
}

export async function getTrackRadio(
  trackId: number,
  limit: number = 20
): Promise<Track[]> {
  try {
    return await invoke<Track[]>("get_track_radio", { trackId, limit });
  } catch (error: any) {
    console.error("Failed to get track radio:", error);
    throw error;
  }
}

// ==================== Favorites (parameterised by userId) ====================

export async function getFavoriteTracks(
  userId: number,
  offset: number = 0,
  limit: number = 50
): Promise<PaginatedTracks> {
  return cached(`fav-tracks:${userId}:${offset}:${limit}`, async () => {
    try {
      return await invoke<PaginatedTracks>("get_favorite_tracks", {
        userId,
        offset,
        limit,
      });
    } catch (error: any) {
      console.error("Failed to get favorite tracks:", error);
      throw error;
    }
  }, TTL.LONG);
}

export async function getFavoriteArtists(
  userId: number,
  limit: number = 20
): Promise<ArtistDetail[]> {
  return cached(`fav-artists:${userId}:${limit}`, () =>
    invoke<ArtistDetail[]>("get_favorite_artists", { userId, limit }),
  TTL.LONG);
}

export async function getFavoriteAlbums(
  userId: number,
  limit: number = 50
): Promise<AlbumDetail[]> {
  return cached(`fav-albums:${userId}:${limit}`, () =>
    invoke<AlbumDetail[]>("get_favorite_albums", { userId, limit }),
  TTL.LONG);
}

// ==================== Auth helpers (never cached) ====================

export async function getSavedCredentials(): Promise<{
  clientId: string;
  clientSecret: string;
}> {
  try {
    const [clientId, clientSecret] = await invoke<[string, string]>(
      "get_saved_credentials"
    );
    return { clientId, clientSecret };
  } catch (error) {
    console.error("Failed to get saved credentials:", error);
    return { clientId: "", clientSecret: "" };
  }
}

export async function parseTokenData(
  rawText: string
): Promise<{
  clientId?: string;
  clientSecret?: string;
  refreshToken?: string;
  accessToken?: string;
}> {
  return await invoke("parse_token_data", { rawText });
}

// ==================== Playback queue persistence ====================

export async function savePlaybackQueue(snapshotJson: string): Promise<void> {
  return invoke("save_playback_queue", { snapshotJson });
}

export async function loadPlaybackQueue(): Promise<string | null> {
  return invoke("load_playback_queue");
}
