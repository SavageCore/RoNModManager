use std::fs;

use tauri::{AppHandle, State};

use crate::models::{ModInfo, ModSource, ModStatus};
use crate::services::steam;
use crate::state::AppState;

#[tauri::command]
pub async fn get_mod_list(state: State<'_, AppState>) -> Result<Vec<ModInfo>, String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);

    if !mods_path.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    for entry in fs::read_dir(&mods_path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_pak = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pak"))
            .unwrap_or(false);
        if !is_pak {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        mods.push(ModInfo {
            name: file_name.clone(),
            source: ModSource::Manual,
            status: ModStatus::Installed,
            filename: file_name,
        });
    }

    mods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(mods)
}

#[tauri::command]
pub async fn install_mods(_app: AppHandle, _state: State<'_, AppState>) -> Result<(), String> {
    Err("install_mods orchestration is not implemented yet".to_string())
}

#[tauri::command]
pub async fn uninstall_mods(_app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);

    if !mods_path.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&mods_path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pak"))
            .unwrap_or(false)
        {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}
