use serde::Deserialize;
use tauri::State;

use crate::models::config::{
    AppConfig, CloseAction, LogLevel, MinimizeTarget, OnGameLaunchAction, ThemeMode,
};
use crate::models::Result;
use crate::services::nexus_api;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub nexus_api_key: Option<String>,
    pub modio_api_key: Option<String>,
    pub modio_game_id: Option<u32>,
    pub active_profile: Option<String>,
    pub modpack_url: Option<String>,
    pub modpack_version: Option<String>,
    pub sync_remote_host: Option<String>,
    pub sync_remote_path: Option<String>,
    pub on_game_launch: Option<OnGameLaunchAction>,
    pub close_action: Option<CloseAction>,
    pub minimize_target: Option<MinimizeTarget>,
    pub asked_close_preference: Option<bool>,
    pub setup_wizard_complete: Option<bool>,
    pub log_level: Option<LogLevel>,
    pub link_on_launch_only: Option<bool>,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig> {
    state.get_config()
}

#[tauri::command]
pub async fn update_config(state: State<'_, AppState>, updates: ConfigUpdate) -> Result<()> {
    let mut should_lookup_game_id = false;
    let mut new_api_key: Option<String> = None;
    state.update_config(|config: &mut AppConfig| {
        if let Some(key) = updates.nexus_api_key {
            config.nexus_api_key = if key.is_empty() { None } else { Some(key) };
        }
        match updates.modio_api_key {
            Some(ref key) if key.is_empty() => {
                config.modio_api_key = None;
                config.modio_game_id = None;
            }
            Some(ref key) => {
                let changed = config.modio_api_key.as_deref() != Some(key);
                config.modio_api_key = Some(key.clone());
                if changed {
                    should_lookup_game_id = true;
                    new_api_key = Some(key.clone());
                }
            }
            None => {}
        }
        if let Some(id) = updates.modio_game_id {
            config.modio_game_id = Some(id);
        }
        if let Some(profile) = updates.active_profile {
            config.active_profile = Some(profile);
        }
        if let Some(url) = updates.modpack_url {
            config.modpack_url = Some(url);
        }
        if let Some(version) = updates.modpack_version {
            config.modpack_version = Some(version);
        }
        if let Some(v) = updates.sync_remote_host {
            config.sync_remote_host = if v.is_empty() { None } else { Some(v) };
        }
        if let Some(v) = updates.sync_remote_path {
            config.sync_remote_path = if v.is_empty() { None } else { Some(v) };
        }
        if let Some(v) = updates.on_game_launch {
            config.on_game_launch = v;
        }
        if let Some(v) = updates.close_action {
            config.close_action = v;
        }
        if let Some(v) = updates.minimize_target {
            config.minimize_target = v;
        }
        if let Some(v) = updates.asked_close_preference {
            config.asked_close_preference = v;
        }
        if let Some(v) = updates.setup_wizard_complete {
            config.setup_wizard_complete = v;
        }
        if let Some(level) = updates.log_level {
            config.log_level = level;
            // Apply at runtime so no restart is needed.
            log::set_max_level(level.into());
        }
        if let Some(v) = updates.link_on_launch_only {
            config.link_on_launch_only = v;
        }
    })?;

    // If a new modio_api_key was set, look up and save the game ID for 'readyornot'
    if should_lookup_game_id {
        if let Some(api_key) = new_api_key {
            let client = &state.client;
            // Use the current config's modio_game_id if available
            let config = state.get_config()?;
            let service = crate::services::modio_api::ModioApiService::new(
                client.clone(),
                config.modio_game_id,
            );
            match service.lookup_game_id(&api_key, "readyornot").await {
                Ok(game_id) => {
                    state.update_config(|config| {
                        config.modio_game_id = Some(game_id);
                    })?;
                }
                Err(e) => {
                    eprintln!("[update_config] Failed to look up mod.io game_id: {}", e);
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn verify_nexus_api_key(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] apiKey: String,
) -> Result<bool> {
    let service = nexus_api::NexusApiService::new(state.client.clone());
    match service.get_mod_info(&apiKey, 981).await {
        // Use a known mod ID to verify
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn verify_modio_api_key(
    state: State<'_, AppState>,
    #[allow(non_snake_case)] apiKey: String,
) -> Result<bool> {
    let config = state.get_config()?;
    let service = crate::services::modio_api::ModioApiService::new(
        state.client.clone(),
        config.modio_game_id,
    );
    match service.lookup_game_id(&apiKey, "readyornot").await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn set_theme(state: State<'_, AppState>, theme: ThemeMode) -> Result<()> {
    state.update_config(|config| {
        config.theme = theme;
    })?;
    Ok(())
}

#[tauri::command]
pub async fn apply_intro_skip(state: State<'_, AppState>) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("game path not configured".to_string())
    })?;
    crate::services::config_tweaks::apply_intro_skip(&game_path)?;
    state.update_config(|c| c.intro_skip_enabled = true)?;
    Ok(())
}

#[tauri::command]
pub async fn undo_intro_skip(state: State<'_, AppState>) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("game path not configured".to_string())
    })?;
    crate::services::config_tweaks::undo_intro_skip(&game_path)?;
    state.update_config(|c| c.intro_skip_enabled = false)?;
    Ok(())
}

#[tauri::command]
pub async fn is_intro_skip_applied(state: State<'_, AppState>) -> Result<bool> {
    let config = state.get_config()?;
    Ok(config.intro_skip_enabled)
}

#[tauri::command]
pub async fn get_gpu_profiles() -> Result<Vec<String>> {
    Ok(crate::services::config_tweaks::available_gpu_profiles())
}

#[tauri::command]
pub async fn detect_gpu_profile() -> Result<Option<String>> {
    Ok(crate::services::config_tweaks::detect_gpu())
}

#[tauri::command]
pub async fn apply_optimization(state: State<'_, AppState>, profile: String) -> Result<()> {
    crate::services::config_tweaks::apply_optimization(&profile)?;
    state.update_config(|c| {
        c.optimization_enabled = true;
        c.optimization_profile = Some(profile);
    })?;
    Ok(())
}

#[tauri::command]
pub async fn get_applied_optimization_profile() -> Result<Option<String>> {
    Ok(crate::services::config_tweaks::detect_applied_profile())
}

#[tauri::command]
pub async fn disable_optimization(state: State<'_, AppState>) -> Result<()> {
    crate::services::config_tweaks::restore_optimization()?;
    state.update_config(|c| {
        c.optimization_enabled = false;
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::ThemeMode;
    use crate::services::config_tweaks;
    use crate::test_support::{mock_app_with, scratch_dir};
    use tauri::Manager;

    fn blank_update() -> ConfigUpdate {
        ConfigUpdate {
            nexus_api_key: None,
            modio_api_key: None,
            modio_game_id: None,
            active_profile: None,
            modpack_url: None,
            modpack_version: None,
            sync_remote_host: None,
            sync_remote_path: None,
            on_game_launch: None,
            close_action: None,
            minimize_target: None,
            asked_close_preference: None,
            setup_wizard_complete: None,
            log_level: None,
            link_on_launch_only: None,
        }
    }

    /// A game install tree with the intro movies in place, so the intro-skip
    /// commands have something real to rename.
    fn fake_game_dir() -> tempfile::TempDir {
        let dir = scratch_dir("game");
        let movies = dir.path().join("ReadyOrNot/Content/Movies");
        std::fs::create_dir_all(&movies).unwrap();
        std::fs::write(movies.join("ReadyOrNot_StartupMovie.mp4"), b"movie").unwrap();
        dir
    }

    #[tokio::test]
    async fn get_config_returns_the_managed_config() {
        let app = mock_app_with(AppConfig {
            active_profile: Some("main".to_string()),
            ..AppConfig::default()
        });
        let config = get_config(app.state::<AppState>()).await.unwrap();
        assert_eq!(config.active_profile, Some("main".to_string()));
    }

    #[tokio::test]
    async fn update_config_clears_blank_api_keys_and_game_id() {
        let app = mock_app_with(AppConfig {
            nexus_api_key: Some("nexus-key".to_string()),
            modio_api_key: Some("modio-key".to_string()),
            modio_game_id: Some(42),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();

        let updates = ConfigUpdate {
            nexus_api_key: Some(String::new()),
            modio_api_key: Some(String::new()),
            ..blank_update()
        };
        update_config(state.clone(), updates).await.unwrap();

        let config = state.get_config().unwrap();
        assert_eq!(config.nexus_api_key, None);
        assert_eq!(config.modio_api_key, None);
        assert_eq!(config.modio_game_id, None);
    }

    #[tokio::test]
    async fn update_config_uses_the_existing_modio_game_id_for_an_unchanged_key() {
        let app = mock_app_with(AppConfig {
            modio_api_key: Some("modio-key".to_string()),
            modio_game_id: Some(42),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();

        let updates = ConfigUpdate {
            modio_api_key: Some("modio-key".to_string()),
            ..blank_update()
        };
        update_config(state.clone(), updates).await.unwrap();

        // An unchanged key must not trigger the game-id lookup.
        let config = state.get_config().unwrap();
        assert_eq!(config.modio_api_key, Some("modio-key".to_string()));
        assert_eq!(config.modio_game_id, Some(42));
    }

    #[tokio::test]
    async fn update_config_applies_every_ui_preference() {
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        let updates = ConfigUpdate {
            active_profile: Some("profile-a".to_string()),
            modpack_url: Some("https://example.com/modpack.json".to_string()),
            modpack_version: Some("1.2.3".to_string()),
            modio_game_id: Some(9),
            sync_remote_host: Some("deploy@example.com".to_string()),
            sync_remote_path: Some("/srv/mods".to_string()),
            on_game_launch: Some(OnGameLaunchAction::Minimize),
            close_action: Some(CloseAction::Minimize),
            minimize_target: Some(MinimizeTarget::Tray),
            asked_close_preference: Some(true),
            setup_wizard_complete: Some(true),
            log_level: Some(LogLevel::Debug),
            link_on_launch_only: Some(true),
            ..blank_update()
        };
        update_config(state.clone(), updates).await.unwrap();

        let config = state.get_config().unwrap();
        assert_eq!(config.active_profile, Some("profile-a".to_string()));
        assert_eq!(
            config.modpack_url,
            Some("https://example.com/modpack.json".to_string())
        );
        assert_eq!(config.modpack_version, Some("1.2.3".to_string()));
        assert_eq!(config.modio_game_id, Some(9));
        assert_eq!(
            config.sync_remote_host,
            Some("deploy@example.com".to_string())
        );
        assert_eq!(config.sync_remote_path, Some("/srv/mods".to_string()));
        assert!(matches!(
            config.on_game_launch,
            OnGameLaunchAction::Minimize
        ));
        assert!(matches!(config.close_action, CloseAction::Minimize));
        assert!(matches!(config.minimize_target, MinimizeTarget::Tray));
        assert!(config.asked_close_preference);
        assert!(config.setup_wizard_complete);
        assert_eq!(config.log_level, LogLevel::Debug);
        assert!(config.link_on_launch_only);
    }

    #[tokio::test]
    async fn update_config_clears_sync_fields_when_blank() {
        let app = mock_app_with(AppConfig {
            sync_remote_host: Some("deploy@example.com".to_string()),
            sync_remote_path: Some("/srv/mods".to_string()),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();

        let updates = ConfigUpdate {
            sync_remote_host: Some(String::new()),
            sync_remote_path: Some(String::new()),
            ..blank_update()
        };
        update_config(state.clone(), updates).await.unwrap();

        let config = state.get_config().unwrap();
        assert_eq!(config.sync_remote_host, None);
        assert_eq!(config.sync_remote_path, None);
    }

    #[tokio::test]
    async fn update_config_leaves_untouched_fields_alone() {
        let app = mock_app_with(AppConfig {
            nexus_api_key: Some("kept".to_string()),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();

        update_config(state.clone(), blank_update()).await.unwrap();

        assert_eq!(
            state.get_config().unwrap().nexus_api_key,
            Some("kept".to_string())
        );
    }

    #[tokio::test]
    async fn set_theme_persists_the_theme() {
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        set_theme(state.clone(), ThemeMode::Dark).await.unwrap();

        assert!(matches!(state.get_config().unwrap().theme, ThemeMode::Dark));
    }

    #[tokio::test]
    async fn intro_skip_toggles_with_the_game_tree() {
        let game = fake_game_dir();
        let app = mock_app_with(AppConfig {
            game_path: Some(game.path().to_path_buf()),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();
        let movies = game.path().join("ReadyOrNot/Content/Movies");

        assert!(!is_intro_skip_applied(state.clone()).await.unwrap());

        apply_intro_skip(state.clone()).await.unwrap();
        assert!(is_intro_skip_applied(state.clone()).await.unwrap());
        assert!(movies.join("ReadyOrNot_StartupMovie.mp4.bak").exists());

        undo_intro_skip(state.clone()).await.unwrap();
        assert!(!is_intro_skip_applied(state.clone()).await.unwrap());
        assert!(movies.join("ReadyOrNot_StartupMovie.mp4").exists());
    }

    #[tokio::test]
    async fn intro_skip_requires_a_configured_game_path() {
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        assert!(apply_intro_skip(state.clone()).await.is_err());
        assert!(undo_intro_skip(state.clone()).await.is_err());
    }

    #[tokio::test]
    async fn gpu_profiles_are_listed() {
        let profiles = get_gpu_profiles().await.unwrap();
        assert!(profiles.contains(&"RTX_4090".to_string()));
    }

    #[tokio::test]
    async fn apply_optimization_rejects_an_unknown_profile() {
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        assert!(apply_optimization(state.clone(), "Voodoo_3dfx".to_string())
            .await
            .is_err());
        assert!(!state.get_config().unwrap().optimization_enabled);
    }

    #[tokio::test]
    async fn apply_and_disable_optimization_round_trip_the_engine_ini() {
        let profile = config_tweaks::available_gpu_profiles()
            .first()
            .cloned()
            .unwrap();
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        apply_optimization(state.clone(), profile.clone())
            .await
            .unwrap();

        let config = state.get_config().unwrap();
        assert!(config.optimization_enabled);
        assert_eq!(config.optimization_profile, Some(profile.clone()));
        assert_eq!(
            get_applied_optimization_profile().await.unwrap(),
            Some(profile)
        );

        disable_optimization(state.clone()).await.unwrap();
        assert!(!state.get_config().unwrap().optimization_enabled);
    }
}
