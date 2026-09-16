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
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &json)
        .map_err(|e| AppError::Validation(format!("failed to write profile to temp file: {e}")))?;
    fs::rename(&tmp_path, &path)
        .map_err(|e| AppError::Validation(format!("failed to replace profile file: {e}")))?;
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
    use crate::test_support::isolated_root;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Profiles live in a process-global directory once `isolated_root` has
    /// redirected it, so every test works on its own profile names.
    fn unique_name(tag: &str) -> String {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        format!("cov-{tag}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
    }

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

    #[test]
    fn save_then_get_round_trips_a_profile() {
        isolated_root();
        let name = unique_name("round-trip");
        let profile = Profile::new(name.clone(), vec!["mod-a.zip".to_string()])
            .with_description("round trip".to_string());

        save_profile(&profile).unwrap();

        let loaded = get_profile(&name).unwrap().expect("saved profile is found");
        assert_eq!(loaded.name, name);
        assert_eq!(loaded.installed_mod_names, vec!["mod-a.zip".to_string()]);
        assert_eq!(loaded.description, Some("round trip".to_string()));
    }

    #[test]
    fn get_profile_missing_or_deleted_returns_none() {
        isolated_root();
        let name = unique_name("missing");
        assert!(get_profile(&name).unwrap().is_none());

        save_profile(&Profile::new(name.clone(), Vec::new())).unwrap();
        delete_profile(&name).unwrap();
        assert!(get_profile(&name).unwrap().is_none());

        // Deleting an absent profile is a no-op, not an error.
        delete_profile(&name).unwrap();
        delete_profile(&unique_name("never-saved")).unwrap();
    }

    #[test]
    fn save_profile_overwrites_in_place_without_leaving_temp_files() {
        isolated_root();
        let name = unique_name("overwrite");
        save_profile(&Profile::new(name.clone(), vec!["a.zip".to_string()])).unwrap();

        let updated = Profile::new(name.clone(), vec!["a.zip".to_string(), "b.zip".to_string()]);
        save_profile(&updated).unwrap();

        assert_eq!(
            get_profile(&name).unwrap().unwrap().installed_mod_names,
            vec!["a.zip".to_string(), "b.zip".to_string()]
        );

        let leftovers: Vec<String> = fs::read_dir(get_profiles_dir().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|file| file.starts_with(&name) && file.ends_with(".tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn get_profile_rejects_malformed_json() {
        isolated_root();
        let name = unique_name("malformed");
        fs::write(get_profile_path(&name).unwrap(), b"{ not a profile").unwrap();

        let err = get_profile(&name).unwrap_err();
        assert!(
            err.to_string().contains("invalid profile JSON"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn save_profile_reports_an_unwritable_name() {
        isolated_root();
        // A name containing a path separator cannot be written: the parent
        // directory of the temp file does not exist.
        let err =
            save_profile(&Profile::new("cov/unwritable/name".to_string(), Vec::new())).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn list_profiles_skips_invalid_entries_and_non_json_files() {
        isolated_root();
        let name = unique_name("listed");
        save_profile(&Profile::new(name.clone(), Vec::new())).unwrap();

        // Malformed JSON is skipped rather than failing the whole listing.
        fs::write(
            get_profile_path(&unique_name("broken")).unwrap(),
            b"not json at all",
        )
        .unwrap();

        // A valid profile stored under another extension is not picked up.
        let other_ext = unique_name("other-ext");
        let json = serde_json::to_string(&Profile::new(other_ext.clone(), Vec::new())).unwrap();
        fs::write(
            get_profile_path(&other_ext).unwrap().with_extension("txt"),
            json,
        )
        .unwrap();

        let listed = list_profiles().unwrap();
        assert!(listed.iter().any(|profile| profile.name == name));
        assert!(!listed.iter().any(|profile| profile.name == other_ext));
    }
}
