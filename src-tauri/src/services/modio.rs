use serde::{Deserialize, Serialize};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, ACCEPT};
use reqwest::Client;
use std::env;

fn modio_api_url(game_id: &str) -> String {
    format!("https://g-{}.modapi.io/v1", game_id)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModioSubscriptionResponse {
    pub success: bool,
    pub status: Option<String>,
}

pub async fn subscribe_to_modio_mod(game_id: &str, mod_id: &str, oauth_token: &str) -> Result<ModioSubscriptionResponse, String> {
    let url = format!(
        "{}/games/{}/mods/{}/subscribe",
        modio_api_url(game_id), game_id, mod_id
    );
    let client = Client::new();
    let resp = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", oauth_token))
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;
    let status = resp.status();
    if status.is_success() {
        Ok(ModioSubscriptionResponse {
            success: true,
            status: Some(status.to_string()),
        })
    } else {
        Err(format!("mod.io subscription failed: {}", status))
    }
}
