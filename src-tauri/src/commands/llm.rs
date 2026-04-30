use tauri::State;

use crate::llm::{build_provider, ChatMessage, LLMSettings};
use crate::AppState;
use crate::SoneError;

#[tauri::command]
pub async fn get_llm_settings(state: State<'_, AppState>) -> Result<LLMSettings, SoneError> {
    let s = state.llm_settings.lock().await.clone();
    Ok(s)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn set_llm_settings(
    state: State<'_, AppState>,
    settings: LLMSettings,
) -> Result<(), SoneError> {
    *state.llm_settings.lock().await = settings.clone();
    let mut persisted = state.load_settings().unwrap_or_default();
    persisted.llm = settings;
    state.save_settings(&persisted)?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn llm_chat(
    state: State<'_, AppState>,
    system: String,
    messages: Vec<ChatMessage>,
) -> Result<String, SoneError> {
    let settings = state.llm_settings.lock().await.clone();
    let provider = build_provider(&settings)?;
    let resp = provider.chat_json(&system, &messages).await?;
    Ok(resp)
}

#[tauri::command]
pub async fn llm_ping(state: State<'_, AppState>) -> Result<(), SoneError> {
    let settings = state.llm_settings.lock().await.clone();
    let provider = build_provider(&settings)?;
    provider.ping().await
}
