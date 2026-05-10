use reqwest::Client;
use serde::Deserialize;

use crate::models::{AppError, Result};

const NEXUS_API_BASE: &str = "https://api.nexusmods.com/v1";
const GAME_DOMAIN: &str = "readyornot";

#[derive(Debug, Clone)]
pub struct NexusApiService {
    client: Client,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NexusModInfo {
    pub mod_id: u64,
    pub name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub picture_url: Option<String>,
    pub domain_name: String,
}

impl NexusApiService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Fetch mod information from Nexus Mods API
    /// Requires a valid API key
    pub async fn get_mod_info(&self, api_key: &str, mod_id: u64) -> Result<NexusModInfo> {
        let url = format!(
            "{}/games/{}/mods/{}.json",
            NEXUS_API_BASE, GAME_DOMAIN, mod_id
        );

        let response = self
            .client
            .get(&url)
            .header("apikey", api_key)
            .header("accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::Validation(format!(
                "Nexus API error ({}): {}",
                status, error_text
            )));
        }

        let mod_info: NexusModInfo = response.json().await?;
        Ok(mod_info)
    }

    /// Validate an API key by checking with the Nexus Mods API
    pub async fn validate_api_key(&self, api_key: &str) -> Result<bool> {
        let url = format!("{}/users/validate.json", NEXUS_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("apikey", api_key)
            .header("accept", "application/json")
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

/// Parse Nexus Mods URL to extract mod ID
/// Supports formats:
/// - https://www.nexusmods.com/readyornot/mods/1234
/// - https://nexusmods.com/readyornot/mods/1234
/// - Just the ID: 1234
pub fn parse_nexus_url_to_mod_id(input: &str) -> Result<u64> {
    let trimmed = input.trim();

    // Try parsing as direct ID first
    if let Ok(id) = trimmed.parse::<u64>() {
        return Ok(id);
    }

    // Try extracting from URL
    if let Some(mods_idx) = trimmed.find("/mods/") {
        let after_mods = &trimmed[mods_idx + 6..];
        let id_str = after_mods.split(&['/', '?', '#'][..]).next().unwrap_or("");

        if let Ok(id) = id_str.parse::<u64>() {
            return Ok(id);
        }
    }

    Err(AppError::Validation(format!(
        "Could not extract mod ID from Nexus input: {}",
        trimmed
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nexus_url() {
        assert_eq!(parse_nexus_url_to_mod_id("1234").unwrap(), 1234);
        assert_eq!(
            parse_nexus_url_to_mod_id("https://www.nexusmods.com/readyornot/mods/1234").unwrap(),
            1234
        );
        assert_eq!(
            parse_nexus_url_to_mod_id("https://nexusmods.com/readyornot/mods/5678?tab=files")
                .unwrap(),
            5678
        );
        assert_eq!(
            parse_nexus_url_to_mod_id("https://www.nexusmods.com/readyornot/mods/999#description")
                .unwrap(),
            999
        );
    }
}
