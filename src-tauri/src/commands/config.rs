use tauri::State;

use crate::models::AppConfig;
use crate::services;
use crate::state::AppState;

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.get_config().map_err(Into::into)
}

#[tauri::command]
pub async fn set_theme(theme: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .update_config(|config| {
            config.theme = match theme.as_str() {
                "light" => crate::models::ThemeMode::Light,
                "dark" => crate::models::ThemeMode::Dark,
                _ => crate::models::ThemeMode::System,
            };
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn apply_intro_skip() -> Result<(), String> {
    services::config_tweaks::apply_intro_skip().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_intro_skip_applied() -> Result<bool, String> {
    services::config_tweaks::is_intro_skip_applied().map_err(|e| e.to_string())
}
