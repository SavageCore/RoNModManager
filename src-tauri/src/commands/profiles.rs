use tauri::State;

use crate::models::Profile;
use crate::services;
use crate::state::AppState;

#[tauri::command]
pub async fn list_profiles() -> Result<Vec<Profile>, String> {
    services::profiles::list_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_profile(name: String) -> Result<Option<Profile>, String> {
    services::profiles::get_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_profile(
    name: String,
    description: Option<String>,
    enabled_collections: Vec<String>,
) -> Result<Profile, String> {
    let mut profile = Profile::new(name, enabled_collections);
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

    // Update app state with enabled collections from profile
    state
        .update_config(|config| {
            config.enabled_collections = profile.enabled_collections.clone();
        })
        .map_err(|e| e.to_string())?;

    Ok(profile)
}
