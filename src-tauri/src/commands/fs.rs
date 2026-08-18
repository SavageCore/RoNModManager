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

/// Open the containing directory of a file in the system file manager.
///
/// On Linux this uses xdg-open on the parent directory, which works natively
/// and inside a Flatpak sandbox (the tauri-plugin-opener reveal-in-dir path
/// fails with org.freedesktop.DBus.Error.ServiceUnknown when no file manager
/// exposes org.freedesktop.FileManager1). Returns the opened directory path.
#[tauri::command]
pub async fn reveal_in_file_manager(
    #[cfg_attr(target_os = "linux", allow(unused_variables))] app: tauri::AppHandle,
    path: String,
) -> Result<String, String> {
    let file = std::path::PathBuf::from(&path);
    if !file.exists() {
        return Err(format!("File is missing on disk: {}", path));
    }
    let canonical = file
        .canonicalize()
        .map_err(|e| format!("Failed to resolve path {}: {}", path, e))?;
    let dir = canonical
        .parent()
        .ok_or_else(|| format!("Failed to determine parent directory of {}", path))?
        .to_path_buf();

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open file location: {}", e))?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .reveal_item_in_dir(&canonical)
            .map_err(|e| format!("Failed to open file location: {}", e))?;
    }

    Ok(dir.to_string_lossy().to_string())
}
