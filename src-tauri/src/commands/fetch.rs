use crate::models::{AppError, Result};
use crate::state::AppState;
use log;
use tauri::State;

#[tauri::command]
pub async fn fetch_modpack_json(
    url: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value> {
    let client = &state.client;
    let response = client.get(&url).send().await.map_err(AppError::from)?;
    let response = response.error_for_status().map_err(AppError::from)?;
    let bytes = response.bytes().await.map_err(AppError::from)?;
    match std::str::from_utf8(&bytes) {
        Ok(_) => {}
        Err(_) => log::error!("Response body: <non-UTF8 bytes, length={}>", bytes.len()),
    }
    let json = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(AppError::from)?;
    Ok(json)
}
