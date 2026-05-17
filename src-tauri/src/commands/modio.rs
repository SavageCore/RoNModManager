use tauri::State;
use serde::Deserialize;
use crate::state::AppState;
use crate::services::modio::subscribe_to_modio_mod;

#[derive(Debug, Deserialize)]
pub struct ModioSubscribeArgs {
    pub mod_id: String,
    pub oauth_token: String,
}

#[tauri::command]
pub async fn modio_subscribe(
    state: State<'_, AppState>,
    args: ModioSubscribeArgs,
) -> Result<String, String> {
    let config = state.get_config().map_err(|e| e.to_string())?;
    let game_id = config.modio_game_id.ok_or_else(|| "modio_game_id not set in config".to_string())?.to_string();
    let result = subscribe_to_modio_mod(&game_id, &args.mod_id, &args.oauth_token).await?;
    Ok(format!("Subscribed: {:?}", result))
}
