use std::path::PathBuf;

use tauri::State;

use crate::models::AppError;
use crate::services::steam;
use crate::state::AppState;

#[tauri::command]
pub async fn detect_game_path(_state: State<'_, AppState>) -> Result<Option<String>, String> {
    match steam::detect_game_path() {
        Ok(path) => Ok(Some(path.to_string_lossy().to_string())),
        Err(AppError::NotFound(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub async fn set_game_path(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err("Game path does not exist".to_string());
    }

    state
        .update_config(|config| {
            config.game_path = Some(path_buf);
        })
        .map(|_| ())
        .map_err(Into::into)
}
