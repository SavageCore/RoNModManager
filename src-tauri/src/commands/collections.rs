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
    profile.collection_colors.remove(&name);
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

    if let Some(color) = profile.collection_colors.remove(&oldName) {
        profile.collection_colors.insert(newName.clone(), color);
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
pub async fn get_collection_colors(state: State<'_, AppState>) -> Result<HashMap<String, String>> {
    let config = state.get_config()?;
    let active_profile_name = match config.active_profile {
        Some(name) => name,
        None => return Ok(HashMap::new()),
    };

    let profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    Ok(profile.collection_colors)
}

#[tauri::command]
pub async fn set_collection_color(
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;

    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    match color {
        Some(c) => {
            profile.collection_colors.insert(name, c);
        }
        None => {
            profile.collection_colors.remove(&name);
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

    // When link-on-launch-only is enabled, toggling a collection only updates the
    // profile; the game folder is (un)linked at launch time.
    if !config.link_on_launch_only {
        if let Some(ref game_path) = config.game_path {
            super::game::sync_mod_links_for_game_path(
                game_path,
                profile.installed_mod_names.clone(),
            )
            .map_err(crate::models::AppError::Validation)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Profile;

    fn profile_with_collections() -> crate::models::Profile {
        let mut profile = Profile::new("test".to_string(), Vec::new());
        profile.collections.insert(
            "Favourites".to_string(),
            vec!["a.zip".to_string(), "b.zip".to_string()],
        );
        profile
            .collections
            .insert("Maps".to_string(), vec!["c.zip".to_string()]);
        profile
    }

    #[test]
    fn enabling_collection_adds_its_mods() {
        let mut profile = profile_with_collections();
        apply_collection_state(&mut profile, "Favourites", true);
        let mut installed = profile.installed_mod_names.clone();
        installed.sort();
        assert_eq!(installed, vec!["a.zip".to_string(), "b.zip".to_string()]);
        assert!(profile
            .enabled_collections
            .contains(&"Favourites".to_string()));
    }

    #[test]
    fn enabling_second_collection_unions_mods() {
        let mut profile = profile_with_collections();
        apply_collection_state(&mut profile, "Favourites", true);
        apply_collection_state(&mut profile, "Maps", true);
        let mut installed = profile.installed_mod_names.clone();
        installed.sort();
        assert_eq!(
            installed,
            vec![
                "a.zip".to_string(),
                "b.zip".to_string(),
                "c.zip".to_string()
            ]
        );
    }

    #[test]
    fn disabling_collection_removes_its_mods() {
        let mut profile = profile_with_collections();
        apply_collection_state(&mut profile, "Favourites", true);
        apply_collection_state(&mut profile, "Favourites", false);
        assert!(profile.installed_mod_names.is_empty());
        assert!(!profile
            .enabled_collections
            .contains(&"Favourites".to_string()));
    }

    #[test]
    fn enabling_unknown_collection_is_stable() {
        let mut profile = profile_with_collections();
        apply_collection_state(&mut profile, "Missing", true);
        assert!(profile.installed_mod_names.is_empty());
        assert!(profile.enabled_collections.contains(&"Missing".to_string()));
    }

    mod commands {
        use super::*;
        use crate::models::AppConfig;
        use crate::services::profiles as profile_service;
        use crate::test_support::{mock_app_with, TestApp};
        use tauri::Manager;

        /// Stores the profile in the isolated app-data dir and returns an app
        /// whose active profile points at it.
        fn app_with_profile(profile: Profile) -> TestApp {
            profile_service::save_profile(&profile).unwrap();
            mock_app_with(AppConfig {
                active_profile: Some(profile.name.clone()),
                ..AppConfig::default()
            })
        }

        fn app_with_collections(name: &str) -> TestApp {
            app_with_profile(profile_with_collections_named(name))
        }

        fn profile_with_collections_named(name: &str) -> Profile {
            let mut profile = Profile::new(name.to_string(), Vec::new());
            profile.collections.insert(
                "Favourites".to_string(),
                vec!["a.zip".to_string(), "b.zip".to_string()],
            );
            profile
                .collections
                .insert("Maps".to_string(), vec!["c.zip".to_string()]);
            profile
        }

        #[tokio::test]
        async fn collections_are_listed_with_their_enabled_state() {
            let app = app_with_collections("coll-list");
            profile_service::save_profile(&Profile {
                enabled_collections: vec!["Maps".to_string()],
                ..profile_with_collections_named("coll-list")
            })
            .unwrap();

            let collections = get_collections(app.state::<AppState>()).await.unwrap();

            assert_eq!(collections.get("Favourites"), Some(&false));
            assert_eq!(collections.get("Maps"), Some(&true));
        }

        #[tokio::test]
        async fn collection_reads_are_empty_without_an_active_profile() {
            let app = mock_app_with(AppConfig::default());

            assert!(get_collections(app.state::<AppState>())
                .await
                .unwrap()
                .is_empty());
            assert!(get_collection_mods(app.state::<AppState>())
                .await
                .unwrap()
                .is_empty());
            assert!(get_collection_colors(app.state::<AppState>())
                .await
                .unwrap()
                .is_empty());
        }

        #[tokio::test]
        async fn collection_writes_require_an_existing_profile() {
            let app = mock_app_with(AppConfig {
                active_profile: Some("coll-missing".to_string()),
                ..AppConfig::default()
            });
            let state = app.state::<AppState>();

            assert!(
                create_collection(state.clone(), "New".to_string(), Vec::new())
                    .await
                    .is_err()
            );
            assert!(delete_collection(state.clone(), "New".to_string())
                .await
                .is_err());
            assert!(set_collection_color(state.clone(), "New".to_string(), None)
                .await
                .is_err());
            assert!(toggle_collection(state.clone(), "New".to_string(), true)
                .await
                .is_err());
        }

        #[tokio::test]
        async fn collection_writes_require_an_active_profile() {
            let app = mock_app_with(AppConfig::default());
            let state = app.state::<AppState>();

            assert!(
                create_collection(state.clone(), "New".to_string(), Vec::new())
                    .await
                    .is_err()
            );
            assert!(remove_mod_from_collection(
                state.clone(),
                "New".to_string(),
                "a.zip".to_string()
            )
            .await
            .is_err());
            assert!(
                rename_collection(state.clone(), "Old".to_string(), "New".to_string())
                    .await
                    .is_err()
            );
        }

        #[tokio::test]
        async fn collections_are_created_and_filled() {
            let app = app_with_collections("coll-create");
            let state = app.state::<AppState>();

            create_collection(state.clone(), "New".to_string(), vec!["d.zip".to_string()])
                .await
                .unwrap();
            assert_eq!(
                get_collection_mods(state.clone()).await.unwrap().get("New"),
                Some(&vec!["d.zip".to_string()])
            );

            add_mod_to_collection(state.clone(), "New".to_string(), "e.zip".to_string())
                .await
                .unwrap();
            // Adding the same mod twice is a no-op.
            add_mod_to_collection(state.clone(), "New".to_string(), "e.zip".to_string())
                .await
                .unwrap();
            // Adding to a collection that does not exist is a no-op too.
            add_mod_to_collection(state.clone(), "Nope".to_string(), "f.zip".to_string())
                .await
                .unwrap();

            let mods = get_collection_mods(state.clone()).await.unwrap();
            assert_eq!(
                mods.get("New"),
                Some(&vec!["d.zip".to_string(), "e.zip".to_string()])
            );
            assert!(!mods.contains_key("Nope"));

            remove_mod_from_collection(state.clone(), "New".to_string(), "d.zip".to_string())
                .await
                .unwrap();
            assert_eq!(
                get_collection_mods(state).await.unwrap().get("New"),
                Some(&vec!["e.zip".to_string()])
            );
        }

        #[tokio::test]
        async fn deleting_a_collection_also_drops_its_colour_and_enabled_flag() {
            let app = app_with_collections("coll-delete");
            let state = app.state::<AppState>();

            set_collection_color(
                state.clone(),
                "Maps".to_string(),
                Some("#ff0000".to_string()),
            )
            .await
            .unwrap();
            toggle_collection(state.clone(), "Maps".to_string(), true)
                .await
                .unwrap();

            delete_collection(state.clone(), "Maps".to_string())
                .await
                .unwrap();

            assert!(!get_collection_mods(state.clone())
                .await
                .unwrap()
                .contains_key("Maps"));
            assert!(!get_collection_colors(state.clone())
                .await
                .unwrap()
                .contains_key("Maps"));
            assert_eq!(
                get_collections(state).await.unwrap().get("Favourites"),
                Some(&false)
            );
        }

        #[tokio::test]
        async fn renaming_a_collection_moves_its_mods_colour_and_enabled_flag() {
            let app = app_with_collections("coll-rename");
            let state = app.state::<AppState>();

            set_collection_color(
                state.clone(),
                "Maps".to_string(),
                Some("#00ff00".to_string()),
            )
            .await
            .unwrap();
            toggle_collection(state.clone(), "Maps".to_string(), true)
                .await
                .unwrap();

            rename_collection(state.clone(), "Maps".to_string(), "Levels".to_string())
                .await
                .unwrap();

            let mods = get_collection_mods(state.clone()).await.unwrap();
            assert!(!mods.contains_key("Maps"));
            assert_eq!(mods.get("Levels"), Some(&vec!["c.zip".to_string()]));
            assert_eq!(
                get_collection_colors(state.clone())
                    .await
                    .unwrap()
                    .get("Levels"),
                Some(&"#00ff00".to_string())
            );
            assert_eq!(
                get_collections(state).await.unwrap().get("Levels"),
                Some(&true)
            );
        }

        #[tokio::test]
        async fn renaming_onto_an_existing_name_is_rejected() {
            let app = app_with_collections("coll-rename-clash");
            let state = app.state::<AppState>();

            assert!(
                rename_collection(state.clone(), "Maps".to_string(), "Favourites".to_string())
                    .await
                    .is_err()
            );
            assert!(
                rename_collection(state.clone(), "Missing".to_string(), "Levels".to_string())
                    .await
                    .is_err()
            );
        }

        #[tokio::test]
        async fn collection_colours_are_set_and_cleared() {
            let app = app_with_collections("coll-colour");
            let state = app.state::<AppState>();

            set_collection_color(
                state.clone(),
                "Maps".to_string(),
                Some("#123456".to_string()),
            )
            .await
            .unwrap();
            assert_eq!(
                get_collection_colors(state.clone())
                    .await
                    .unwrap()
                    .get("Maps"),
                Some(&"#123456".to_string())
            );

            set_collection_color(state.clone(), "Maps".to_string(), None)
                .await
                .unwrap();
            assert!(get_collection_colors(state).await.unwrap().is_empty());
        }

        #[tokio::test]
        async fn toggling_a_collection_updates_the_installed_mods() {
            let app = app_with_collections("coll-toggle");
            let state = app.state::<AppState>();

            toggle_collection(state.clone(), "Maps".to_string(), true)
                .await
                .unwrap();
            toggle_collection(state.clone(), "Favourites".to_string(), true)
                .await
                .unwrap();

            let profile = profile_service::get_profile("coll-toggle")
                .unwrap()
                .unwrap();
            let mut installed = profile.installed_mod_names.clone();
            installed.sort();
            assert_eq!(
                installed,
                vec![
                    "a.zip".to_string(),
                    "b.zip".to_string(),
                    "c.zip".to_string()
                ]
            );

            toggle_collection(state.clone(), "Maps".to_string(), false)
                .await
                .unwrap();
            let profile = profile_service::get_profile("coll-toggle")
                .unwrap()
                .unwrap();
            let mut installed = profile.installed_mod_names.clone();
            installed.sort();
            assert_eq!(installed, vec!["a.zip".to_string(), "b.zip".to_string()]);
        }
    }
}
