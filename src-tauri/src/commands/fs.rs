use crate::state;
use crate::state::AppState;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub fn file_exists(_state: State<'_, AppState>, path: String) -> bool {
    let path = PathBuf::from(path);
    path.exists()
}

/// Returns the absolute path to the archive root directory (as a string)
#[tauri::command]
pub fn get_archive_root_path(_state: State<'_, AppState>) -> Result<String, String> {
    match state::app_data_root() {
        Ok(root) => Ok(root.join("staged/archives").to_string_lossy().to_string()),
        Err(e) => Err(format!("Failed to get archive root: {}", e)),
    }
}
