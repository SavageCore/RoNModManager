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

#[tauri::command]
pub async fn get_broken_mods(state: State<'_, AppState>) -> Result<HashMap<String, String>> {
    let config = state.get_config()?;
    let active_profile_name = match config.active_profile {
        Some(name) => name,
        None => return Ok(HashMap::new()),
    };
    let profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;
    Ok(profile.broken_mods)
}

#[tauri::command]
pub async fn set_mod_broken(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] modName: String,
    note: String,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;
    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;
    profile.broken_mods.insert(modName, note);
    profiles::save_profile(&profile)?;
    Ok(())
}

#[tauri::command]
pub async fn clear_mod_broken(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] modName: String,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;
    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;
    profile.broken_mods.remove(&modName);
    profiles::save_profile(&profile)?;
    Ok(())
}

#[tauri::command]
pub async fn get_no_world_gen_mods(state: State<'_, AppState>) -> Result<Vec<String>> {
    let config = state.get_config()?;
    let active_profile_name = match config.active_profile {
        Some(name) => name,
        None => return Ok(Vec::new()),
    };
    let profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;
    Ok(profile.no_world_gen)
}

#[tauri::command]
pub async fn set_mod_no_world_gen(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] modName: String,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;
    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;
    if !profile.no_world_gen.contains(&modName) {
        profile.no_world_gen.push(modName);
    }
    profiles::save_profile(&profile)?;
    Ok(())
}

#[tauri::command]
pub async fn clear_mod_no_world_gen(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] modName: String,
) -> Result<()> {
    let config = state.get_config()?;
    let active_profile_name = config
        .active_profile
        .ok_or_else(|| crate::models::AppError::Validation("No active profile".to_string()))?;
    let mut profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        crate::models::AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;
    profile.no_world_gen.retain(|n| n != &modName);
    profiles::save_profile(&profile)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppConfig, Profile};
    use crate::services::profiles as profile_service;
    use crate::test_support::{mock_app_with, TestApp};
    use tauri::Manager;

    /// Stores the profile in the isolated app-data dir and returns an app whose
    /// active profile points at it.
    fn app_with_profile(profile: Profile) -> TestApp {
        profile_service::save_profile(&profile).unwrap();
        mock_app_with(AppConfig {
            active_profile: Some(profile.name.clone()),
            ..AppConfig::default()
        })
    }

    fn profile_named(name: &str) -> Profile {
        Profile::new(name.to_string(), vec!["a.zip".to_string()])
    }

    #[tokio::test]
    async fn tags_are_read_from_the_active_profile() {
        let mut profile = profile_named("tags-read");
        profile
            .tags
            .insert("Favourites".to_string(), vec!["a.zip".to_string()]);
        let app = app_with_profile(profile);

        let tags = get_tags(app.state::<AppState>()).await.unwrap();

        assert_eq!(tags.get("Favourites"), Some(&vec!["a.zip".to_string()]));
    }

    #[tokio::test]
    async fn tag_reads_are_empty_without_an_active_profile() {
        let app = mock_app_with(AppConfig::default());

        assert!(get_tags(app.state::<AppState>()).await.unwrap().is_empty());
        assert!(get_broken_mods(app.state::<AppState>())
            .await
            .unwrap()
            .is_empty());
        assert!(get_no_world_gen_mods(app.state::<AppState>())
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn tag_reads_reject_a_missing_active_profile() {
        let app = mock_app_with(AppConfig {
            active_profile: Some("does-not-exist".to_string()),
            ..AppConfig::default()
        });

        assert!(get_tags(app.state::<AppState>()).await.is_err());
        assert!(get_broken_mods(app.state::<AppState>()).await.is_err());
        assert!(get_no_world_gen_mods(app.state::<AppState>())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn set_mod_tags_moves_the_mod_and_prunes_empty_tags() {
        let mut profile = profile_named("tags-write");
        profile
            .tags
            .insert("Old".to_string(), vec!["a.zip".to_string()]);
        profile.tags.insert(
            "Shared".to_string(),
            vec!["a.zip".to_string(), "b.zip".to_string()],
        );
        let app = app_with_profile(profile);
        let state = app.state::<AppState>();

        set_mod_tags(
            state.clone(),
            "a.zip".to_string(),
            vec!["Shared".to_string(), "New".to_string()],
        )
        .await
        .unwrap();

        let tags = get_tags(state).await.unwrap();
        assert!(!tags.contains_key("Old"));
        assert_eq!(tags.get("New"), Some(&vec!["a.zip".to_string()]));
        let shared = tags.get("Shared").unwrap();
        assert!(shared.contains(&"a.zip".to_string()));
        assert!(shared.contains(&"b.zip".to_string()));
    }

    #[tokio::test]
    async fn tag_writes_require_an_active_profile() {
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        assert!(set_mod_tags(state.clone(), "a.zip".to_string(), Vec::new())
            .await
            .is_err());
        assert!(delete_tag(state.clone(), "Old".to_string()).await.is_err());
        assert!(
            set_mod_broken(state.clone(), "a.zip".to_string(), String::new())
                .await
                .is_err()
        );
        assert!(clear_mod_broken(state.clone(), "a.zip".to_string())
            .await
            .is_err());
        assert!(set_mod_no_world_gen(state.clone(), "a.zip".to_string())
            .await
            .is_err());
        assert!(clear_mod_no_world_gen(state.clone(), "a.zip".to_string())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn delete_tag_drops_the_whole_tag() {
        let mut profile = profile_named("tags-delete");
        profile
            .tags
            .insert("Unwanted".to_string(), vec!["a.zip".to_string()]);
        let app = app_with_profile(profile);
        let state = app.state::<AppState>();

        delete_tag(state.clone(), "Unwanted".to_string())
            .await
            .unwrap();

        assert!(get_tags(state).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn broken_mods_round_trip() {
        let app = app_with_profile(profile_named("tags-broken"));
        let state = app.state::<AppState>();

        set_mod_broken(state.clone(), "a.zip".to_string(), "crashes".to_string())
            .await
            .unwrap();
        assert_eq!(
            get_broken_mods(state.clone()).await.unwrap().get("a.zip"),
            Some(&"crashes".to_string())
        );

        clear_mod_broken(state.clone(), "a.zip".to_string())
            .await
            .unwrap();
        assert!(get_broken_mods(state).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_world_gen_mods_are_added_once_and_removed() {
        let app = app_with_profile(profile_named("tags-noworldgen"));
        let state = app.state::<AppState>();

        set_mod_no_world_gen(state.clone(), "map.zip".to_string())
            .await
            .unwrap();
        set_mod_no_world_gen(state.clone(), "map.zip".to_string())
            .await
            .unwrap();
        assert_eq!(
            get_no_world_gen_mods(state.clone()).await.unwrap(),
            vec!["map.zip".to_string()]
        );

        clear_mod_no_world_gen(state.clone(), "map.zip".to_string())
            .await
            .unwrap();
        assert!(get_no_world_gen_mods(state).await.unwrap().is_empty());
    }
}
