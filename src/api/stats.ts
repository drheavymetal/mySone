import { invoke } from "@tauri-apps/api/core";

export type StatsWindow = "day" | "week" | "month" | "year" | "all";

export interface StatsOverview {
  totalPlays: number;
  completedPlays: number;
  totalListenedSecs: number;
  distinctTracks: number;
  distinctArtists: number;
  distinctAlbums: number;
  sinceUnix: number;
}

export interface TopTrack {
  trackId: number | null;
  title: string;
  artist: string;
  album: string | null;
  plays: number;
  listenedSecs: number;
}

export interface TopArtist {
  artist: string;
  plays: number;
  listenedSecs: number;
  distinctTracks: number;
}

export interface TopAlbum {
  album: string;
  artist: string;
  plays: number;
  listenedSecs: number;
}

export interface HeatmapCell {
  /** SQLite strftime('%w'): 0=Sunday … 6=Saturday */
  dow: number;
  hour: number;
  plays: number;
  listenedSecs: number;
}

export interface DailyMinutes {
  /** YYYY-MM-DD in local time */
  date: string;
  minutes: number;
}

export interface HourMinutes {
  /** 0–23 in local time. */
  hour: number;
  minutes: number;
}

export interface DiscoveryPoint {
  /** YYYY-MM-DD in local time */
  date: string;
  /** Artists heard for the very first time (across the whole local DB)
   *  on this day. */
  newArtists: number;
  /** Tracks heard for the very first time (across the whole local DB)
   *  on this day. */
  newTracks: number;
}

export async function getStatsOverview(
  window: StatsWindow,
): Promise<StatsOverview> {
  return invoke<StatsOverview>("get_stats_overview", { window });
}

export async function getTopTracks(
  window: StatsWindow,
  limit: number,
): Promise<TopTrack[]> {
  return invoke<TopTrack[]>("get_top_tracks", { window, limit });
}

export async function getTopArtists(
  window: StatsWindow,
  limit: number,
): Promise<TopArtist[]> {
  return invoke<TopArtist[]>("get_top_artists", { window, limit });
}

export async function getTopAlbums(
  window: StatsWindow,
  limit: number,
): Promise<TopAlbum[]> {
  return invoke<TopAlbum[]>("get_top_albums", { window, limit });
}

export async function getListeningHeatmap(
  window: StatsWindow,
): Promise<HeatmapCell[]> {
  return invoke<HeatmapCell[]>("get_listening_heatmap", { window });
}

export async function getDailyMinutes(
  window: StatsWindow,
): Promise<DailyMinutes[]> {
  return invoke<DailyMinutes[]>("get_daily_minutes", { window });
}

export async function getHourMinutes(
  window: StatsWindow,
): Promise<HourMinutes[]> {
  return invoke<HourMinutes[]>("get_hour_minutes", { window });
}

export async function getDiscoveryCurve(
  window: StatsWindow,
): Promise<DiscoveryPoint[]> {
  return invoke<DiscoveryPoint[]>("get_discovery_curve", { window });
}

// ─── History import (shared shape: ListenBrainz + Last.fm) ────────────────

export interface ImportResult {
  imported: number;
  skipped: number;
  pages: number;
  username: string;
}

export interface ImportProgress {
  page: number;
  imported: number;
  skipped: number;
  oldestTs: number;
  /** Only present for Last.fm — total pages reported by the API. */
  totalPages?: number;
}

/** Backwards-compat aliases — the existing modal still imports these. */
export type LbImportResult = ImportResult;
export type LbImportProgress = ImportProgress;

/** Trigger a ListenBrainz history backfill. Streams progress via the
 * `import-listenbrainz-progress` event. The user must already be
 * connected to ListenBrainz. */
export async function importListenBrainzHistory(
  sinceUnix?: number,
): Promise<ImportResult> {
  return invoke<ImportResult>("import_listenbrainz_history", {
    sinceUnix: sinceUnix ?? null,
  });
}

/** Trigger a Last.fm history backfill. Streams progress via the
 * `import-lastfm-progress` event. The user must already be connected
 * to Last.fm; the import uses the embedded API key + their public
 * username (no session token needed). */
export async function importLastfmHistory(
  sinceUnix?: number,
): Promise<ImportResult> {
  return invoke<ImportResult>("import_lastfm_history", {
    sinceUnix: sinceUnix ?? null,
  });
}
