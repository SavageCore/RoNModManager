use std::collections::HashMap;
use std::collections::HashSet;

use tauri::State;

use crate::models::Result;
use crate::services::profiles;
use crate::state::AppState;

fn apply_collection_state(
    profile: &mut crate::models::Profile,
    collection_name: &str,
    enabled: bool,
) {
    let mut enabled_collection_set: HashSet<String> =
        profile.enabled_collections.iter().cloned().collect();

    if enabled {
        enabled_collection_set.insert(collection_name.to_string());
    } else {
        enabled_collection_set.remove(collection_name);
    }

    profile.enabled_collections = enabled_collection_set.into_iter().collect();

    // Get all mods from all currently enabled collections
    let mut enabled_groups: HashSet<String> = HashSet::new();
    for name in &profile.enabled_collections {
        if let Some(mods) = profile.collections.get(name) {
            for m in mods {
                enabled_groups.insert(m.clone());
            }
        }
    }

    profile.installed_mod_names = enabled_groups.into_iter().collect();
}

#[tauri::command]
pub async fn get_collections(state: State<'_, AppState>) -> Result<HashMap<String, bool>> {
    let config = state.get_config()?;
    let active_profile_name = match config.active_profile {
        Some(name) => name,
        None => return Ok(HashMap::new()),
    };

    let profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    let mut result = HashMap::new();
    let enabled_collection_set: HashSet<String> = profile.enabled_collections.into_iter().collect();
    for name in profile.collections.keys() {
        let enabled = enabled_collection_set.contains(name);
        result.insert(name.to_string(), enabled);
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_collection_mods(
    state: State<'_, AppState>,
) -> Result<HashMap<String, Vec<String>>> {
    let config = state.get_config()?;
    let active_profile_name = match config.active_profile {
        Some(name) => name,
        None => return Ok(HashMap::new()),
    };

    let profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    Ok(profile.collections)
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    name: String,
    #[allow(non_snake_case)] modNames: Vec<String>,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    profile.collections.insert(name, modNames);
    profiles::save_profile(&profile)?;

    Ok(())
}

#[tauri::command]
pub async fn add_mod_to_collection(
    state: State<'_, AppState>,
    collection: String,
    #[allow(non_snake_case)] modName: String,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    if let Some(mods) = profile.collections.get_mut(&collection) {
        if !mods.contains(&modName) {
            mods.push(modName);
            profiles::save_profile(&profile)?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn remove_mod_from_collection(
    state: State<'_, AppState>,
    collection: String,
    #[allow(non_snake_case)] modName: String,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    if let Some(mods) = profile.collections.get_mut(&collection) {
        mods.retain(|m| m != &modName);
        profiles::save_profile(&profile)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, name: String) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    profile.collections.remove(&name);
    profile.enabled_collections.retain(|c| c != &name);
    profiles::save_profile(&profile)?;

    Ok(())
}

#[tauri::command]
pub async fn rename_collection(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] oldName: String,
    #[allow(non_snake_case)] newName: String,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    if profile.collections.contains_key(&newName) {
        return Err(crate::models::AppError::Validation(format!(
            "Collection '{}' already exists",
            newName
        )));
    }

    if let Some(mods) = profile.collections.remove(&oldName) {
        profile.collections.insert(newName.clone(), mods);
    } else {
        return Err(crate::models::AppError::Validation(format!(
            "Collection '{}' not found",
            oldName
        )));
    }

    for item in profile.enabled_collections.iter_mut() {
        if *item == oldName {
            *item = newName.clone();
            break;
        }
    }

    profiles::save_profile(&profile)?;
    Ok(())
}

#[tauri::command]
pub async fn toggle_collection(
    state: State<'_, AppState>,
    name: String,
    enabled: bool,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    apply_collection_state(&mut profile, &name, enabled);
    profiles::save_profile(&profile)?;

    if let Some(ref game_path) = config.game_path {
        super::game::sync_mod_links_for_game_path(game_path, profile.installed_mod_names.clone())
            .map_err(crate::models::AppError::Validation)?;
    }

    Ok(())
}
