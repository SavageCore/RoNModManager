use std::fs;
use std::path::PathBuf;

use crate::models::{AppError, Profile, Result};
use crate::state::app_data_root;

/// Get the profiles directory path
fn get_profiles_dir() -> Result<PathBuf> {
    let config_dir = app_data_root().map_err(|e| AppError::Validation(e.to_string()))?;

    let profiles_dir = config_dir.join("profiles");
    fs::create_dir_all(&profiles_dir)
        .map_err(|e| AppError::Validation(format!("failed to create profiles directory: {e}")))?;
    Ok(profiles_dir)
}

/// Get path to a specific profile file
fn get_profile_path(name: &str) -> Result<PathBuf> {
    let profiles_dir = get_profiles_dir()?;
    Ok(profiles_dir.join(format!("{}.json", name)))
}

/// Load all profiles
pub fn list_profiles() -> Result<Vec<Profile>> {
    let profiles_dir = get_profiles_dir()?;
    let mut profiles = Vec::new();

    if !profiles_dir.exists() {
        return Ok(profiles);
    }

    for entry in fs::read_dir(&profiles_dir)
        .map_err(|e| AppError::Validation(format!("failed to read profiles directory: {e}")))?
    {
        let entry = entry
            .map_err(|e| AppError::Validation(format!("failed to read profile entry: {e}")))?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)
                .map_err(|e| AppError::Validation(format!("failed to read profile: {e}")))?;
            match serde_json::from_str::<Profile>(&content) {
                Ok(profile) => profiles.push(profile),
                Err(_) => {
                    // Skip invalid profiles silently
                }
            }
        }
    }

    Ok(profiles)
}

/// Load a specific profile by name
pub fn get_profile(name: &str) -> Result<Option<Profile>> {
    let path = get_profile_path(name)?;

    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| AppError::Validation(format!("failed to read profile: {e}")))?;
    let profile = serde_json::from_str::<Profile>(&content)
        .map_err(|e| AppError::Validation(format!("invalid profile JSON: {e}")))?;
    Ok(Some(profile))
}

/// Save a profile
pub fn save_profile(profile: &Profile) -> Result<()> {
    let path = get_profile_path(&profile.name)?;
    let json = serde_json::to_string_pretty(profile)
        .map_err(|e| AppError::Validation(format!("failed to serialize profile: {e}")))?;
    fs::write(&path, json)
        .map_err(|e| AppError::Validation(format!("failed to save profile: {e}")))?;
    Ok(())
}

/// Delete a profile
pub fn delete_profile(name: &str) -> Result<()> {
    let path = get_profile_path(name)?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| AppError::Validation(format!("failed to delete profile: {e}")))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_serialization() {
        let profile = Profile::new("Test Profile".to_string(), vec!["Collection1".to_string()])
            .with_description("Test Description".to_string());

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "Test Profile");
        assert_eq!(
            deserialized.description,
            Some("Test Description".to_string())
        );
        assert_eq!(deserialized.installed_mod_names.len(), 1);
    }
}
