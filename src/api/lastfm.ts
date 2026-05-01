/**
 * Last.fm public-data wrappers (no auth required).
 *
 * Returns get cached in localStorage so re-opening the drawer for the
 * same track doesn't re-hit the API. TTL is 7 days for similars (the
 * graph evolves slowly) and 30 days for tags (mostly static).
 */
import { invoke } from "@tauri-apps/api/core";

const SIMILAR_TTL_MS = 7 * 24 * 3600 * 1000;
const TAG_TTL_MS = 30 * 24 * 3600 * 1000;
const STORAGE_KEY = "sone:lastfm-cache:v1";

interface CacheEntry<T> {
  data: T;
  ts: number;
}

let mem: Map<string, CacheEntry<unknown>> | null = null;

function loadMem(): Map<string, CacheEntry<unknown>> {
  if (mem) return mem;
  mem = new Map();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const obj = JSON.parse(raw) as Record<string, CacheEntry<unknown>>;
      for (const [k, v] of Object.entries(obj)) mem.set(k, v);
    }
  } catch {
    // ignore — start with empty cache
  }
  return mem;
}

let pending: number | null = null;
function schedulePersist() {
  if (pending != null || typeof window === "undefined") return;
  pending = window.setTimeout(() => {
    pending = null;
    if (!mem) return;
    try {
      const obj: Record<string, CacheEntry<unknown>> = {};
      for (const [k, v] of mem.entries()) obj[k] = v;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(obj));
    } catch {
      // quota — ignore, mem still holds
    }
  }, 500);
}

function read<T>(key: string, ttl: number): T | null {
  const e = loadMem().get(key);
  if (!e) return null;
  if (Date.now() - e.ts > ttl) {
    loadMem().delete(key);
    return null;
  }
  return e.data as T;
}

function write<T>(key: string, data: T) {
  loadMem().set(key, { data: data as unknown, ts: Date.now() });
  schedulePersist();
}

function norm(s: string): string {
  return s.toLowerCase().trim().replace(/\s+/g, " ");
}

// ─── Similar tracks ──────────────────────────────────────────────────────

export interface LfmSimilarTrack {
  name: string;
  artist: string;
  /** Match score in [0,1] from Last.fm's collaborative-filter graph. */
  matchScore: number;
  mbid?: string;
  url?: string;
  playcount?: number;
}

export async function getLastfmSimilarTracks(
  track: string,
  artist: string,
  limit = 25,
): Promise<LfmSimilarTrack[]> {
  const key = `similar:${norm(track)}|${norm(artist)}|${limit}`;
  const cached = read<LfmSimilarTrack[]>(key, SIMILAR_TTL_MS);
  if (cached) return cached;
  try {
    const data = await invoke<LfmSimilarTrack[]>("get_lastfm_similar_tracks", {
      track,
      artist,
      limit,
    });
    write(key, data);
    return data;
  } catch {
    return [];
  }
}

// ─── Tags ────────────────────────────────────────────────────────────────

export interface LfmTag {
  name: string;
  count: number;
  url?: string;
}

export async function getLastfmTrackTags(
  track: string,
  artist: string,
): Promise<LfmTag[]> {
  const key = `track-tags:${norm(track)}|${norm(artist)}`;
  const cached = read<LfmTag[]>(key, TAG_TTL_MS);
  if (cached) return cached;
  try {
    const data = await invoke<LfmTag[]>("get_lastfm_track_tags", {
      track,
      artist,
    });
    write(key, data);
    return data;
  } catch {
    return [];
  }
}

export async function getLastfmArtistTags(
  artist: string,
): Promise<LfmTag[]> {
  const key = `artist-tags:${norm(artist)}`;
  const cached = read<LfmTag[]>(key, TAG_TTL_MS);
  if (cached) return cached;
  try {
    const data = await invoke<LfmTag[]>("get_lastfm_artist_tags", {
      artist,
    });
    write(key, data);
    return data;
  } catch {
    return [];
  }
}
