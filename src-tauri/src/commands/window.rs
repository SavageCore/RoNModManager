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
