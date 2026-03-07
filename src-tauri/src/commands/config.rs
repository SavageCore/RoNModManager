use tauri::State;

use crate::models::AppConfig;
use crate::state::AppState;

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.get_config().map_err(Into::into)
}

#[tauri::command]
pub async fn set_modpack_url(url: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .update_config(|config| {
            config.modpack_url = Some(url);
        })
        .map(|_| ())
        .map_err(Into::into)
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
