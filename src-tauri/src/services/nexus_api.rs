use std::cmp::Reverse;

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

#[derive(Debug, Deserialize, Clone)]
pub struct NexusModFile {
    pub file_id: u64,
    pub file_name: String,
    pub category_id: Option<u32>,
    pub category_name: Option<String>,
    pub is_primary: Option<bool>,
    pub uploaded_timestamp: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct NexusFilesResponse {
    files: Vec<NexusModFile>,
}

/// Pick the best file to download from a mod's file list.
/// Prefers: explicitly primary > newest MAIN (category_id=1) > newest of any active file.
/// Excludes OLD_VERSION (4), DELETED (6), and ARCHIVED (7) files.
pub fn pick_primary_file(files: &[NexusModFile]) -> Option<&NexusModFile> {
    let active: Vec<&NexusModFile> = files
        .iter()
        .filter(|f| {
            f.category_id
                .map(|c| c != 6 && c != 4 && c != 7)
                .unwrap_or(true)
        })
        .collect();

    if let Some(primary) = active.iter().find(|f| f.is_primary == Some(true)) {
        return Some(primary);
    }

    let mut mains: Vec<&&NexusModFile> =
        active.iter().filter(|f| f.category_id == Some(1)).collect();
    mains.sort_by_key(|f| Reverse(f.uploaded_timestamp.unwrap_or(0)));
    if let Some(main_file) = mains.first() {
        return Some(main_file);
    }

    let mut sorted = active.clone();
    sorted.sort_by_key(|f| Reverse(f.uploaded_timestamp.unwrap_or(0)));
    sorted.into_iter().next()
}

#[derive(Debug, Deserialize, Clone)]
pub struct NexusUserInfo {
    pub user_id: u64,
    pub name: String,
    pub is_premium: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NexusDownloadLink {
    pub name: String,
    pub short_name: String,
    #[serde(rename = "URI")]
    pub uri: String,
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

    /// List files for a mod from Nexus Mods API
    pub async fn list_mod_files(&self, api_key: &str, mod_id: u64) -> Result<Vec<NexusModFile>> {
        let url = format!(
            "{}/games/{}/mods/{}/files.json",
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

        let files_response: NexusFilesResponse = response.json().await?;
        Ok(files_response.files)
    }

    /// Validate an API key and return user info (including premium status)
    pub async fn get_user_info(&self, api_key: &str) -> Result<NexusUserInfo> {
        let url = format!("{}/users/validate.json", NEXUS_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("apikey", api_key)
            .header("accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AppError::Validation(
                "Nexus API key is invalid or expired.".to_string(),
            ));
        }

        let user: NexusUserInfo = response.json().await?;
        Ok(user)
    }

    /// Validate an API key by checking with the Nexus Mods API
    pub async fn validate_api_key(&self, api_key: &str) -> Result<bool> {
        Ok(self.get_user_info(api_key).await.is_ok())
    }

    /// Get CDN download links for a specific file (Premium accounts only)
    pub async fn get_download_links(
        &self,
        api_key: &str,
        mod_id: u64,
        file_id: u64,
    ) -> Result<Vec<NexusDownloadLink>> {
        let url = format!(
            "{}/games/{}/mods/{}/files/{}/download_link.json",
            NEXUS_API_BASE, GAME_DOMAIN, mod_id, file_id
        );

        let response = self
            .client
            .get(&url)
            .header("apikey", api_key)
            .header("accept", "application/json")
            .send()
            .await?;

        if response.status() == 403 {
            return Err(AppError::Validation(
                "Direct download requires a Nexus Mods Premium account.".to_string(),
            ));
        }

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

        let links: Vec<NexusDownloadLink> = response.json().await?;
        Ok(links)
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
