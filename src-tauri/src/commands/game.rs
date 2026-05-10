use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tauri::State;

use crate::models::AppError;
use crate::services::manifest;
use crate::services::steam;
use crate::state::{app_data_root, AppState};

fn get_local_store_root() -> Result<PathBuf, String> {
    app_data_root().map_err(|e| e.to_string())
}

fn get_staging_root() -> Result<PathBuf, String> {
    let root = get_local_store_root()?.join("staged");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn create_file_link(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst).map_err(|e| e.to_string())
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(src, dst).map_err(|e| e.to_string())
    }
}

fn target_path_for_staged_file(
    staged_path: &Path,
    live_mods_path: &Path,
    live_savegames_path: &Path,
) -> Option<PathBuf> {
    let file_name = staged_path.file_name()?;
    let ext = staged_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    if ext.eq_ignore_ascii_case("pak") {
        return Some(live_mods_path.join(file_name));
    }
    if ext.eq_ignore_ascii_case("sav") {
        return Some(live_savegames_path.join(file_name));
    }
    None
}

fn launch_game_internal(_game_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let exe_path = _game_path.join("ReadyOrNot.exe");
        if !exe_path.exists() {
            return Err(format!(
                "Game executable not found at {}",
                exe_path.display()
            ));
        }
        Command::new("cmd")
            .args(&["/C", &format!("start \"\" \"{}\"", exe_path.display())])
            .spawn()
            .map_err(|e| format!("Failed to launch game: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Linux, launch through Steam using the app ID for Ready or Not (1144200)
        // This ensures Proton compatibility and proper game initialization
        Command::new("steam")
            .args(["steam://rungameid/1144200"])
            .spawn()
            .map_err(|e| {
                format!(
                    "Failed to launch game via Steam: {}. Make sure Steam is running.",
                    e
                )
            })?;
    }

    Ok(())
}

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

#[tauri::command]
pub async fn launch_game(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(|e: AppError| e.to_string())?;

    let game_path = config
        .game_path
        .ok_or_else(|| "Game path not configured".to_string())?;

    launch_game_internal(&game_path)
}

fn remove_orphan_symlinks(live_mods_path: &Path, live_savegames_path: &Path) -> Result<(), String> {
    // Remove broken symlinks in mods folder
    if let Ok(entries) = fs::read_dir(live_mods_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Check if it's a symlink and the target doesn't exist
            if path.is_symlink() {
                if let Ok(target) = fs::read_link(&path) {
                    if !target.exists() {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // Remove broken symlinks in savegames folder
    if let Ok(entries) = fs::read_dir(live_savegames_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Check if it's a symlink and the target doesn't exist
            if path.is_symlink() {
                if let Ok(target) = fs::read_link(&path) {
                    if !target.exists() {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn sync_mod_links_for_game_path(
    game_path: &Path,
    enabled_groups: Vec<String>,
) -> Result<(), String> {
    let live_mods_path = steam::get_mods_path(game_path);
    let live_savegames_path = steam::get_savegames_path().map_err(|e| e.to_string())?;
    fs::create_dir_all(&live_mods_path).map_err(|e| e.to_string())?;
    fs::create_dir_all(&live_savegames_path).map_err(|e| e.to_string())?;

    let enabled: HashSet<String> = enabled_groups.into_iter().collect();
    let manager = manifest::ManifestManager::new(&get_staging_root()?);
    let manifests = manager.list_all_manifests().unwrap_or_default();

    // First remove all managed live links/files for tracked staged files.
    for manifest_data in manifests.values() {
        for staged_path in &manifest_data.installed_files {
            if let Some(target) =
                target_path_for_staged_file(staged_path, &live_mods_path, &live_savegames_path)
            {
                if target.exists() {
                    let _ = fs::remove_file(&target);
                }
            }
        }
    }

    // Then link only enabled groups.
    for manifest_data in manifests.values() {
        if !enabled.contains(&manifest_data.source_archive) {
            continue;
        }
        for staged_path in &manifest_data.installed_files {
            if !staged_path.exists() {
                continue;
            }
            let Some(target) =
                target_path_for_staged_file(staged_path, &live_mods_path, &live_savegames_path)
            else {
                continue;
            };

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            create_file_link(staged_path, &target)?;
        }
    }

    // Clean up any orphaned symlinks (pointing to non-existent files)
    remove_orphan_symlinks(&live_mods_path, &live_savegames_path)?;

    Ok(())
}

#[tauri::command]
pub async fn sync_mod_links(
    state: State<'_, AppState>,
    enabled_groups: Vec<String>,
) -> Result<(), String> {
    let config = state.get_config().map_err(|e: AppError| e.to_string())?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path not configured".to_string())?;

    sync_mod_links_for_game_path(&game_path, enabled_groups)
}

#[tauri::command]
pub async fn launch_game_with_groups(
    state: State<'_, AppState>,
    enabled_groups: Vec<String>,
) -> Result<(), String> {
    let config = state.get_config().map_err(|e: AppError| e.to_string())?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path not configured".to_string())?;

    sync_mod_links_for_game_path(&game_path, enabled_groups)?;
    launch_game_internal(&game_path)
}
