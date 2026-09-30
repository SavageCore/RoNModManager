#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ModioGameSummary {
    pub id: u32,
    pub name: Option<String>,
    pub name_id: Option<String>,
    pub slug: Option<String>,
}
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::time::sleep;

use crate::models::{AppError, Result};

const DEFAULT_BASE_URL: &str = "https://api.mod.io/v1";
const MAX_ATTEMPTS: usize = 3;

/// Mods per batched listing request. `id-in` is comma-separated and a listing
/// page holds at most 100 rows.
pub const MODIO_BATCH: usize = 100;

#[derive(Debug, Clone)]
pub struct ModioApiService {
    client: Client,
    base_url: String,
    game_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ModioModSummary {
    pub id: u64,
    pub name: String,
    pub name_id: String,
}

#[derive(Debug, Clone)]
pub struct ModioModDownload {
    pub id: u64,
    pub name: String,
    pub name_id: String,
    pub profile_url: String,
    pub filename: String,
    pub download_url: String,
    pub remote_md5: Option<String>,
    pub version: Option<String>,
}

/// Live published file for a mod, as reported by the batched listing.
#[derive(Debug, Clone)]
pub struct ModioModLatest {
    pub id: u64,
    pub remote_md5: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModioModLatestEntry {
    id: u64,
    #[serde(default)]
    modfile: Option<ModioFileInfo>,
}

#[derive(Debug, Deserialize)]
struct ModioListResponse<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct ModioDownloadInfo {
    binary_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModioFileHash {
    md5: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModioFileInfo {
    filename: Option<String>,
    version: Option<String>,
    download: Option<ModioDownloadInfo>,
    filehash: Option<ModioFileHash>,
}

#[derive(Debug, Deserialize)]
struct ModioModDetailResponse {
    id: u64,
    name: String,
    name_id: String,
    profile_url: Option<String>,
    modfile: Option<ModioFileInfo>,
}

impl ModioApiService {
    pub fn new(client: Client, game_id: Option<u32>) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
            game_id,
        }
    }

    #[cfg(test)]
    pub fn with_base_url(client: Client, base_url: String) -> Self {
        Self {
            client,
            base_url,
            game_id: Some(3791),
        }
    }

    /// Looks up the mod.io game ID for a given slug using the public API key.
    pub async fn lookup_game_id(&self, api_key: &str, slug: &str) -> Result<u32> {
        let url = format!(
            "{}/games?name_id={}&api_key={}",
            self.base_url, slug, api_key
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(AppError::Http)?;

        if !response.status().is_success() {
            return Err(AppError::Http(response.error_for_status().unwrap_err()));
        }

        let payload: ModioListResponse<ModioGameSummary> = match response.json().await {
            Ok(p) => p,
            Err(e) => {
                return Err(AppError::Validation(format!(
                    "Error decoding response body: {}",
                    e
                )));
            }
        };

        payload
            .data
            .first()
            .map(|entry| entry.id)
            .ok_or_else(|| AppError::NotFound(format!("game slug not found: {}", slug)))
    }

    pub async fn validate_oauth_token(&self, oauth_token: &str) -> Result<bool> {
        let url = format!("{}/me", self.base_url);
        match self
            .execute_with_retry(|| self.client.get(&url).bearer_auth(oauth_token))
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn resolve_slug_to_mod_id(&self, oauth_token: &str, slug: &str) -> Result<u64> {
        let game_id = self
            .game_id
            .ok_or_else(|| AppError::Validation("mod.io game_id not set".to_string()))?;
        let url = format!("{}/games/{}/mods?name_id={}", self.base_url, game_id, slug);
        let response = self
            .execute_with_retry(|| self.client.get(&url).bearer_auth(oauth_token))
            .await?;

        let payload: ModioListResponse<ModioModSummary> = response.json().await?;
        payload
            .data
            .first()
            .map(|entry| entry.id)
            .ok_or_else(|| AppError::NotFound(format!("mod slug not found: {slug}")))
    }

    pub async fn get_mod_download_info(
        &self,
        oauth_token: &str,
        mod_id: u64,
    ) -> Result<ModioModDownload> {
        let game_id = self
            .game_id
            .ok_or_else(|| AppError::Validation("mod.io game_id not set".to_string()))?;
        let url = format!("{}/games/{}/mods/{}", self.base_url, game_id, mod_id);
        let response = self
            .execute_with_retry(|| self.client.get(&url).bearer_auth(oauth_token))
            .await?;

        let payload: ModioModDetailResponse = response.json().await?;
        let modfile = payload.modfile.ok_or_else(|| {
            AppError::NotFound(format!("No downloadable file found for mod id {}", mod_id))
        })?;

        let filename = modfile
            .filename
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::NotFound(format!("Missing filename for mod id {}", mod_id)))?;

        let download_url = modfile
            .download
            .and_then(|download| download.binary_url)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                AppError::NotFound(format!("Missing download URL for mod id {}", mod_id))
            })?;

        let name_id = payload.name_id;
        let profile_url = payload
            .profile_url
            .unwrap_or_else(|| format!("https://mod.io/g/readyornot/m/{}", name_id));

        let remote_md5 = modfile.filehash.as_ref().and_then(|fh| fh.md5.clone());
        let version = modfile.version.clone();
        Ok(ModioModDownload {
            id: payload.id,
            name: payload.name,
            name_id,
            profile_url,
            filename,
            download_url,
            remote_md5,
            version,
        })
    }

    /// Live published file for many mods at once, keyed by mod id.
    ///
    /// [`Self::get_mod_download_info`] returns the same data, so checking a
    /// library of N mods used to cost N requests, each paced apart from the
    /// next. The `id-in` filter on the mods listing embeds the live `modfile`
    /// for every match, so the same library costs one request per 100 mods.
    ///
    /// Mods the listing does not return are simply missing from the map, so the
    /// caller can fall back to the per-mod endpoint for those alone.
    pub async fn get_mods_latest_batch(
        &self,
        oauth_token: &str,
        mod_ids: &[u64],
    ) -> std::collections::HashMap<u64, ModioModLatest> {
        let mut latest = std::collections::HashMap::new();
        let Some(game_id) = self.game_id else {
            return latest;
        };

        for chunk in mod_ids.chunks(MODIO_BATCH) {
            let ids = chunk
                .iter()
                .filter(|id| **id > 0)
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            if ids.is_empty() {
                continue;
            }
            // Comma-separated numeric ids, so nothing here needs escaping.
            let url = format!(
                "{}/games/{}/mods?id-in={}&_limit={}",
                self.base_url, game_id, ids, MODIO_BATCH
            );

            let response = self
                .execute_with_retry(|| self.client.get(&url).bearer_auth(oauth_token))
                .await;

            let Ok(response) = response else { continue };
            if !response.status().is_success() {
                continue;
            }
            let Ok(payload) = response
                .json::<ModioListResponse<ModioModLatestEntry>>()
                .await
            else {
                continue;
            };

            for entry in payload.data {
                let Some(modfile) = entry.modfile else {
                    continue;
                };
                latest.insert(
                    entry.id,
                    ModioModLatest {
                        id: entry.id,
                        remote_md5: modfile.filehash.and_then(|hash| hash.md5),
                        version: modfile.version,
                    },
                );
            }
        }

        latest
    }

    async fn execute_with_retry<F>(&self, mut build_request: F) -> Result<reqwest::Response>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        let mut attempt = 0;

        loop {
            let response_result = build_request().send().await;

            match response_result {
                Ok(response) => {
                    let status = response.status();

                    if status.as_u16() == 401 {
                        return Err(AppError::Validation(
                            "mod.io token is invalid or expired (401)".to_string(),
                        ));
                    }

                    if status.as_u16() == 403 {
                        return Err(AppError::Unsupported(
                            "mod.io request forbidden (mod may be hidden/DMCA)".to_string(),
                        ));
                    }

                    if status.as_u16() == 429 {
                        if attempt + 1 >= MAX_ATTEMPTS {
                            return Err(AppError::Validation(
                                "mod.io rate limit exceeded after retries".to_string(),
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

                    if status.is_success() {
                        return Ok(response);
                    }

                    return Err(AppError::Http(response.error_for_status().unwrap_err()));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    #[tokio::test]
    async fn resolve_slug_returns_first_mod_id() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/games/3791/mods")
            .match_query(Matcher::UrlEncoded(
                "name_id".to_string(),
                "fairfax-residence-remake".to_string(),
            ))
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"data":[{"id":1234,"name":"Fairfax","name_id":"fairfax-residence-remake"}]}"#,
            )
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let mod_id = service
            .resolve_slug_to_mod_id("test-token", "fairfax-residence-remake")
            .await
            .unwrap();

        assert_eq!(mod_id, 1234);
        mock.assert_async().await;
    }

    /// The whole point of the batch: one request covering many ids, each entry
    /// reduced to what an update check compares.
    #[tokio::test]
    async fn latest_batch_covers_every_id_in_one_request() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/games/3791/mods")
            .match_query(Matcher::UrlEncoded(
                "id-in".to_string(),
                "11,22,33".to_string(),
            ))
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"data":[
                    {"id":11,"modfile":{"filename":"a.zip","version":"2.0","filehash":{"md5":"AAA"}}},
                    {"id":22,"modfile":{"filename":"b.zip","version":"3.0","filehash":{"md5":"BBB"}}},
                    {"id":33,"modfile":{"filename":"c.zip","version":"4.0","filehash":{"md5":"CCC"}}}
                ]}"#,
            )
            .expect(1)
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let latest = service
            .get_mods_latest_batch("test-token", &[11, 22, 33])
            .await;

        mock.assert_async().await;
        assert_eq!(latest.len(), 3);
        assert_eq!(latest[&11].version.as_deref(), Some("2.0"));
        assert_eq!(latest[&11].remote_md5.as_deref(), Some("AAA"));
        assert_eq!(latest[&33].version.as_deref(), Some("4.0"));
    }

    /// Mods the listing omits - deleted, hidden, or simply not matched - stay
    /// absent so the caller can fall back for those alone.
    #[tokio::test]
    async fn latest_batch_omits_unlisted_mods() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/games/3791/mods")
            .match_query(Matcher::UrlEncoded(
                "id-in".to_string(),
                "11,22".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":[{"id":11,"modfile":{"version":"2.0"}}]}"#)
            .expect(1)
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let latest = service.get_mods_latest_batch("test-token", &[11, 22]).await;

        mock.assert_async().await;
        assert!(latest.contains_key(&11));
        assert!(!latest.contains_key(&22));
    }

    /// An entry with no published file is not an update candidate either.
    #[tokio::test]
    async fn latest_batch_skips_entries_without_a_file() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/games/3791/mods")
            .match_query(Matcher::UrlEncoded(
                "id-in".to_string(),
                "11,22".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":[{"id":11},{"id":22,"modfile":{"version":"2.0"}}]}"#)
            .expect(1)
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let latest = service.get_mods_latest_batch("test-token", &[11, 22]).await;

        mock.assert_async().await;
        assert_eq!(latest.keys().copied().collect::<Vec<_>>(), vec![22]);
    }

    /// A failed batch resolves to an empty map rather than an error, so one bad
    /// response can't fail the whole update check.
    #[tokio::test]
    async fn latest_batch_tolerates_a_failed_response() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/games/3791/mods")
            .match_query(Matcher::UrlEncoded(
                "id-in".to_string(),
                "11,22".to_string(),
            ))
            .with_status(500)
            .with_body("boom")
            .expect(1)
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let latest = service.get_mods_latest_batch("test-token", &[11, 22]).await;

        mock.assert_async().await;
        assert!(latest.is_empty());
    }

    /// Without a game id there is no listing to query.
    #[tokio::test]
    async fn latest_batch_without_a_game_id_returns_nothing() {
        let service = ModioApiService::new(Client::new(), None);
        let latest = service.get_mods_latest_batch("test-token", &[11]).await;
        assert!(latest.is_empty());
    }

    /// More ids than fit one page must be split across requests rather than
    /// silently truncated at the page size.
    #[tokio::test]
    async fn latest_batch_chunks_past_the_page_size() {
        let mut server = Server::new_async().await;

        let ids: Vec<u64> = (1..=(MODIO_BATCH as u64 + 5)).collect();
        let (first, second) = ids.split_at(MODIO_BATCH);

        let first_mock = server
            .mock("GET", "/games/3791/mods")
            .match_query(Matcher::UrlEncoded("id-in".to_string(), join_ids(first)))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":[{"id":1,"modfile":{"version":"1.0"}}]}"#)
            .expect(1)
            .create_async()
            .await;
        let second_mock = server
            .mock("GET", "/games/3791/mods")
            .match_query(Matcher::UrlEncoded("id-in".to_string(), join_ids(second)))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data":[{"id":9999,"modfile":{"version":"9.0"}}]}"#)
            .expect(1)
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let latest = service.get_mods_latest_batch("test-token", &ids).await;

        first_mock.assert_async().await;
        second_mock.assert_async().await;
        // One entry from each chunk, merged into a single map.
        assert_eq!(latest.len(), 2);
        assert!(latest.contains_key(&1));
        assert!(latest.contains_key(&9999));
    }

    fn join_ids(ids: &[u64]) -> String {
        ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}
