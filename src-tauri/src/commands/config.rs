use serde::Deserialize;
use tauri::State;

use crate::models::config::{AppConfig, ThemeMode};
use crate::models::Result;
use crate::services::nexus_api;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub nexus_api_key: Option<String>,
    pub modio_api_key: Option<String>,
    pub modio_game_id: Option<u32>,
    pub active_profile: Option<String>,
    pub modpack_url: Option<String>,
    pub modpack_version: Option<String>,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig> {
    state.get_config()
}

#[tauri::command]
pub async fn update_config(state: State<'_, AppState>, updates: ConfigUpdate) -> Result<()> {
    let mut should_lookup_game_id = false;
    let mut new_api_key: Option<String> = None;
    state.update_config(|config: &mut AppConfig| {
        if let Some(key) = updates.nexus_api_key {
            config.nexus_api_key = if key.is_empty() { None } else { Some(key) };
        }
        match updates.modio_api_key {
            Some(ref key) if key.is_empty() => {
                config.modio_api_key = None;
                config.modio_game_id = None;
            }
            Some(ref key) => {
                let changed = config.modio_api_key.as_deref() != Some(key);
                config.modio_api_key = Some(key.clone());
                if changed {
                    should_lookup_game_id = true;
                    new_api_key = Some(key.clone());
                }
            }
            None => {}
        }
        if let Some(id) = updates.modio_game_id {
            config.modio_game_id = Some(id);
        }
        if let Some(profile) = updates.active_profile {
            config.active_profile = Some(profile);
        }
        if let Some(url) = updates.modpack_url {
            config.modpack_url = Some(url);
        }
        if let Some(version) = updates.modpack_version {
            config.modpack_version = Some(version);
        }
    })?;

    // If a new modio_api_key was set, look up and save the game ID for 'readyornot'
    if should_lookup_game_id {
        if let Some(api_key) = new_api_key {
            let client = &state.client;
            // Use the current config's modio_game_id if available
            let config = state.get_config()?;
            let service = crate::services::modio_api::ModioApiService::new(
                client.clone(),
                config.modio_game_id,
            );
            match service.lookup_game_id(&api_key, "readyornot").await {
                Ok(game_id) => {
                    state.update_config(|config| {
                        config.modio_game_id = Some(game_id);
                    })?;
                }
                Err(e) => {
                    eprintln!("[update_config] Failed to look up mod.io game_id: {}", e);
                }
            }
        }
    }
    Ok(())
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
pub async fn apply_intro_skip(state: State<'_, AppState>) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("game path not configured".to_string())
    })?;
    crate::services::config_tweaks::apply_intro_skip(&game_path)
}

#[tauri::command]
pub async fn undo_intro_skip(state: State<'_, AppState>) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("game path not configured".to_string())
    })?;
    crate::services::config_tweaks::undo_intro_skip(&game_path)
}

#[tauri::command]
pub async fn is_intro_skip_applied(state: State<'_, AppState>) -> Result<bool> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("game path not configured".to_string())
    })?;
    crate::services::config_tweaks::is_intro_skip_applied(&game_path)
}
