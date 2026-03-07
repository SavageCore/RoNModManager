use std::fs;

use tauri::State;

use crate::models::ModPack;
use crate::state::AppState;

#[tauri::command]
pub async fn share_modpack_via_code(
    _modpack: ModPack,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    Err("share_modpack_via_code requires sync server and is not implemented yet".to_string())
}

#[tauri::command]
pub async fn import_from_code(
    _code: String,
    _state: State<'_, AppState>,
) -> Result<ModPack, String> {
    Err("import_from_code requires sync server and is not implemented yet".to_string())
}

#[tauri::command]
pub async fn push_modpack_update(
    _code: String,
    _modpack: ModPack,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    Err("push_modpack_update requires sync server and is not implemented yet".to_string())
}

#[tauri::command]
pub async fn import_modpack_from_file(path: String) -> Result<ModPack, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice::<ModPack>(&bytes).map_err(|error| error.to_string())
}
