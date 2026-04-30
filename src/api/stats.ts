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
