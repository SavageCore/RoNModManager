use tauri::State;

use super::game;
use crate::models::{AppError, Profile, Result};
use crate::services;
use crate::state::AppState;

const DEFAULT_PROFILE_NAME: &str = "Default";

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>> {
    let mut profiles = services::profiles::list_profiles()?;

    if !profiles
        .iter()
        .any(|profile| profile.name == DEFAULT_PROFILE_NAME)
    {
        let default_profile = Profile::new(DEFAULT_PROFILE_NAME.to_string(), Vec::new());
        services::profiles::save_profile(&default_profile)?;
        profiles.push(default_profile);
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));

    let config = state.get_config()?;
    let active_profile_valid = config
        .active_profile
        .as_ref()
        .is_some_and(|active_name| profiles.iter().any(|profile| &profile.name == active_name));

    if !active_profile_valid {
        let fallback_profile = profiles
            .iter()
            .find(|profile| profile.name == DEFAULT_PROFILE_NAME)
            .or_else(|| profiles.first())
            .ok_or_else(|| AppError::Validation("No profiles available".to_string()))?;

        let fallback_name = fallback_profile.name.clone();

        state.update_config(|cfg| {
            cfg.active_profile = Some(fallback_name);
        })?;
    }

    Ok(profiles)
}

#[tauri::command]
pub async fn get_profile(name: String) -> Result<Option<Profile>> {
    services::profiles::get_profile(&name)
}

#[tauri::command]
pub async fn save_profile(
    name: String,
    description: Option<String>,
    installed_mod_names: Vec<String>,
) -> Result<Profile> {
    let mut profile = Profile::new(name, installed_mod_names);
    if let Some(existing) = services::profiles::get_profile(&profile.name)? {
        profile.enabled_collections = existing.enabled_collections;
        profile.collections = existing.collections;
        profile.tags = existing.tags;
        profile.collection_colors = existing.collection_colors;
        profile.created_at = existing.created_at;
        profile.broken_mods = existing.broken_mods;
        profile.no_world_gen = existing.no_world_gen;
    }
    if let Some(desc) = description {
        profile = profile.with_description(desc);
    }
    services::profiles::save_profile(&profile)?;
    Ok(profile)
}

#[tauri::command]
pub async fn delete_profile(name: String) -> Result<()> {
    services::profiles::delete_profile(&name)
}

#[tauri::command]
pub async fn apply_profile(name: String, state: State<'_, AppState>) -> Result<Profile> {
    let profile = services::profiles::get_profile(&name)?
        .ok_or_else(|| AppError::Validation(format!("Profile '{}' not found", name)))?;

    let updated_config = state.update_config(|config| {
        config.active_profile = Some(profile.name.clone());
    })?;

    if let Some(ref game_path) = updated_config.game_path {
        game::sync_mod_links_for_game_path(game_path, profile.installed_mod_names.clone())
            .map_err(AppError::Validation)?;
    }

    Ok(profile)
}
