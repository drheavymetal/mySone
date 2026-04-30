use tauri::State;

use crate::share_link::{ShareNowState, ShareStatus};
use crate::AppState;
use crate::SoneError;

#[tauri::command]
pub async fn share_start(state: State<'_, AppState>) -> Result<ShareStatus, SoneError> {
    log::info!("[share_start]");
    state
        .share_link
        .start_sharing()
        .map_err(SoneError::Audio)
}

#[tauri::command]
pub async fn share_stop(state: State<'_, AppState>) -> Result<ShareStatus, SoneError> {
    log::info!("[share_stop]");
    state.share_link.stop_sharing().map_err(SoneError::Audio)
}

#[tauri::command]
pub async fn share_status(state: State<'_, AppState>) -> Result<ShareStatus, SoneError> {
    Ok(state.share_link.status())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn share_set_state(
    state: State<'_, AppState>,
    now_state: ShareNowState,
) -> Result<(), SoneError> {
    state
        .share_link
        .set_now_state(now_state)
        .map_err(SoneError::Audio)
}
