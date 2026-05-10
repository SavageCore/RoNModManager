use serde::Deserialize;
use tauri::State;

use crate::models::AppConfig;
use crate::services;
use crate::services::nexus_api;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub nexus_api_key: Option<String>,
    pub enabled_collections: Option<Vec<String>>,
    pub active_profile: Option<String>,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.get_config().map_err(Into::into)
}

#[tauri::command]
pub async fn update_config(
    updates: ConfigUpdate,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .update_config(|config| {
            if let Some(key) = updates.nexus_api_key {
                config.nexus_api_key = if key.trim().is_empty() {
                    None
                } else {
                    Some(key.trim().to_string())
                };
            }
            if let Some(enabled) = updates.enabled_collections {
                config.enabled_collections = enabled;
            }
            if let Some(profile_name) = updates.active_profile {
                config.active_profile = if profile_name.trim().is_empty() {
                    None
                } else {
                    Some(profile_name.trim().to_string())
                };
            }
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn verify_nexus_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let service = nexus_api::NexusApiService::new(state.client.clone());
    service
        .validate_api_key(&api_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_theme(theme: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .update_config(|config| {
            config.theme = match theme.as_str() {
                "light" => crate::models::ThemeMode::Light,
                "dark" => crate::models::ThemeMode::Dark,
                _ => crate::models::ThemeMode::System,
            };
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn apply_intro_skip() -> Result<(), String> {
    services::config_tweaks::apply_intro_skip().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn undo_intro_skip() -> Result<(), String> {
    services::config_tweaks::undo_intro_skip().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_intro_skip_applied() -> Result<bool, String> {
    services::config_tweaks::is_intro_skip_applied().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_intro_skip_ini_path() -> Result<String, String> {
    services::config_tweaks::get_game_ini_path()
        .map(|path| path.display().to_string())
        .map_err(|e| e.to_string())
}
