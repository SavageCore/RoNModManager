use tauri::State;
use log;
use crate::state::AppState;
use crate::models::{AppError, Result};

#[tauri::command]
pub async fn fetch_modpack_json(url: String, state: State<'_, AppState>) -> Result<serde_json::Value> {
    let client = &state.client;
    log::info!("Fetching modpack JSON from URL: {}", url);
    let response = client.get(&url).send().await.map_err(AppError::from)?;
    log::info!("HTTP status: {}", response.status());
    let response = response.error_for_status().map_err(AppError::from)?;
    let bytes = response.bytes().await.map_err(AppError::from)?;
    match std::str::from_utf8(&bytes) {
        Ok(body_str) => log::info!("Response body: {}", body_str),
        Err(_) => log::info!("Response body: <non-UTF8 bytes, length={}>", bytes.len()),
    }
    let json = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(AppError::from)?;
    Ok(json)
}
