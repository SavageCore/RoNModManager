use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::time::sleep;

use crate::models::{AppError, Result};

const DEFAULT_BASE_URL: &str = "https://api.mod.io/v1";
const DEFAULT_GAME_ID: u32 = 3791;
const MAX_ATTEMPTS: usize = 3;

#[derive(Debug, Clone)]
pub struct ModioApiService {
    client: Client,
    base_url: String,
    game_id: u32,
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
struct ModioFileInfo {
    filename: Option<String>,
    download: Option<ModioDownloadInfo>,
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
    pub fn new(client: Client) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
            game_id: DEFAULT_GAME_ID,
        }
    }

    #[cfg(test)]
    pub fn with_base_url(client: Client, base_url: String) -> Self {
        Self {
            client,
            base_url,
            game_id: DEFAULT_GAME_ID,
        }
    }

    pub async fn fetch_subscribed_mods(&self, oauth_token: &str) -> Result<Vec<ModioModSummary>> {
        let url = format!("{}/me/subscribed?game_id={}", self.base_url, self.game_id);

        let response = self
            .execute_with_retry(|| self.client.get(&url).bearer_auth(oauth_token))
            .await?;

        let payload: ModioListResponse<ModioModSummary> = response.json().await?;
        Ok(payload.data)
    }

    pub async fn subscribe_mod(&self, oauth_token: &str, mod_id: u64) -> Result<()> {
        let url = format!(
            "{}/games/{}/mods/{}/subscribe",
            self.base_url, self.game_id, mod_id
        );

        self.execute_with_retry(|| self.client.post(&url).bearer_auth(oauth_token))
            .await?;

        Ok(())
    }

    pub async fn unsubscribe_mod(&self, oauth_token: &str, mod_id: u64) -> Result<()> {
        let url = format!(
            "{}/games/{}/mods/{}/subscribe",
            self.base_url, self.game_id, mod_id
        );

        self.execute_with_retry(|| self.client.delete(&url).bearer_auth(oauth_token))
            .await?;

        Ok(())
    }

    pub async fn resolve_slug_to_mod_id(&self, oauth_token: &str, slug: &str) -> Result<u64> {
        let url = format!(
            "{}/games/{}/mods?name_id={}",
            self.base_url, self.game_id, slug
        );

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
        let url = format!("{}/games/{}/mods/{}", self.base_url, self.game_id, mod_id);
        let response = self
            .execute_with_retry(|| self.client.get(&url).bearer_auth(oauth_token))
            .await?;

        let payload: ModioModDetailResponse = response.json().await?;
        let modfile = payload
            .modfile
            .ok_or_else(|| AppError::NotFound(format!("No downloadable file found for mod id {}", mod_id)))?;

        let filename = modfile
            .filename
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::NotFound(format!("Missing filename for mod id {}", mod_id)))?;

        let download_url = modfile
            .download
            .and_then(|download| download.binary_url)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::NotFound(format!("Missing download URL for mod id {}", mod_id)))?;

        let name_id = payload.name_id;
        let profile_url = payload
            .profile_url
            .unwrap_or_else(|| format!("https://mod.io/g/readyornot/m/{}", name_id));

        Ok(ModioModDownload {
            id: payload.id,
            name: payload.name,
            name_id,
            profile_url,
            filename,
            download_url,
        })
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

    #[tokio::test]
    async fn fetch_subscribed_mods_parses_response() {
        let mut server = Server::new_async().await;

        let mock = server
			.mock("GET", "/me/subscribed")
			.match_query(Matcher::UrlEncoded("game_id".to_string(), "3791".to_string()))
			.match_header("authorization", "Bearer test-token")
			.with_status(200)
			.with_header("content-type", "application/json")
			.with_body(
				r#"{"data":[{"id":10,"name":"Map Pack","name_id":"map-pack"},{"id":11,"name":"Gear","name_id":"gear"}]}"#,
			)
			.create_async()
			.await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let mods = service.fetch_subscribed_mods("test-token").await.unwrap();

        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].id, 10);
        assert_eq!(mods[1].name_id, "gear");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn unauthorized_token_returns_validation_error() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("GET", "/me/subscribed")
            .match_query(Matcher::UrlEncoded(
                "game_id".to_string(),
                "3791".to_string(),
            ))
            .with_status(401)
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let result = service.fetch_subscribed_mods("bad-token").await;

        assert!(matches!(result, Err(AppError::Validation(_))));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn rate_limit_retries_then_succeeds() {
        let mut server = Server::new_async().await;

        let first = server
            .mock("POST", "/games/3791/mods/999/subscribe")
            .match_header("authorization", "Bearer test-token")
            .with_status(429)
            .with_header("retry-after", "0")
            .expect(1)
            .create_async()
            .await;

        let second = server
            .mock("POST", "/games/3791/mods/999/subscribe")
            .match_header("authorization", "Bearer test-token")
            .with_status(204)
            .expect(1)
            .create_async()
            .await;

        let service = ModioApiService::with_base_url(Client::new(), server.url());
        let result = service.subscribe_mod("test-token", 999).await;

        assert!(result.is_ok());
        first.assert_async().await;
        second.assert_async().await;
    }
}
