use tauri::State;

use crate::models::config::AppConfig;
use crate::models::Result;
use crate::services::modio_api::ModioApiService;
use crate::state::AppState;

#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<bool> {
    let config = state.get_config()?;
    Ok(config.oauth_token.is_some())
}

#[tauri::command]
pub async fn save_token(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
    token: String,
) -> Result<()> {
    state
        .update_config(|config: &mut AppConfig| {
            config.oauth_token = Some(token);
        })
        .map(|_| ())
}

#[tauri::command]
pub async fn validate_token(state: State<'_, AppState>) -> Result<bool> {
    let config = state.get_config()?;
    let token = match config.oauth_token {
        Some(token) => token,
        None => return Ok(false),
    };

    let service = ModioApiService::new(state.client.clone(), config.modio_game_id);
    service.validate_oauth_token(&token).await
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<()> {
    state
        .update_config(|config: &mut AppConfig| {
            config.oauth_token = None;
        })
        .map(|_| ())
}

#[tauri::command]
pub async fn open_modio_login(app: tauri::AppHandle) -> Result<()> {
    let login_url = "https://mod.io/g/readyornot"; // Placeholder or actual login URL
    tauri::async_runtime::spawn(async move {
        let _ = tauri_plugin_opener::OpenerExt::opener(&app).open_url(login_url, None::<String>);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::mock_app_with;
    use tauri::Manager;

    #[tokio::test]
    async fn auth_status_follows_the_stored_token() {
        let app = mock_app_with(AppConfig {
            oauth_token: Some("token".to_string()),
            ..AppConfig::default()
        });

        assert!(get_auth_status(app.state::<AppState>()).await.unwrap());
    }

    #[tokio::test]
    async fn logout_clears_the_token() {
        let app = mock_app_with(AppConfig {
            oauth_token: Some("token".to_string()),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();

        logout(state.clone()).await.unwrap();

        assert!(state.get_config().unwrap().oauth_token.is_none());
        assert!(!get_auth_status(state).await.unwrap());
    }

    #[tokio::test]
    async fn validation_without_a_token_is_false_and_never_calls_out() {
        let app = mock_app_with(AppConfig::default());

        assert!(!validate_token(app.state::<AppState>()).await.unwrap());
    }
}
