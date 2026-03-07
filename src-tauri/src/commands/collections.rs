use std::collections::HashMap;

use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_collections(state: State<'_, AppState>) -> Result<HashMap<String, bool>, String> {
    let config = state.get_config().map_err(String::from)?;
    Ok(config.collections)
}

#[tauri::command]
pub async fn toggle_collection(
    name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .update_config(|config| {
            config.collections.insert(name, enabled);
        })
        .map(|_| ())
        .map_err(Into::into)
}
