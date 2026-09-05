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
        profile.modpack_meta = existing.modpack_meta;
        profile.sync_remote_host = existing.sync_remote_host;
        profile.sync_remote_path = existing.sync_remote_path;
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
pub async fn rename_profile(
    old_name: String,
    new_name: String,
    description: Option<String>,
    installed_mod_names: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Profile> {
    let old_name = old_name.trim().to_string();
    let new_name = new_name.trim().to_string();
    if old_name.is_empty() || new_name.is_empty() {
        return Err(AppError::Validation("Profile name is required".to_string()));
    }
    if old_name == new_name {
        return Err(AppError::Validation(
            "New name must be different".to_string(),
        ));
    }
    if services::profiles::get_profile(&new_name)?.is_some() {
        return Err(AppError::Validation(format!(
            "Profile '{}' already exists",
            new_name
        )));
    }
    let mut profile = services::profiles::get_profile(&old_name)?
        .ok_or_else(|| AppError::Validation(format!("Profile '{}' not found", old_name)))?;
    profile.name = new_name.clone();
    profile.description = description;
    profile.installed_mod_names = installed_mod_names;
    // Save the profile under the new name
    services::profiles::save_profile(&profile)?;
    // Delete the old profile file
    services::profiles::delete_profile(&old_name)?;
    // If the renamed profile was the active one, update the active profile in config
    let config = state.get_config()?;
    if config.active_profile.as_deref() == Some(old_name.as_str()) {
        let _ = state.update_config(|cfg| {
            cfg.active_profile = Some(new_name.clone());
        })?;
    }
    Ok(profile)
}

#[tauri::command]
pub async fn duplicate_profile(old_name: String, new_name: String) -> Result<Profile> {
    let old_name = old_name.trim().to_string();
    let new_name = new_name.trim().to_string();
    if old_name.is_empty() || new_name.is_empty() {
        return Err(AppError::Validation("Profile name is required".to_string()));
    }
    if old_name == new_name {
        return Err(AppError::Validation(
            "New name must be different".to_string(),
        ));
    }
    if services::profiles::get_profile(&new_name)?.is_some() {
        return Err(AppError::Validation(format!(
            "Profile '{}' already exists",
            new_name
        )));
    }
    let mut profile = services::profiles::get_profile(&old_name)?
        .ok_or_else(|| AppError::Validation(format!("Profile '{}' not found", old_name)))?;
    profile.name = new_name;
    profile.created_at = chrono::Utc::now().to_rfc3339();
    services::profiles::save_profile(&profile)?;
    Ok(profile)
}

#[tauri::command]
pub async fn get_modpack_meta(
    state: State<'_, AppState>,
) -> Result<Option<crate::models::ModpackMeta>> {
    let config = state.get_config()?;
    let active_profile_name = match config.active_profile {
        Some(name) => name,
        None => return Ok(None),
    };

    Ok(services::profiles::get_profile(&active_profile_name)?
        .and_then(|profile| profile.modpack_meta))
}

#[tauri::command]
pub async fn set_modpack_meta(
    name: String,
    version: String,
    description: Option<String>,
    author: Option<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| AppError::Validation("No active profile".to_string()))?;

    let mut profile = services::profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    profile.modpack_meta = Some(crate::models::ModpackMeta {
        name,
        version,
        description: description.unwrap_or_default(),
        author,
    });
    services::profiles::save_profile(&profile)?;
    Ok(())
}

pub fn resolve_sync_details(
    state: &State<'_, AppState>,
) -> Result<(Option<String>, Option<String>)> {
    let config = state.get_config()?;
    let Some(active_profile_name) = config.active_profile.clone() else {
        return Ok((
            config.sync_remote_host.clone(),
            config.sync_remote_path.clone(),
        ));
    };

    match services::profiles::get_profile(&active_profile_name)? {
        Some(profile) => Ok((
            profile
                .sync_remote_host
                .or_else(|| config.sync_remote_host.clone()),
            profile
                .sync_remote_path
                .or_else(|| config.sync_remote_path.clone()),
        )),
        None => Ok((
            config.sync_remote_host.clone(),
            config.sync_remote_path.clone(),
        )),
    }
}

#[tauri::command]
pub async fn get_sync_details(state: State<'_, AppState>) -> Result<crate::models::SyncDetails> {
    let (host, path) = resolve_sync_details(&state)?;
    Ok(crate::models::SyncDetails { host, path })
}

#[tauri::command]
pub async fn set_sync_details(
    host: Option<String>,
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| AppError::Validation("No active profile".to_string()))?;

    let mut profile = services::profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    profile.sync_remote_host = normalize(host);
    profile.sync_remote_path = normalize(path);
    services::profiles::save_profile(&profile)?;
    Ok(())
}

/// Trim and treat empty strings as None so clearing an input removes the stored value.
fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
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
