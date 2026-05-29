use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

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
