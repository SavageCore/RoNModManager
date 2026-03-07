use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::models::{Collection, ModPack};
use crate::services::{modpack as modpack_service, steam};
use crate::state::AppState;

#[tauri::command]
pub async fn set_modpack_url(url: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .update_config(|config| {
            config.modpack_url = Some(url);
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn sync_modpack(_app: AppHandle, state: State<'_, AppState>) -> Result<ModPack, String> {
    let config = state.get_config().map_err(String::from)?;
    let url = config
        .modpack_url
        .ok_or_else(|| "Modpack URL is not configured".to_string())?;

    let pack = modpack_service::fetch_modpack(&state.client, &url)
        .await
        .map_err(String::from)?;

    let version = pack.version.clone();
    state
        .update_config(|cfg| {
            cfg.modpack_version = Some(version);
        })
        .map_err(String::from)?;

    Ok(pack)
}

#[tauri::command]
pub async fn get_modpack_collections(
    state: State<'_, AppState>,
) -> Result<HashMap<String, Collection>, String> {
    let config = state.get_config().map_err(String::from)?;
    let url = config
        .modpack_url
        .ok_or_else(|| "Modpack URL is not configured".to_string())?;

    let pack = modpack_service::fetch_modpack(&state.client, &url)
        .await
        .map_err(String::from)?;

    Ok(pack.collections)
}

#[tauri::command]
pub async fn build_modpack_from_installed(state: State<'_, AppState>) -> Result<ModPack, String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;

    let mods_path = steam::get_mods_path(&game_path);
    let mut mods = Vec::new();

    if mods_path.exists() {
        let entries = fs::read_dir(&mods_path).map_err(|error| error.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("pak"))
                .unwrap_or(false)
            {
                if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                    mods.push(file_name.to_string());
                }
            }
        }
    }

    let mut collections = HashMap::new();
    collections.insert(
        "Default".to_string(),
        Collection {
            default_enabled: true,
            description: Some("Generated from currently installed mods".to_string()),
            mods: mods.clone(),
        },
    );

    Ok(ModPack {
        schema_version: 1,
        name: "Generated ModPack".to_string(),
        version: "0.1.0".to_string(),
        description: "Generated from installed mods".to_string(),
        author: None,
        subscriptions: Vec::new(),
        collections,
    })
}

#[tauri::command]
pub async fn export_modpack_to_file(modpack: ModPack, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let data = serde_json::to_vec_pretty(&modpack).map_err(|error| error.to_string())?;
    fs::write(path, data).map_err(|error| error.to_string())?;
    Ok(())
}
