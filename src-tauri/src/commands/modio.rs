use tauri::{State, Window, Manager, Emitter};
use serde::Deserialize;
use crate::state::AppState;
use crate::services::modio_api::ModioApiService;

#[derive(Debug, Deserialize)]
pub struct ModioSubscribeArgs {
    pub mod_id: String,
    pub oauth_token: String,
}

#[tauri::command]
pub async fn modio_subscribe(
    window: Window,
    state: State<'_, AppState>,
    args: ModioSubscribeArgs,
) -> Result<String, String> {
    let config = state.get_config().map_err(|e| e.to_string())?;
    let game_id = config.modio_game_id.ok_or_else(|| "modio_game_id not set in config".to_string())?;
    let client = state.client.clone();
    let api = ModioApiService::new(client, Some(game_id));
    let mod_id_num = args.mod_id.parse::<u64>().map_err(|e| e.to_string())?;
    api.subscribe_mod(&args.oauth_token, mod_id_num).await.map_err(|e| e.to_string())?;
    let _ = window.emit("modio_subscribe_status", "subscribed");
    Ok("Subscribed".to_string())
}

#[derive(Default)]
struct MyState {
  s: std::sync::Mutex<String>,
  t: std::sync::Mutex<std::collections::HashMap<String, String>>,
}
#[tauri::command]
pub async fn get_modio_subscription_status(
    state: State<'_, AppState>,
    args: ModioSubscribeArgs,
) -> Result<String, String> {
    let config = state.get_config().map_err(|e| e.to_string())?;
    let game_id = config.modio_game_id.ok_or_else(|| "modio_game_id not set in config".to_string())?;
    let client = state.client.clone();
    let api = ModioApiService::new(client, Some(game_id));
    let mod_id_num = args.mod_id.parse::<u64>().map_err(|e| e.to_string())?;
    // DEBUG: log the mod ID being checked for subscription
    log::info!("Checking subscription status for mod ID: {}", mod_id_num);
    let is_subbed = api.is_subscribed(&args.oauth_token, mod_id_num).await.map_err(|e| e.to_string())?;
    if is_subbed {
        log::info!("User is subscribed to mod ID: {}", mod_id_num);
        Ok("subscribed".to_string())
    } else {
        log::info!("User is NOT subscribed to mod ID: {}", mod_id_num);
        Ok("not_subscribed".to_string())
    }
}
