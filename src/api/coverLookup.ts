/**
 * Cover / artist-picture lookup with localStorage cache.
 *
 * Stats rows only carry names (and sometimes a track_id) — TIDAL covers
 * and artist photos are resolved on demand by hitting `search_tidal`
 * (and `get_track` when we already have an id), then cached so the
 * Stats page only pays the network cost once per name.
 *
 * Design choices:
 *  - localStorage cache with TTLs (positive 30d, negative 7d).
 *  - In-memory mirror so reads don't hit JSON.parse on every avatar.
 *  - Concurrency-limited launcher so a 50-row tab doesn't fan out 50
 *    parallel searches.
 *  - Negative results cached so we don't keep retrying tracks with
 *    no matching album cover.
 */
import { invoke } from "@tauri-apps/api/core";
import type { SearchResults } from "../types";

const STORAGE_KEY = "sone:stats-cover-cache:v1";
const POS_TTL_MS = 30 * 24 * 3600 * 1000;
const NEG_TTL_MS = 7 * 24 * 3600 * 1000;
const MAX_ACTIVE = 4;

interface CacheEntry {
  /** Raw TIDAL UUID (cover or picture). null = lookup miss. */
  raw: string | null;
  /** Unix ms when stored. */
  ts: number;
}

type Kind = "track" | "album" | "artist";

let mem: Map<string, CacheEntry> | null = null;
const inflight = new Map<string, Promise<string | null>>();

function loadMem(): Map<string, CacheEntry> {
  if (mem) return mem;
  mem = new Map();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const obj = JSON.parse(raw) as Record<string, CacheEntry>;
      for (const [k, v] of Object.entries(obj)) mem.set(k, v);
    }
  } catch {
    // ignore — start with empty cache
  }
  return mem;
}

let pendingPersist: number | null = null;
function schedulePersist() {
  if (pendingPersist != null || typeof window === "undefined") return;
  pendingPersist = window.setTimeout(() => {
    pendingPersist = null;
    if (!mem) return;
    try {
      const obj: Record<string, CacheEntry> = {};
      for (const [k, v] of mem.entries()) obj[k] = v;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(obj));
    } catch {
      // quota or serialization — ignore, mem cache still holds
    }
  }, 500);
}

function normalize(s: string): string {
  return s.toLowerCase().trim().replace(/\s+/g, " ");
}

function keyOf(kind: Kind, ...parts: string[]): string {
  return `${kind}:${parts.map(normalize).join("|")}`;
}

function readCache(k: string): CacheEntry | undefined {
  const m = loadMem();
  const e = m.get(k);
  if (!e) return undefined;
  const ttl = e.raw ? POS_TTL_MS : NEG_TTL_MS;
  if (Date.now() - e.ts > ttl) {
    m.delete(k);
    return undefined;
  }
  return e;
}

function writeCache(k: string, raw: string | null) {
  loadMem().set(k, { raw, ts: Date.now() });
  schedulePersist();
}

const waiting: (() => void)[] = [];
let active = 0;
function schedule<T>(fn: () => Promise<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    const run = () => {
      active++;
      fn()
        .then(resolve, reject)
        .finally(() => {
          active--;
          const next = waiting.shift();
          if (next) next();
        });
    };
    if (active < MAX_ACTIVE) run();
    else waiting.push(run);
  });
}

interface MaybeAlbum {
  cover?: string;
}
interface MaybeTrackJson {
  album?: MaybeAlbum;
}

async function searchOnce(
  query: string,
  kind: Kind,
): Promise<string | null> {
  try {
    const res = await invoke<SearchResults>("search_tidal", {
      query,
      limit: 5,
    });
    if (kind === "artist") return res.artists[0]?.picture ?? null;
    if (kind === "album") return res.albums[0]?.cover ?? null;
    return res.tracks[0]?.album?.cover ?? null;
  } catch {
    return null;
  }
}

async function trackCoverByIdOrSearch(
  trackId: number | null,
  title: string,
  artist: string,
): Promise<string | null> {
  if (trackId) {
    try {
      const t = await invoke<MaybeTrackJson>("get_track", { trackId });
      const c = t?.album?.cover;
      if (c) return c;
    } catch {
      // fall through to name search
    }
  }
  return searchOnce(`${title} ${artist}`, "track");
}

function dedupe(
  k: string,
  fn: () => Promise<string | null>,
): Promise<string | null> {
  const existing = inflight.get(k);
  if (existing) return existing;
  const p = schedule(fn).then((raw) => {
    writeCache(k, raw);
    inflight.delete(k);
    return raw;
  });
  inflight.set(k, p);
  return p;
}

export async function getTrackCover(
  trackId: number | null,
  title: string,
  artist: string,
): Promise<string | null> {
  const k = keyOf("track", trackId ? `id:${trackId}` : `${title}|${artist}`);
  const cached = readCache(k);
  if (cached) return cached.raw;
  return dedupe(k, () => trackCoverByIdOrSearch(trackId, title, artist));
}

export async function getAlbumCover(
  album: string,
  artist: string,
): Promise<string | null> {
  const k = keyOf("album", album, artist);
  const cached = readCache(k);
  if (cached) return cached.raw;
  return dedupe(k, () => searchOnce(`${album} ${artist}`, "album"));
}

export async function getArtistPicture(artist: string): Promise<string | null> {
  const k = keyOf("artist", artist);
  const cached = readCache(k);
  if (cached) return cached.raw;
  return dedupe(k, () => searchOnce(artist, "artist"));
}
