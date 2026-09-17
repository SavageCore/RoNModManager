use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
            // A profile can be deleted between the directory scan and the read
            // (the app deletes from the UI thread), so a vanished file is
            // skipped rather than failing the whole listing.
            let content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(AppError::Validation(format!("failed to read profile: {e}"))),
            };
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
    let tmp_path = temp_profile_path(&path);
    fs::write(&tmp_path, &json)
        .map_err(|e| AppError::Validation(format!("failed to write profile to temp file: {e}")))?;
    fs::rename(&tmp_path, &path)
        .map_err(|e| AppError::Validation(format!("failed to replace profile file: {e}")))?;
    Ok(())
}

/// A temp file name unique to this write.
///
/// A fixed `<name>.json.tmp` is shared by every concurrent save of the same
/// profile, and the app does save one profile from more than one place at a time
/// (the UI save and background tag assignment). One writer's rename then moves
/// the temp file the other was about to rename, and the loser fails with ENOENT.
fn temp_profile_path(path: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let suffix = format!(
        "json.{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    path.with_extension(suffix)
}

/// Replaces the tags a mod carries: drops it from every tag it currently has,
/// prunes tags left empty, then adds it to each of `tags`, creating missing
/// ones.
pub fn set_mod_tags(profile: &mut Profile, mod_name: &str, tags: &[String]) {
    for members in profile.tags.values_mut() {
        members.retain(|m| m != mod_name);
    }
    profile.tags.retain(|_, members| !members.is_empty());
    for tag in tags {
        profile
            .tags
            .entry(tag.clone())
            .or_default()
            .push(mod_name.to_string());
    }
}

/// Adds one tag to a mod in the active profile, keeping its other tags.
/// Returns true when the profile changed and was saved. A missing or absent
/// active profile is a no-op, not an error: auto-tagging is best-effort.
pub fn add_tag_to_active_profile(
    active_profile: Option<&str>,
    mod_name: &str,
    tag: &str,
) -> Result<bool> {
    let Some(name) = active_profile else {
        return Ok(false);
    };
    let Some(mut profile) = get_profile(name)? else {
        return Ok(false);
    };

    if profile
        .tags
        .get(tag)
        .is_some_and(|members| members.iter().any(|m| m == mod_name))
    {
        return Ok(false);
    }

    profile
        .tags
        .entry(tag.to_string())
        .or_default()
        .push(mod_name.to_string());
    save_profile(&profile)?;
    Ok(true)
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
    fn set_mod_tags_moves_a_mod_and_prunes_emptied_tags() {
        isolated_root();
        let mut profile = Profile::new(unique_name("tag-move"), Vec::new());
        profile
            .tags
            .insert("Old".to_string(), vec!["a.zip".to_string()]);
        profile.tags.insert(
            "Shared".to_string(),
            vec!["a.zip".to_string(), "b.zip".to_string()],
        );

        set_mod_tags(
            &mut profile,
            "a.zip",
            &["Shared".to_string(), "New".to_string()],
        );

        assert!(!profile.tags.contains_key("Old"));
        assert_eq!(profile.tags.get("New"), Some(&vec!["a.zip".to_string()]));
        let shared = profile.tags.get("Shared").unwrap();
        assert!(shared.contains(&"a.zip".to_string()));
        assert!(shared.contains(&"b.zip".to_string()));
    }

    #[test]
    fn add_tag_to_active_profile_adds_once_and_keeps_other_tags() {
        isolated_root();
        let name = unique_name("tag-add");
        let mut profile = Profile::new(name.clone(), vec!["a.zip".to_string()]);
        profile
            .tags
            .insert("Manual".to_string(), vec!["a.zip".to_string()]);
        save_profile(&profile).unwrap();

        assert!(add_tag_to_active_profile(Some(&name), "a.zip", "Maps").unwrap());
        // The second call is a no-op, so callers can tell the profile is settled.
        assert!(!add_tag_to_active_profile(Some(&name), "a.zip", "Maps").unwrap());

        let tags = get_profile(&name).unwrap().unwrap().tags;
        assert_eq!(tags.get("Maps"), Some(&vec!["a.zip".to_string()]));
        assert_eq!(tags.get("Manual"), Some(&vec!["a.zip".to_string()]));
    }

    #[test]
    fn add_tag_to_active_profile_is_a_no_op_without_a_profile() {
        isolated_root();

        assert!(!add_tag_to_active_profile(None, "a.zip", "Maps").unwrap());
        assert!(
            !add_tag_to_active_profile(Some(&unique_name("tag-absent")), "a.zip", "Maps").unwrap()
        );
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

    /// The app saves the same profile from more than one place at a time (the
    /// UI save and background tag assignment both write the active profile), so
    /// concurrent writes to one name have to all succeed.
    #[test]
    fn concurrent_saves_of_one_profile_all_succeed() {
        isolated_root();
        let name = unique_name("concurrent");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let name = name.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    save_profile(&Profile::new(name, vec![format!("{i}.zip")]))
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap().unwrap();
        }

        assert!(get_profile(&name).unwrap().is_some());
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
