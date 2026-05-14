use std::collections::HashMap;
use std::collections::HashSet;

use tauri::State;

use super::game;
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

    let target_mods = profile
        .collections
        .get(collection_name)
        .cloned()
        .unwrap_or_default();

    let mut installed_mod_set: HashSet<String> =
        profile.installed_mod_names.iter().cloned().collect();

    if enabled {
        for mod_name in &target_mods {
            installed_mod_set.insert(mod_name.clone());
        }
    } else {
        let mut protected_mods = HashSet::new();
        for enabled_collection in &enabled_collection_set {
            if let Some(collection_mods) = profile.collections.get(enabled_collection) {
                for mod_name in collection_mods {
                    protected_mods.insert(mod_name.clone());
                }
            }
        }

        for mod_name in &target_mods {
            if !protected_mods.contains(mod_name) {
                installed_mod_set.remove(mod_name);
            }
        }
    }

    let mut installed_mod_names: Vec<String> = installed_mod_set.into_iter().collect();
    installed_mod_names.sort();
    profile.installed_mod_names = installed_mod_names;

    let mut enabled_collection_names: Vec<String> = enabled_collection_set.into_iter().collect();
    enabled_collection_names.sort();
    profile.enabled_collections = enabled_collection_names;
}

fn update_config_collection_state(
    state: &State<'_, AppState>,
    collection_name: String,
    enabled: bool,
    enabled_groups: Vec<String>,
) -> Result<(), String> {
    state
        .update_config(|cfg| {
            cfg.collections.insert(collection_name, enabled);
            cfg.enabled_collections = enabled_groups;
        })
        .map(|_| ())
        .map_err(String::from)
}

#[tauri::command]
pub async fn get_collections(state: State<'_, AppState>) -> Result<HashMap<String, bool>, String> {
    let config = state.get_config().map_err(String::from)?;
    let Some(active_profile_name) = config.active_profile else {
        return Ok(HashMap::new());
    };

    let profile = profiles::get_profile(&active_profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", active_profile_name))?;

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
) -> Result<HashMap<String, Vec<String>>, String> {
    let config = state.get_config().map_err(String::from)?;
    let Some(active_profile_name) = config.active_profile else {
        return Ok(HashMap::new());
    };

    let profile = profiles::get_profile(&active_profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", active_profile_name))?;

    Ok(profile.collections)
}

#[tauri::command]
pub async fn create_collection(
    name: String,
    mod_names: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Collection name cannot be empty".to_string());
    }

    let config = state.get_config().map_err(String::from)?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| "No active profile selected".to_string())?;

    let mut profile = profiles::get_profile(&active_profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", active_profile_name))?;

    if profile.collections.contains_key(trimmed) {
        return Err(format!("Collection '{}' already exists", trimmed));
    }

    let mut unique_mods: Vec<String> = mod_names
        .into_iter()
        .filter(|m| !m.trim().is_empty())
        .collect();
    unique_mods.sort();
    unique_mods.dedup();

    profile.collections.insert(trimmed.to_string(), unique_mods);
    profiles::save_profile(&profile).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn add_mod_to_collection(
    collection: String,
    mod_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let collection = collection.trim();
    let mod_name = mod_name.trim();
    if collection.is_empty() || mod_name.is_empty() {
        return Err("Collection name and mod name are required".to_string());
    }

    let config = state.get_config().map_err(String::from)?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| "No active profile selected".to_string())?;

    let mut profile = profiles::get_profile(&active_profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", active_profile_name))?;

    let collection_mods = profile
        .collections
        .get_mut(collection)
        .ok_or_else(|| format!("Collection '{}' not found", collection))?;

    if !collection_mods.iter().any(|m| m == mod_name) {
        collection_mods.push(mod_name.to_string());
        collection_mods.sort();
    }

    if profile.enabled_collections.iter().any(|c| c == collection)
        && !profile.installed_mod_names.iter().any(|m| m == mod_name)
    {
        profile.installed_mod_names.push(mod_name.to_string());
        profile.installed_mod_names.sort();
    }

    let installed_groups = profile.installed_mod_names.clone();
    profiles::save_profile(&profile).map_err(|e| e.to_string())?;

    if let Some(game_path) = config.game_path {
        game::sync_mod_links_for_game_path(&game_path, installed_groups)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn remove_mod_from_collection(
    collection: String,
    mod_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let collection = collection.trim();
    let mod_name = mod_name.trim();
    if collection.is_empty() || mod_name.is_empty() {
        return Err("Collection name and mod name are required".to_string());
    }

    let config = state.get_config().map_err(String::from)?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| "No active profile selected".to_string())?;

    let mut profile = profiles::get_profile(&active_profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", active_profile_name))?;

    let collection_mods = profile
        .collections
        .get_mut(collection)
        .ok_or_else(|| format!("Collection '{}' not found", collection))?;

    collection_mods.retain(|m| m != mod_name);

    let enabled_other_mods: HashSet<String> = profile
        .enabled_collections
        .iter()
        .filter(|c| c.as_str() != collection)
        .filter_map(|c| profile.collections.get(c))
        .flatten()
        .cloned()
        .collect();

    if profile.enabled_collections.iter().any(|c| c == collection)
        && !enabled_other_mods.contains(mod_name)
    {
        profile.installed_mod_names.retain(|m| m != mod_name);
    }

    let installed_groups = profile.installed_mod_names.clone();
    profiles::save_profile(&profile).map_err(|e| e.to_string())?;

    if let Some(game_path) = config.game_path {
        game::sync_mod_links_for_game_path(&game_path, installed_groups)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_collection(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Collection name cannot be empty".to_string());
    }

    let config = state.get_config().map_err(String::from)?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| "No active profile selected".to_string())?;

    let mut profile = profiles::get_profile(&active_profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", active_profile_name))?;

    let removed_mods = profile
        .collections
        .remove(trimmed)
        .ok_or_else(|| format!("Collection '{}' not found", trimmed))?;

    profile.enabled_collections.retain(|name| name != trimmed);

    let protected_mods: HashSet<String> = profile
        .enabled_collections
        .iter()
        .filter_map(|collection_name| profile.collections.get(collection_name))
        .flatten()
        .cloned()
        .collect();

    for mod_name in removed_mods {
        if !protected_mods.contains(&mod_name) {
            profile.installed_mod_names.retain(|m| m != &mod_name);
        }
    }

    let installed_groups = profile.installed_mod_names.clone();
    profiles::save_profile(&profile).map_err(|e| e.to_string())?;

    state
        .update_config(|cfg| {
            cfg.collections.remove(trimmed);
            cfg.enabled_collections = installed_groups.clone();
        })
        .map_err(String::from)?;

    if let Some(game_path) = config.game_path {
        game::sync_mod_links_for_game_path(&game_path, installed_groups)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn toggle_collection(
    name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| "No active profile selected".to_string())?;

    let mut profile = profiles::get_profile(&active_profile_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Profile '{}' not found", active_profile_name))?;

    if !profile.collections.contains_key(&name) {
        return Err(format!("Collection '{}' not found", name));
    }

    apply_collection_state(&mut profile, &name, enabled);

    let installed_mod_names = profile.installed_mod_names.clone();
    profiles::save_profile(&profile).map_err(|e| e.to_string())?;

    update_config_collection_state(&state, name, enabled, installed_mod_names.clone())?;

    if let Some(game_path) = config.game_path {
        game::sync_mod_links_for_game_path(&game_path, installed_mod_names)?;
    }

    Ok(())
}
