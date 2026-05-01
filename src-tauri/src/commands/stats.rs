use tauri::State;

use crate::stats::{
    DailyMinutes, DiscoveryPoint, HeatmapCell, HourMinutes, StatsOverview, StatsWindow, TopAlbum,
    TopArtist, TopTrack,
};
use crate::{AppState, SoneError};

fn map_err<T>(r: rusqlite::Result<T>) -> Result<T, SoneError> {
    r.map_err(|e| SoneError::Scrobble(format!("stats db: {e}")))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_stats_overview(
    state: State<'_, AppState>,
    window: StatsWindow,
) -> Result<StatsOverview, SoneError> {
    map_err(state.stats.overview(window))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_top_tracks(
    state: State<'_, AppState>,
    window: StatsWindow,
    limit: u32,
) -> Result<Vec<TopTrack>, SoneError> {
    map_err(state.stats.top_tracks(window, limit))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_top_artists(
    state: State<'_, AppState>,
    window: StatsWindow,
    limit: u32,
) -> Result<Vec<TopArtist>, SoneError> {
    map_err(state.stats.top_artists(window, limit))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_top_albums(
    state: State<'_, AppState>,
    window: StatsWindow,
    limit: u32,
) -> Result<Vec<TopAlbum>, SoneError> {
    map_err(state.stats.top_albums(window, limit))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_listening_heatmap(
    state: State<'_, AppState>,
    window: StatsWindow,
) -> Result<Vec<HeatmapCell>, SoneError> {
    map_err(state.stats.heatmap(window))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_daily_minutes(
    state: State<'_, AppState>,
    window: StatsWindow,
) -> Result<Vec<DailyMinutes>, SoneError> {
    map_err(state.stats.daily_minutes(window))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_hour_minutes(
    state: State<'_, AppState>,
    window: StatsWindow,
) -> Result<Vec<HourMinutes>, SoneError> {
    map_err(state.stats.hour_minutes(window))
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_discovery_curve(
    state: State<'_, AppState>,
    window: StatsWindow,
) -> Result<Vec<DiscoveryPoint>, SoneError> {
    map_err(state.stats.discovery_curve(window))
}
