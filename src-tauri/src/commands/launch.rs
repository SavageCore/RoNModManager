use tauri::State;

use crate::models::Result;
use crate::services::{desktop_shortcut, launch_args::LaunchRequest, steam_shortcuts};
use crate::state::AppState;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct LaunchArgs(pub Mutex<LaunchRequest>);

#[tauri::command]
pub async fn get_launch_request(state: State<'_, LaunchArgs>) -> Result<LaunchRequest> {
    Ok(state.0.lock().unwrap().clone())
}

/// Headless launch: set active profile (or active), sync + launch, suppress exit cleanup.
#[tauri::command]
pub async fn headless_launch(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    profile: Option<String>,
    vanilla: Option<bool>,
) -> Result<String> {
    use crate::commands::game;
    use crate::services;

    let vanilla = vanilla.unwrap_or(false);
    let groups: Vec<String> = if vanilla {
        Vec::new()
    } else if let Some(name) = profile.clone() {
        let p = services::profiles::get_profile(&name)?.ok_or_else(|| {
            crate::models::AppError::Validation(format!("Profile '{}' not found", name))
        })?;
        state.update_config(|c| {
            c.active_profile = Some(p.name.clone());
        })?;
        p.installed_mod_names.clone()
    } else {
        let cfg = state.get_config()?;
        match cfg.active_profile.clone() {
            Some(n) => services::profiles::get_profile(&n)?
                .map(|p| p.installed_mod_names)
                .unwrap_or_default(),
            None => Vec::new(),
        }
    };
    let config = state.get_config()?;
    let game_path = config.game_path.clone().ok_or_else(|| {
        crate::models::AppError::Validation("Game path not configured".to_string())
    })?;
    game::sync_mod_links_for_game_path(&game_path, groups)
        .map_err(crate::models::AppError::Validation)?;
    game::launch_game_internal_pub(&game_path, config.intro_skip_enabled)
        .map_err(crate::models::AppError::Validation)?;
    if config.link_on_launch_only {
        crate::services::game_watch::spawn_game_exit_watcher(app, game_path);
    }
    state
        .suppress_exit_cleanup
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Ok("launched".to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub desktop_path: Option<PathBuf>,
    pub in_steam: bool,
    pub desktop_content: Option<String>,
}

#[tauri::command]
pub async fn create_profile_shortcut(
    profile: String,
    vanilla: Option<bool>,
    desktop_copy: Option<bool>,
) -> Result<PathBuf> {
    desktop_shortcut::create_desktop_shortcut(
        &profile,
        vanilla.unwrap_or(false),
        desktop_copy.unwrap_or(false),
    )
}

#[tauri::command]
pub async fn remove_profile_shortcut(profile: String) -> Result<bool> {
    desktop_shortcut::remove_desktop_shortcut(&profile)
}

#[tauri::command]
pub async fn add_profile_to_steam(profile: String, vanilla: Option<bool>) -> Result<Vec<PathBuf>> {
    steam_shortcuts::add_to_steam(&profile, vanilla.unwrap_or(false))
}

#[tauri::command]
pub async fn remove_profile_from_steam(profile: String) -> Result<Vec<PathBuf>> {
    steam_shortcuts::remove_from_steam(&profile)
}

#[tauri::command]
pub async fn profile_shortcut_status(profile: String) -> Result<ShortcutStatus> {
    let (desktop_path, _) = desktop_shortcut::shortcut_status(&profile);
    let in_steam = steam_shortcuts::steam_status(&profile).unwrap_or(false);
    // For Flatpak fallback export: render content so UI can offer download/save.
    let desktop_content = if desktop_path.is_none() {
        let exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "ronmodmanager".to_string());
        Some(desktop_shortcut::desktop_file_content(
            &profile, &exe, false,
        ))
    } else {
        None
    };
    Ok(ShortcutStatus {
        desktop_path,
        in_steam,
        desktop_content,
    })
}

#[tauri::command]
pub async fn steam_running() -> Result<bool> {
    Ok(steam_shortcuts::is_steam_running())
}

#[tauri::command]
pub async fn quit_steam() -> Result<String> {
    steam_shortcuts::quit_steam()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{isolated_root, TestApp};
    use std::sync::Mutex;
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

    fn app_with_launch_request(request: LaunchRequest) -> TestApp {
        mock_builder()
            .manage(LaunchArgs(Mutex::new(request)))
            .build(mock_context(noop_assets()))
            .unwrap()
    }

    fn launch_request() -> LaunchRequest {
        LaunchRequest {
            profile: Some("main".to_string()),
            launch: true,
            vanilla: false,
            hide: true,
        }
    }

    #[tokio::test]
    async fn launch_requests_are_returned_from_the_cli_state() {
        let app = app_with_launch_request(launch_request());

        let request = get_launch_request(app.state::<LaunchArgs>()).await.unwrap();

        assert_eq!(request.profile, Some("main".to_string()));
        assert!(request.launch);
        assert!(!request.vanilla);
        assert!(request.hide);
    }

    #[tokio::test]
    async fn shortcuts_are_created_and_removed_on_disk() {
        // Guard against writing to the developer's real desktop directory.
        isolated_root();

        let path = create_profile_shortcut("launch-shortcut".to_string(), None, None)
            .await
            .unwrap();
        assert!(path.starts_with(isolated_root()));
        assert!(path.exists());

        let status = profile_shortcut_status("launch-shortcut".to_string())
            .await
            .unwrap();
        assert_eq!(status.desktop_path, Some(path.clone()));
        assert!(status.desktop_content.is_none());

        assert!(remove_profile_shortcut("launch-shortcut".to_string())
            .await
            .unwrap());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn a_missing_desktop_shortcut_falls_back_to_rendered_content() {
        isolated_root();

        let status = profile_shortcut_status("launch-missing".to_string())
            .await
            .unwrap();

        assert!(status.desktop_path.is_none());
        assert!(!status.in_steam);
        let content = status.desktop_content.unwrap();
        assert!(content.contains("launch-missing"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn adding_to_steam_fails_without_a_steam_install() {
        isolated_root();
        // Other tests create a shortcut file in the same fake tree.
        let _steam = crate::test_support::shared_tree_guard();

        // Either Steam is running (rejected up front) or there is no userdata
        // to write to; both are errors as far as the caller is concerned.
        assert!(add_profile_to_steam("launch-steam".to_string(), None)
            .await
            .is_err());
    }
}
