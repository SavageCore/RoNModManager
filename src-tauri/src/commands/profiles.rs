use tauri::State;

use super::game;
use crate::models::Profile;
use crate::services;
use crate::state::AppState;

const DEFAULT_PROFILE_NAME: &str = "Default";

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String> {
    let mut profiles = services::profiles::list_profiles().map_err(|e| e.to_string())?;

    if !profiles.iter().any(|profile| profile.name == DEFAULT_PROFILE_NAME) {
        let default_profile = Profile::new(DEFAULT_PROFILE_NAME.to_string(), Vec::new());
        services::profiles::save_profile(&default_profile).map_err(|e| e.to_string())?;
        profiles.push(default_profile);
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));

    let config = state.get_config().map_err(|e| e.to_string())?;
    let active_profile_valid = config
        .active_profile
        .as_ref()
        .is_some_and(|active_name| profiles.iter().any(|profile| &profile.name == active_name));

    if !active_profile_valid {
        let fallback_profile = profiles
            .iter()
            .find(|profile| profile.name == DEFAULT_PROFILE_NAME)
            .or_else(|| profiles.first())
            .ok_or_else(|| "No profiles available".to_string())?;

        let enabled_groups = fallback_profile.installed_mod_names.clone();
        let fallback_name = fallback_profile.name.clone();

        state
            .update_config(|cfg| {
                cfg.active_profile = Some(fallback_name);
                cfg.enabled_collections = enabled_groups;
            })
            .map_err(|e| e.to_string())?;
    }

    Ok(profiles)
}

#[tauri::command]
pub async fn get_profile(name: String) -> Result<Option<Profile>, String> {
    services::profiles::get_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_profile(
    name: String,
    description: Option<String>,
    installed_mod_names: Vec<String>,
) -> Result<Profile, String> {
    let mut profile = Profile::new(name, installed_mod_names);
    if let Some(desc) = description {
        profile = profile.with_description(desc);
    }
    services::profiles::save_profile(&profile).map_err(|e| e.to_string())?;
    Ok(profile)
}

#[tauri::command]
pub async fn delete_profile(name: String) -> Result<(), String> {
    services::profiles::delete_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_profile(name: String, state: State<'_, AppState>) -> Result<Profile, String> {
    let profile = services::profiles::get_profile(&name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", name))?;

    // Immediately refresh live links for the selected profile so swapping profiles
    // updates the game's active mods without requiring a separate launch/sync step.
    let current_config = state.get_config().map_err(|e| e.to_string())?;
    if let Some(game_path) = current_config.game_path {
        game::sync_mod_links_for_game_path(&game_path, profile.installed_mod_names.clone())?;
    }

    // Update app state with enabled collections from profile
    state
        .update_config(|config| {
            config.enabled_collections = profile.installed_mod_names.clone();
            config.active_profile = Some(profile.name.clone());
        })
        .map_err(|e| e.to_string())?;

    Ok(profile)
}
