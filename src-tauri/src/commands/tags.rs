use std::collections::HashMap;

use tauri::State;

use crate::models::Result;
use crate::services::profiles;
use crate::state::AppState;

#[tauri::command]
pub async fn get_tags(state: State<'_, AppState>) -> Result<HashMap<String, Vec<String>>> {
    let config = state.get_config()?;
    let active_profile_name = match config.active_profile {
        Some(name) => name,
        None => return Ok(HashMap::new()),
    };

    let profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    Ok(profile.tags)
}

#[tauri::command]
pub async fn set_mod_tags(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] modName: String,
    #[allow(non_snake_case)] newTags: Vec<String>,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    // Remove this mod from all existing tags
    for mods in profile.tags.values_mut() {
        mods.retain(|m| m != &modName);
    }
    // Prune tags with no remaining mods
    profile.tags.retain(|_, mods| !mods.is_empty());

    // Add the mod to each desired tag, creating missing tags
    for tag_name in newTags {
        profile
            .tags
            .entry(tag_name)
            .or_default()
            .push(modName.clone());
    }

    profiles::save_profile(&profile)?;
    Ok(())
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, name: String) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    profile.tags.remove(&name);
    profiles::save_profile(&profile)?;
    Ok(())
}
