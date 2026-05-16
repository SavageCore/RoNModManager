use serde::Deserialize;
use tauri::State;

use crate::models::config::{AppConfig, ThemeMode};
use crate::models::Result;
use crate::services::nexus_api;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub nexus_api_key: Option<String>,
    pub active_profile: Option<String>,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig> {
    state.get_config()
}

#[tauri::command]
pub async fn update_config(state: State<'_, AppState>, updates: ConfigUpdate) -> Result<()> {
    state
        .update_config(|config: &mut AppConfig| {
            if let Some(key) = updates.nexus_api_key {
                config.nexus_api_key = Some(key);
            }
            if let Some(profile) = updates.active_profile {
                config.active_profile = Some(profile);
            }
        })
        .map(|_| ())
}

#[tauri::command]
pub async fn verify_nexus_api_key(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] apiKey: String,
) -> Result<bool> {
    let service = nexus_api::NexusApiService::new(state.client.clone());
    match service.get_mod_info(&apiKey, 981).await {
        // Use a known mod ID to verify
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn set_theme(state: State<'_, AppState>, theme: ThemeMode) -> Result<()> {
    state.update_config(|config| {
        config.theme = theme;
    })?;
    Ok(())
}

#[tauri::command]
pub async fn apply_intro_skip(_state: State<'_, AppState>) -> Result<()> {
    crate::services::config_tweaks::apply_intro_skip()?;
    Ok(())
}

#[tauri::command]
pub async fn undo_intro_skip(_state: State<'_, AppState>) -> Result<()> {
    crate::services::config_tweaks::undo_intro_skip()?;
    Ok(())
}

#[tauri::command]
pub async fn is_intro_skip_applied(_state: State<'_, AppState>) -> Result<bool> {
    crate::services::config_tweaks::is_intro_skip_applied()
}

#[tauri::command]
pub async fn get_intro_skip_ini_path(_state: State<'_, AppState>) -> Result<String> {
    Ok(crate::services::config_tweaks::get_game_ini_path()?
        .to_string_lossy()
        .to_string())
}
