use tauri::{AppHandle, State};

use crate::services::modio_api::ModioApiService;
use crate::state::AppState;

#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<bool, String> {
    let config = state.get_config().map_err(String::from)?;
    Ok(config.oauth_token.is_some())
}

#[tauri::command]
pub async fn open_modio_login(_app: AppHandle) -> Result<(), String> {
    tauri_plugin_opener::open_url("https://mod.io/me/access", Option::<&str>::None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_token(token: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .update_config(|config| {
            config.oauth_token = Some(token);
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn validate_token(state: State<'_, AppState>) -> Result<bool, String> {
    let config = state.get_config().map_err(String::from)?;
    let token = match config.oauth_token {
        Some(token) => token,
        None => return Ok(false),
    };

    let service = ModioApiService::new(state.client.clone());
    match service.fetch_subscribed_mods(&token).await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    state
        .update_config(|config| {
            config.oauth_token = None;
        })
        .map(|_| ())
        .map_err(Into::into)
}
