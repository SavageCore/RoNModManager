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
    services::profiles::delete_profile(&name)?;
    // Best-effort: drop stale one-click shortcuts for the deleted profile.
    let _ = services::desktop_shortcut::remove_desktop_shortcut(&name);
    let _ = services::steam_shortcuts::remove_from_steam(&name);
    Ok(())
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
    // Best-effort: move one-click shortcuts to the new name.
    {
        let had_desktop = services::desktop_shortcut::shortcut_status(&old_name)
            .0
            .is_some();
        let had_steam = services::steam_shortcuts::steam_status(&old_name).unwrap_or(false);
        let _ = services::desktop_shortcut::remove_desktop_shortcut(&old_name);
        let _ = services::steam_shortcuts::remove_from_steam(&old_name);
        if had_desktop {
            let _ = services::desktop_shortcut::create_desktop_shortcut(&new_name, false, false);
        }
        if had_steam {
            let _ = services::steam_shortcuts::add_to_steam(&new_name, false);
        }
    }
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

/// Resolved against a plain `&AppState` (rather than Tauri's `State`) so the
/// fallback logic stays unit-testable without an app handle.
pub fn resolve_sync_details(state: &AppState) -> Result<(Option<String>, Option<String>)> {
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

    // When link-on-launch-only is enabled, applying a profile only updates the
    // active profile; the game folder is (un)linked at launch time.
    if !updated_config.link_on_launch_only {
        if let Some(ref game_path) = updated_config.game_path {
            game::sync_mod_links_for_game_path(game_path, profile.installed_mod_names.clone())
                .map_err(AppError::Validation)?;
        }
    }

    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppConfig;
    use reqwest::Client;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicU64};
    use std::sync::{Arc, Mutex, RwLock};
    use tempfile::TempDir;

    fn state_with_config(config: AppConfig) -> (TempDir, AppState) {
        let dir = TempDir::new().unwrap();
        let state = AppState {
            config: RwLock::new(config),
            client: Client::new(),
            config_path: dir.path().join("config.json"),
            nexus_cancel: Arc::new(Mutex::new(HashMap::new())),
            nexus_wait_id: Arc::new(AtomicU64::new(1)),
            nexus_open_throttle: Arc::new(Mutex::new(HashMap::new())),
            suppress_exit_cleanup: AtomicBool::new(false),
            game_watcher_running: AtomicBool::new(false),
        };
        (dir, state)
    }

    #[test]
    fn normalize_trims_and_drops_empties() {
        assert_eq!(
            normalize(Some("  host  ".to_string())),
            Some("host".to_string())
        );
        assert_eq!(normalize(Some("   ".to_string())), None);
        assert_eq!(normalize(Some(String::new())), None);
        assert_eq!(normalize(None), None);
    }

    #[test]
    fn resolve_sync_details_falls_back_to_global_without_active_profile() {
        let config = AppConfig {
            sync_remote_host: Some("deploy@example.com".to_string()),
            sync_remote_path: Some("/srv/mods".to_string()),
            ..AppConfig::default()
        };
        let (_dir, state) = state_with_config(config);

        assert_eq!(
            resolve_sync_details(&state).unwrap(),
            (
                Some("deploy@example.com".to_string()),
                Some("/srv/mods".to_string())
            )
        );
    }

    #[test]
    fn resolve_sync_details_returns_none_when_nothing_configured() {
        let (_dir, state) = state_with_config(AppConfig::default());
        assert_eq!(resolve_sync_details(&state).unwrap(), (None, None));
    }

    mod commands {
        use super::*;
        use crate::services::profiles as profile_service;
        use crate::test_support::{isolated_root, mock_app_with, TestApp};
        use tauri::Manager;

        /// Saves a profile, making sure the path redirect is in place first.
        fn store(profile: &Profile) {
            isolated_root();
            profile_service::save_profile(profile).unwrap();
        }

        /// A profile stored in the isolated app-data dir, plus an app whose
        /// active profile points at it.
        fn app_with_profile(name: &str) -> TestApp {
            isolated_root();
            profile_service::save_profile(&Profile::new(name.to_string(), Vec::new())).unwrap();
            mock_app_with(AppConfig {
                active_profile: Some(name.to_string()),
                ..AppConfig::default()
            })
        }

        #[tokio::test]
        async fn listing_profiles_creates_the_default_and_activates_it() {
            let app = mock_app_with(AppConfig::default());
            let state = app.state::<AppState>();

            let profiles = list_profiles(state.clone()).await.unwrap();

            assert!(profiles.iter().any(|p| p.name == "Default"));
            assert_eq!(
                state.get_config().unwrap().active_profile,
                Some("Default".to_string())
            );
        }

        #[tokio::test]
        async fn listing_profiles_switches_away_from_a_stale_active_profile() {
            let app = mock_app_with(AppConfig {
                active_profile: Some("deleted-profile".to_string()),
                ..AppConfig::default()
            });
            let state = app.state::<AppState>();

            list_profiles(state.clone()).await.unwrap();

            assert_eq!(
                state.get_config().unwrap().active_profile,
                Some("Default".to_string())
            );
        }

        #[tokio::test]
        async fn profiles_are_read_and_written_by_name() {
            isolated_root();

            assert!(get_profile("prof-round-trip".to_string())
                .await
                .unwrap()
                .is_none());

            let saved = save_profile(
                "prof-round-trip".to_string(),
                Some("for testing".to_string()),
                vec!["a.zip".to_string()],
            )
            .await
            .unwrap();
            assert_eq!(saved.description, Some("for testing".to_string()));

            let loaded = get_profile("prof-round-trip".to_string())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(loaded.installed_mod_names, vec!["a.zip".to_string()]);

            delete_profile("prof-round-trip".to_string()).await.unwrap();
            assert!(get_profile("prof-round-trip".to_string())
                .await
                .unwrap()
                .is_none());
        }

        #[tokio::test]
        async fn saving_over_a_profile_keeps_its_collections_and_tags() {
            let mut existing = Profile::new("prof-keep".to_string(), Vec::new());
            existing.enabled_collections = vec!["Favourites".to_string()];
            existing
                .collections
                .insert("Favourites".to_string(), vec!["a.zip".to_string()]);
            existing
                .tags
                .insert("Broken".to_string(), vec!["b.zip".to_string()]);
            existing
                .collection_colors
                .insert("Favourites".to_string(), "#fff".to_string());
            existing
                .broken_mods
                .insert("b.zip".to_string(), "note".to_string());
            existing.no_world_gen = vec!["map.zip".to_string()];
            existing.sync_remote_host = Some("deploy@example.com".to_string());
            let created_at = existing.created_at.clone();
            store(&existing);

            let saved = save_profile("prof-keep".to_string(), None, vec!["c.zip".to_string()])
                .await
                .unwrap();

            assert_eq!(saved.enabled_collections, vec!["Favourites".to_string()]);
            assert_eq!(
                saved.collections.get("Favourites"),
                Some(&vec!["a.zip".to_string()])
            );
            assert_eq!(saved.tags.get("Broken"), Some(&vec!["b.zip".to_string()]));
            assert_eq!(
                saved.collection_colors.get("Favourites"),
                Some(&"#fff".to_string())
            );
            assert_eq!(saved.broken_mods.get("b.zip"), Some(&"note".to_string()));
            assert_eq!(saved.no_world_gen, vec!["map.zip".to_string()]);
            assert_eq!(saved.created_at, created_at);
            assert_eq!(saved.installed_mod_names, vec!["c.zip".to_string()]);
        }

        #[tokio::test]
        async fn renaming_a_profile_moves_the_file_and_the_active_pointer() {
            let app = app_with_profile("prof-rename-old");
            let state = app.state::<AppState>();

            let renamed = rename_profile(
                "  prof-rename-old  ".to_string(),
                "  prof-rename-new  ".to_string(),
                Some("renamed".to_string()),
                vec!["a.zip".to_string()],
                state.clone(),
            )
            .await
            .unwrap();

            assert_eq!(renamed.name, "prof-rename-new");
            assert!(profile_service::get_profile("prof-rename-old")
                .unwrap()
                .is_none());
            assert!(profile_service::get_profile("prof-rename-new")
                .unwrap()
                .is_some());
            assert_eq!(
                state.get_config().unwrap().active_profile,
                Some("prof-rename-new".to_string())
            );
        }

        #[tokio::test]
        async fn renaming_rejects_bad_input() {
            let app = app_with_profile("prof-rename-guard");
            let state = app.state::<AppState>();
            store(&Profile::new("prof-rename-taken".to_string(), Vec::new()));

            let same = rename_profile(
                "prof-rename-guard".to_string(),
                "prof-rename-guard".to_string(),
                None,
                Vec::new(),
                state.clone(),
            )
            .await;
            assert!(same.unwrap_err().to_string().contains("must be different"));

            let empty = rename_profile(
                "  ".to_string(),
                "somewhere".to_string(),
                None,
                Vec::new(),
                state.clone(),
            )
            .await;
            assert!(empty.unwrap_err().to_string().contains("is required"));

            let taken = rename_profile(
                "prof-rename-guard".to_string(),
                "prof-rename-taken".to_string(),
                None,
                Vec::new(),
                state.clone(),
            )
            .await;
            assert!(taken.unwrap_err().to_string().contains("already exists"));

            let missing = rename_profile(
                "nope".to_string(),
                "prof-rename-fresh".to_string(),
                None,
                Vec::new(),
                state,
            )
            .await;
            assert!(missing.unwrap_err().to_string().contains("not found"));
        }

        #[tokio::test]
        async fn duplicating_a_profile_copies_it_under_a_new_name() {
            let mut source = Profile::new("prof-dup-src".to_string(), vec!["a.zip".to_string()]);
            source
                .collections
                .insert("Favourites".to_string(), vec!["a.zip".to_string()]);
            store(&source);

            let copy = duplicate_profile("prof-dup-src".to_string(), "prof-dup-copy".to_string())
                .await
                .unwrap();

            assert_eq!(copy.name, "prof-dup-copy");
            assert_eq!(copy.installed_mod_names, vec!["a.zip".to_string()]);
            assert!(copy.collections.contains_key("Favourites"));
            assert!(profile_service::get_profile("prof-dup-src")
                .unwrap()
                .is_some());
        }

        #[tokio::test]
        async fn duplicating_rejects_bad_input() {
            store(&Profile::new("prof-dup-guard".to_string(), Vec::new()));
            store(&Profile::new("prof-dup-taken".to_string(), Vec::new()));

            let same = duplicate_profile("a".to_string(), "a".to_string()).await;
            assert!(same.unwrap_err().to_string().contains("must be different"));

            let empty = duplicate_profile("a".to_string(), "  ".to_string()).await;
            assert!(empty.unwrap_err().to_string().contains("is required"));

            let taken =
                duplicate_profile("prof-dup-guard".to_string(), "prof-dup-taken".to_string()).await;
            assert!(taken.unwrap_err().to_string().contains("already exists"));

            let missing = duplicate_profile("nope".to_string(), "prof-dup-fresh".to_string()).await;
            assert!(missing.unwrap_err().to_string().contains("not found"));
        }

        #[tokio::test]
        async fn modpack_meta_is_read_from_and_written_to_the_active_profile() {
            let app = app_with_profile("prof-meta");
            let state = app.state::<AppState>();

            assert!(get_modpack_meta(state.clone()).await.unwrap().is_none());

            set_modpack_meta(
                "My Pack".to_string(),
                "2.0.0".to_string(),
                Some("notes".to_string()),
                Some("SavageCore".to_string()),
                state.clone(),
            )
            .await
            .unwrap();

            let meta = get_modpack_meta(state).await.unwrap().unwrap();
            assert_eq!(meta.name, "My Pack");
            assert_eq!(meta.version, "2.0.0");
            assert_eq!(meta.description, "notes");
            assert_eq!(meta.author, Some("SavageCore".to_string()));
        }

        #[tokio::test]
        async fn modpack_meta_reads_are_safe_without_an_active_profile() {
            let app = mock_app_with(AppConfig::default());

            assert!(get_modpack_meta(app.state::<AppState>())
                .await
                .unwrap()
                .is_none());
            assert!(set_modpack_meta(
                "pack".to_string(),
                "1.0.0".to_string(),
                None,
                None,
                app.state::<AppState>(),
            )
            .await
            .is_err());
        }

        #[tokio::test]
        async fn sync_details_prefer_the_profile_then_fall_back_to_the_config() {
            let app = app_with_profile("prof-sync");
            let state = app.state::<AppState>();
            state
                .update_config(|config| {
                    config.sync_remote_host = Some("global@example.com".to_string());
                    config.sync_remote_path = Some("/global".to_string());
                })
                .unwrap();

            let inherited = get_sync_details(state.clone()).await.unwrap();
            assert_eq!(inherited.host, Some("global@example.com".to_string()));
            assert_eq!(inherited.path, Some("/global".to_string()));

            set_sync_details(
                Some("  deploy@example.com ".to_string()),
                Some("/srv/mods".to_string()),
                state.clone(),
            )
            .await
            .unwrap();

            let overridden = get_sync_details(state.clone()).await.unwrap();
            assert_eq!(overridden.host, Some("deploy@example.com".to_string()));
            assert_eq!(overridden.path, Some("/srv/mods".to_string()));

            // Clearing the input falls back to the global config again.
            set_sync_details(Some("   ".to_string()), None, state.clone())
                .await
                .unwrap();
            let cleared = get_sync_details(state).await.unwrap();
            assert_eq!(cleared.host, Some("global@example.com".to_string()));
            assert_eq!(cleared.path, Some("/global".to_string()));
        }

        #[tokio::test]
        async fn sync_details_writes_require_an_active_profile() {
            let app = mock_app_with(AppConfig::default());

            assert!(set_sync_details(None, None, app.state::<AppState>())
                .await
                .is_err());
        }

        #[tokio::test]
        async fn applying_a_profile_activates_it() {
            store(&Profile::new(
                "prof-apply".to_string(),
                vec!["a.zip".to_string()],
            ));
            let app = mock_app_with(AppConfig::default());
            let state = app.state::<AppState>();

            let applied = apply_profile("prof-apply".to_string(), state.clone())
                .await
                .unwrap();

            assert_eq!(applied.name, "prof-apply");
            assert_eq!(
                state.get_config().unwrap().active_profile,
                Some("prof-apply".to_string())
            );
        }

        #[tokio::test]
        async fn applying_an_unknown_profile_is_rejected() {
            let app = mock_app_with(AppConfig::default());

            assert!(apply_profile("nope".to_string(), app.state::<AppState>())
                .await
                .is_err());
        }
    }
}
