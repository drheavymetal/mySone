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

// ─── ListenBrainz history import ──────────────────────────────────────────

export interface LbImportResult {
  imported: number;
  skipped: number;
  pages: number;
  username: string;
}

export interface LbImportProgress {
  page: number;
  imported: number;
  skipped: number;
  oldestTs: number;
}

/** Trigger a ListenBrainz history backfill. Streams progress via the
 * `import-listenbrainz-progress` event. The user must already be
 * connected to ListenBrainz. */
export async function importListenBrainzHistory(
  sinceUnix?: number,
): Promise<LbImportResult> {
  return invoke<LbImportResult>("import_listenbrainz_history", {
    sinceUnix: sinceUnix ?? null,
  });
}

// ─── Remote stats sources (ListenBrainz + Last.fm) ────────────────────────

export type StatsSource = "local" | "listenbrainz" | "lastfm";

/** Top tracks from ListenBrainz for the connected user. */
export async function getListenBrainzTopTracks(
  window: StatsWindow,
  limit: number,
): Promise<TopTrack[]> {
  return invoke<TopTrack[]>("get_listenbrainz_top_tracks", { window, limit });
}

export async function getListenBrainzTopArtists(
  window: StatsWindow,
  limit: number,
): Promise<TopArtist[]> {
  return invoke<TopArtist[]>("get_listenbrainz_top_artists", { window, limit });
}

export async function getListenBrainzTopAlbums(
  window: StatsWindow,
  limit: number,
): Promise<TopAlbum[]> {
  return invoke<TopAlbum[]>("get_listenbrainz_top_albums", { window, limit });
}

/** Top tracks from Last.fm for the connected user. */
export async function getLastfmUserTopTracks(
  window: StatsWindow,
  limit: number,
): Promise<TopTrack[]> {
  return invoke<TopTrack[]>("get_lastfm_user_top_tracks", { window, limit });
}

export async function getLastfmUserTopArtists(
  window: StatsWindow,
  limit: number,
): Promise<TopArtist[]> {
  return invoke<TopArtist[]>("get_lastfm_user_top_artists", { window, limit });
}

export async function getLastfmUserTopAlbums(
  window: StatsWindow,
  limit: number,
): Promise<TopAlbum[]> {
  return invoke<TopAlbum[]>("get_lastfm_user_top_albums", { window, limit });
}
