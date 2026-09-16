use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

pub struct StartupUrls(pub Mutex<Option<Vec<String>>>);

#[tauri::command]
pub fn get_startup_urls(state: State<'_, StartupUrls>) -> Vec<String> {
    state.0.lock().unwrap().take().unwrap_or_default()
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

#[tauri::command]
pub fn is_screenshot_mode() -> bool {
    std::env::var("SCREENSHOT_MODE").is_ok()
}

/// When set, forces the setup wizard to show even in screenshot mode, so the
/// welcome page can be captured separately from the main pages.
#[tauri::command]
pub fn is_wizard_screenshot_mode() -> bool {
    std::env::var("WIZARD_SCREENSHOT").is_ok()
}

#[tauri::command]
pub fn screenshot_theme() -> Option<String> {
    std::env::var("SCREENSHOT_THEME").ok()
}

/// Whether the app should manage (persist + restore) its own window geometry.
///
/// Native Wayland clients cannot set an absolute position and the compositor
/// already centres and sizes windows sensibly, so we leave geometry entirely to
/// it there. Forcing XWayland via `GDK_BACKEND=x11`, or running a real X11
/// session, lets us reliably control size and position, so we manage it then.
#[tauri::command]
pub fn manage_window_geometry() -> bool {
    let on_wayland = std::env::var("WAYLAND_DISPLAY")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let forced_x11 = std::env::var("GDK_BACKEND")
        .map(|value| value == "x11")
        .unwrap_or(false);

    !on_wayland || forced_x11
}

#[tauri::command]
pub fn set_window_title(app: AppHandle, title: String) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Failed to get main window")?;

    window
        .set_title(&title)
        .map_err(|e| format!("Failed to set window title: {}", e))
}

#[tauri::command]
pub async fn save_window_state(
    state: State<'_, AppState>,
    width: Option<f64>,
    height: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), String> {
    state
        .update_config(|config| {
            if let Some(value) = width {
                config.window_width = Some(value);
            }
            if let Some(value) = height {
                config.window_height = Some(value);
            }
            if let Some(value) = x {
                config.window_x = Some(value);
            }
            if let Some(value) = y {
                config.window_y = Some(value);
            }
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_window_state(state: State<'_, AppState>) -> Result<WindowState, String> {
    let config = state.get_config().map_err(String::from)?;
    Ok(WindowState {
        width: config.window_width,
        height: config.window_height,
        x: config.window_x,
        y: config.window_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppConfig;
    use crate::test_support::{mock_app_with, test_state};
    use std::sync::Mutex;
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

    #[test]
    fn startup_urls_are_handed_out_once() {
        let app = mock_builder()
            .manage(StartupUrls(Mutex::new(Some(vec![
                "ronmodmanager://mod/1".to_string()
            ]))))
            .build(mock_context(noop_assets()))
            .unwrap();
        let state = app.state::<StartupUrls>();

        assert_eq!(
            get_startup_urls(state.clone()),
            vec!["ronmodmanager://mod/1".to_string()]
        );
        // Taken, so a second read must not replay the same launch.
        assert!(get_startup_urls(state).is_empty());
    }

    #[test]
    fn startup_urls_default_to_empty_when_never_set() {
        let app = mock_builder()
            .manage(StartupUrls(Mutex::new(None)))
            .build(mock_context(noop_assets()))
            .unwrap();

        assert!(get_startup_urls(app.state::<StartupUrls>()).is_empty());
    }

    #[test]
    fn screenshot_env_vars_are_reported() {
        assert!(!is_screenshot_mode());
        assert!(!is_wizard_screenshot_mode());
        assert_eq!(screenshot_theme(), None);

        std::env::set_var("SCREENSHOT_MODE", "1");
        std::env::set_var("WIZARD_SCREENSHOT", "1");
        std::env::set_var("SCREENSHOT_THEME", "dark");
        assert!(is_screenshot_mode());
        assert!(is_wizard_screenshot_mode());
        assert_eq!(screenshot_theme(), Some("dark".to_string()));

        std::env::remove_var("SCREENSHOT_MODE");
        std::env::remove_var("WIZARD_SCREENSHOT");
        std::env::remove_var("SCREENSHOT_THEME");
        assert!(!is_screenshot_mode());
        assert!(!is_wizard_screenshot_mode());
        assert_eq!(screenshot_theme(), None);
    }

    #[test]
    fn geometry_management_defers_to_a_native_wayland_compositor() {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("GDK_BACKEND");
        assert!(manage_window_geometry());

        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        assert!(!manage_window_geometry());

        // XWayland gives us real position control, so geometry is managed again.
        std::env::set_var("GDK_BACKEND", "x11");
        assert!(manage_window_geometry());

        std::env::set_var("GDK_BACKEND", "wayland");
        assert!(!manage_window_geometry());

        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("GDK_BACKEND");
    }

    #[tokio::test]
    async fn window_state_round_trips_partial_updates() {
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        save_window_state(
            state.clone(),
            Some(1280.0),
            Some(720.0),
            Some(10.0),
            Some(20.0),
        )
        .await
        .unwrap();

        let saved = get_window_state(state.clone()).await.unwrap();
        assert_eq!(saved.width, Some(1280.0));
        assert_eq!(saved.height, Some(720.0));
        assert_eq!(saved.x, Some(10.0));
        assert_eq!(saved.y, Some(20.0));

        // A resize must not wipe the recorded position.
        save_window_state(state.clone(), Some(800.0), None, None, None)
            .await
            .unwrap();

        let resized = get_window_state(state).await.unwrap();
        assert_eq!(resized.width, Some(800.0));
        assert_eq!(resized.x, Some(10.0));
        assert_eq!(resized.y, Some(20.0));
    }

    #[tokio::test]
    async fn window_state_reads_the_persisted_config() {
        let state = test_state(AppConfig {
            window_width: Some(1024.0),
            ..AppConfig::default()
        });
        let app = crate::test_support::mock_app_with_state(state);
        let window_state = get_window_state(app.state::<AppState>()).await.unwrap();

        assert_eq!(window_state.width, Some(1024.0));
        assert_eq!(window_state.height, None);
    }
}
