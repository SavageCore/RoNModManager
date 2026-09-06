use std::cmp::Reverse;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::time::sleep;

use crate::models::{AppError, Result};

const NEXUS_API_BASE: &str = "https://api.nexusmods.com/v1";
const GAME_DOMAIN: &str = "readyornot";
const MAX_ATTEMPTS: usize = 3;

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
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<u32>,
    pub category_name: Option<String>,
    pub is_primary: Option<bool>,
    pub uploaded_timestamp: Option<u64>,
    pub size_in_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct NexusFilesResponse {
    files: Vec<NexusModFile>,
}

/// Returns the candidates a user should choose from.
/// - Excludes OLD_VERSION (4), DELETED (6), ARCHIVED (7)
/// - Returns all MAIN (category_id=1) files, or all active files if none are MAIN
/// - Sorted: is_primary first, then newest-first by uploaded_timestamp
///
/// `is_primary` is a pre-selection hint, not a filter - callers that show a picker
/// should pre-select `result[0]` but still show all options. Callers that need a
/// single automatic choice (premium auto-download) use `pick_primary_file`.
pub fn get_file_options(files: &[NexusModFile]) -> Vec<&NexusModFile> {
    let active: Vec<&NexusModFile> = files
        .iter()
        .filter(|f| {
            f.category_id
                .map(|c| c != 6 && c != 4 && c != 7)
                .unwrap_or(true)
        })
        .collect();

    let mut mains: Vec<&NexusModFile> = active
        .iter()
        .copied()
        .filter(|f| f.category_id == Some(1))
        .collect();

    let candidates = if !mains.is_empty() {
        &mut mains
    } else {
        let mut all = active;
        // sort inline and return - can't use the same reference trick, so handle separately
        all.sort_by_key(|f| {
            (
                if f.is_primary == Some(true) { 0u8 } else { 1u8 },
                Reverse(f.uploaded_timestamp.unwrap_or(0)),
            )
        });
        return all;
    };

    candidates.sort_by_key(|f| {
        (
            if f.is_primary == Some(true) { 0u8 } else { 1u8 },
            Reverse(f.uploaded_timestamp.unwrap_or(0)),
        )
    });
    candidates.to_vec()
}

/// Pick the best file to download from a mod's file list.
/// Prefers: explicitly primary > newest MAIN (category_id=1) > newest of any active file.
/// Excludes OLD_VERSION (4), DELETED (6), and ARCHIVED (7) files.
pub fn pick_primary_file(files: &[NexusModFile]) -> Option<&NexusModFile> {
    get_file_options(files).into_iter().next()
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

    /// Sends a request, retrying on HTTP 429 (honouring `Retry-After`, falling back to
    /// exponential backoff) and on transient network errors. Any other response status
    /// (success or otherwise) is returned as-is for the caller to inspect.
    async fn execute_with_retry<F>(&self, mut build_request: F) -> Result<reqwest::Response>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        let mut attempt = 0;

        loop {
            match build_request().send().await {
                Ok(response) => {
                    if response.status().as_u16() == 429 {
                        if attempt + 1 >= MAX_ATTEMPTS {
                            return Err(AppError::Validation(
                                "Nexus API rate limit exceeded after retries".to_string(),
                            ));
                        }

                        let retry_after = response
                            .headers()
                            .get(reqwest::header::RETRY_AFTER)
                            .and_then(|value| value.to_str().ok())
                            .and_then(|value| value.parse::<u64>().ok())
                            .unwrap_or(1);

                        sleep(Duration::from_secs(retry_after)).await;
                        attempt += 1;
                        continue;
                    }

                    return Ok(response);
                }
                Err(error) => {
                    if attempt + 1 >= MAX_ATTEMPTS {
                        return Err(AppError::Http(error));
                    }

                    // Exponential backoff: 200ms, 400ms, 800ms
                    let backoff_ms = 200_u64 * (1_u64 << attempt);
                    sleep(Duration::from_millis(backoff_ms)).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Fetch mod information from Nexus Mods API
    /// Requires a valid API key
    ///
    /// Hidden, deleted, and staff-removed mods are not accessible via the API
    /// and come back as HTTP 404 - mapped to `AppError::NotFound` with a
    /// stable "hidden or removed" marker so callers can report them as
    /// hidden instead of failed.
    pub async fn get_mod_info(&self, api_key: &str, mod_id: u64) -> Result<NexusModInfo> {
        let url = format!(
            "{}/games/{}/mods/{}.json",
            NEXUS_API_BASE, GAME_DOMAIN, mod_id
        );

        let response = self
            .execute_with_retry(|| {
                self.client
                    .get(&url)
                    .header("apikey", api_key)
                    .header("accept", "application/json")
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(nexus_mod_error(mod_id, status, &error_text));
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
            .execute_with_retry(|| {
                self.client
                    .get(&url)
                    .header("apikey", api_key)
                    .header("accept", "application/json")
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(nexus_mod_error(mod_id, status, &error_text));
        }

        let files_response: NexusFilesResponse = response.json().await?;
        Ok(files_response.files)
    }

    /// Validate an API key and return user info (including premium status)
    pub async fn get_user_info(&self, api_key: &str) -> Result<NexusUserInfo> {
        let url = format!("{}/users/validate.json", NEXUS_API_BASE);

        let response = self
            .execute_with_retry(|| {
                self.client
                    .get(&url)
                    .header("apikey", api_key)
                    .header("accept", "application/json")
            })
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
            .execute_with_retry(|| {
                self.client
                    .get(&url)
                    .header("apikey", api_key)
                    .header("accept", "application/json")
            })
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

/// Maps a failed Nexus mod/files response to a typed error.
/// HTTP 404 means the mod is hidden, deleted, or staff-removed (the API does
/// not distinguish these) - callers use the `NotFound` variant with its
/// stable marker to report the mod as hidden instead of failed.
fn nexus_mod_error(mod_id: u64, status: reqwest::StatusCode, body: &str) -> AppError {
    if status.as_u16() == 404 {
        return AppError::NotFound(format!(
            "Nexus mod {mod_id} not found (hidden or removed): {body}"
        ));
    }
    AppError::Validation(format!("Nexus API error ({status}): {body}"))
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
    fn test_nexus_mod_error_maps_404_to_not_found() {
        let err = nexus_mod_error(4212, reqwest::StatusCode::NOT_FOUND, "not found");
        assert!(matches!(err, AppError::NotFound(_)));
        assert!(err.to_string().contains("hidden or removed"));
    }

    #[test]
    fn test_nexus_mod_error_keeps_other_statuses_as_validation() {
        let err = nexus_mod_error(4212, reqwest::StatusCode::FORBIDDEN, "denied");
        assert!(matches!(err, AppError::Validation(_)));
    }

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
