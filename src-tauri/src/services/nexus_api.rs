use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Semaphore;
use tokio::time::sleep;

use crate::models::{AppError, Result};

const NEXUS_API_BASE: &str = "https://api.nexusmods.com/v1";
const NEXUS_GRAPHQL_BASE: &str = "https://api.nexusmods.com/v2/graphql";
const GAME_DOMAIN: &str = "readyornot";
const MAX_ATTEMPTS: usize = 3;

/// Mods per GraphQL request. `modFiles` returns at most this many aliased
/// fields per request, so larger libraries are chunked.
pub const GRAPHQL_FILE_BATCH: usize = 20;

/// GraphQL requests in flight across all batches of one call. Batched requests
/// are cheap and unrate-limited, so this only needs to bound socket pressure.
const GRAPHQL_CONCURRENCY: usize = 4;

#[derive(Debug, Clone)]
pub struct NexusApiService {
    client: Client,
    base_url: String,
    graphql_url: String,
    /// Category id-to-name map for the game, fetched at most once per service
    /// instance: a bulk metadata refresh then costs one extra request instead
    /// of one per mod.
    categories: Arc<tokio::sync::OnceCell<Vec<NexusGameCategory>>>,
    /// Numeric Nexus game id, needed by the GraphQL `modFiles` field. Resolved
    /// at most once per service instance.
    game_id: Arc<tokio::sync::OnceCell<u64>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NexusModInfo {
    pub mod_id: u64,
    pub name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub picture_url: Option<String>,
    pub domain_name: String,
    #[serde(default)]
    pub category_id: Option<u64>,
    #[serde(default)]
    pub category_name: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NexusGameCategory {
    pub category_id: u64,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NexusGameInfo {
    pub categories: Vec<NexusGameCategory>,
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

/// `modFiles` is aliased per mod, so the payload is a map of `m<modId>` to that
/// mod's file list.
#[derive(Debug, Deserialize)]
struct GraphQlModFilesResponse {
    #[serde(default)]
    data: HashMap<String, Vec<GraphQlModFile>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlModFile {
    #[serde(rename = "fileId")]
    file_id: u64,
    #[serde(rename = "uri", default)]
    file_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "categoryId", default)]
    category_id: Option<u32>,
    #[serde(default)]
    category: Option<GraphQlCategory>,
    #[serde(rename = "sizeInBytes", default)]
    size_in_bytes: Option<u64>,
    #[serde(rename = "date", default)]
    uploaded_timestamp: Option<u64>,
}

/// Nexus returns the file category as a bare enum string on some endpoints and
/// as a `{ name }` object on others; accept either.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum GraphQlCategory {
    Name(String),
    Object { name: Option<String> },
}

impl GraphQlCategory {
    fn into_name(self) -> Option<String> {
        match self {
            GraphQlCategory::Name(value) => Some(value),
            GraphQlCategory::Object { name } => name,
        }
        .filter(|value| !value.trim().is_empty())
    }
}

impl GraphQlModFile {
    /// Reuses [`NexusModFile`] so batched and REST results stay interchangeable
    /// for the variant logic in `commands::mods`.
    fn to_mod_file(&self) -> NexusModFile {
        NexusModFile {
            file_id: self.file_id,
            file_name: self.file_name.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            category_id: self.category_id,
            category_name: self.category.clone().and_then(GraphQlCategory::into_name),
            is_primary: None,
            uploaded_timestamp: self.uploaded_timestamp,
            size_in_bytes: self.size_in_bytes,
        }
    }
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
        Self::with_base_url(client, NEXUS_API_BASE.to_string())
    }

    /// Overrides the API endpoint so tests can point at a local mock server.
    /// Not `#[cfg(test)]` like `ModioApiService::with_base_url`: `AppState::nexus`
    /// needs it on the production path.
    pub fn with_base_url(client: Client, base_url: String) -> Self {
        Self::with_endpoints(client, base_url, NEXUS_GRAPHQL_BASE.to_string())
    }

    /// Overrides both endpoints. The GraphQL API lives on a different path from
    /// REST, so tests that exercise it need to point at the mock server too.
    pub fn with_endpoints(client: Client, base_url: String, graphql_url: String) -> Self {
        Self {
            client,
            base_url,
            graphql_url,
            categories: Arc::new(tokio::sync::OnceCell::new()),
            game_id: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    /// Redirects the GraphQL endpoint, keeping the REST base as-is.
    pub fn with_graphql_url(mut self, graphql_url: String) -> Self {
        self.graphql_url = graphql_url;
        self
    }

    /// Nexus category id-to-name map, fetched at most once per instance.
    /// Empty when the lookup fails - callers treat a missing category as
    /// "no tag" rather than failing the resolution.
    pub async fn categories(&self, api_key: &str) -> &[NexusGameCategory] {
        self.categories
            .get_or_init(|| async {
                self.get_game_info(api_key)
                    .await
                    .map(|info| info.categories)
                    .unwrap_or_default()
            })
            .await
    }

    /// File lists for many mods at once, keyed by mod id.
    ///
    /// The REST `files.json` endpoint is hourly rate limited per API key, so
    /// checking one mod per request is both slow and quota-hungry - a 100-mod
    /// library burns 100 requests. GraphQL is not rate limited, and `modFiles`
    /// can be aliased up to [`GRAPHQL_FILE_BATCH`] times per request, so the
    /// same library costs 5.
    ///
    /// Mods absent from the response are simply missing from the map, so the
    /// caller can fall back to REST for those alone.
    pub async fn graphql_mod_files_batch(
        &self,
        api_key: &str,
        mod_ids: &[u64],
    ) -> HashMap<u64, Vec<NexusModFile>> {
        let mut files = HashMap::new();
        if mod_ids.is_empty() {
            return files;
        }

        // Without a game id there is no `modFiles` to query. Bail out entirely
        // and let the caller's REST fallback cover the library.
        let game_id = match self.resolve_game_id(api_key).await {
            Ok(id) => id,
            Err(_) => return files,
        };

        let permits = Arc::new(Semaphore::new(GRAPHQL_CONCURRENCY));
        let mut tasks = Vec::new();
        for batch in mod_ids.chunks(GRAPHQL_FILE_BATCH) {
            let service = self.clone();
            let permits = Arc::clone(&permits);
            let api_key = api_key.to_string();
            let batch = batch.to_vec();
            tasks.push(tokio::spawn(async move {
                let _permit = permits.acquire_owned().await;
                service
                    .fetch_mod_files_graphql(&api_key, game_id, &batch)
                    .await
            }));
        }

        for task in tasks {
            if let Ok(batch_files) = task.await {
                files.extend(batch_files);
            }
        }
        files
    }

    /// Numeric game id, which the GraphQL `modFiles` field requires instead of
    /// a domain string. Resolved over GraphQL so it costs no REST quota, and
    /// cached for the life of the service.
    ///
    /// A failure caches as 0 for this instance. That is deliberate rather than
    /// sloppy: `AppState::nexus()` hands out a fresh service per command, so a
    /// stuck negative result never outlives the check that produced it.
    async fn resolve_game_id(&self, api_key: &str) -> Result<u64> {
        let game_id = *self
            .game_id
            .get_or_init(|| async {
                self.post_graphql(
                    api_key,
                    &format!(r#"{{ game(domainName: "{GAME_DOMAIN}") {{ id }} }}"#),
                )
                .await
                .and_then(|payload| payload.get("data")?.get("game")?.get("id")?.as_u64())
                .unwrap_or(0)
            })
            .await;

        if game_id == 0 {
            return Err(AppError::NotFound(
                "could not resolve Nexus game id for GraphQL".to_string(),
            ));
        }
        Ok(game_id)
    }

    /// One aliased `modFiles` request covering up to [`GRAPHQL_FILE_BATCH`] mods.
    async fn fetch_mod_files_graphql(
        &self,
        api_key: &str,
        game_id: u64,
        mod_ids: &[u64],
    ) -> HashMap<u64, Vec<NexusModFile>> {
        let mut files = HashMap::new();
        if mod_ids.is_empty() {
            return files;
        }

        let aliases = mod_ids
            .iter()
            .map(|mod_id| {
                format!(
                    "  m{mod_id}: modFiles(gameId: {game_id}, modId: {mod_id}) {{ \
                     fileId uri name version description categoryId category sizeInBytes date }}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let Some(payload) = self
            .post_graphql(api_key, &format!("query ModFilesBatch {{\n{aliases}\n}}"))
            .await
        else {
            return files;
        };
        // A response carrying `errors` alongside a partial `data` still parses,
        // so absent fields fall through as missing mods rather than failing the
        // whole batch.
        let Ok(response) = serde_json::from_value::<GraphQlModFilesResponse>(payload) else {
            return files;
        };

        for mod_id in mod_ids {
            let alias = format!("m{mod_id}");
            let Some(entries) = response.data.get(&alias) else {
                continue;
            };
            if entries.is_empty() {
                continue;
            }
            files.insert(
                *mod_id,
                entries.iter().map(GraphQlModFile::to_mod_file).collect(),
            );
        }
        files
    }

    /// POSTs a GraphQL query. `None` on any transport, status or decode
    /// failure - callers treat that as "fall back to REST".
    async fn post_graphql(&self, api_key: &str, query: &str) -> Option<serde_json::Value> {
        let response = self
            .execute_with_retry(|| {
                self.client
                    .post(&self.graphql_url)
                    .header("apikey", api_key)
                    .header("accept", "application/json")
                    .json(&serde_json::json!({ "query": query }))
            })
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }
        response.json::<serde_json::Value>().await.ok()
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
            self.base_url, GAME_DOMAIN, mod_id
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
            self.base_url, GAME_DOMAIN, mod_id
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
        let url = format!("{}/users/validate.json", self.base_url);

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
            self.base_url, GAME_DOMAIN, mod_id, file_id
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

    /// Fetch game metadata including the category id-to-name map.
    pub async fn get_game_info(&self, api_key: &str) -> Result<NexusGameInfo> {
        let url = format!("{}/games/{}", self.base_url, GAME_DOMAIN);

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
            return Err(nexus_mod_error(0, status, &error_text));
        }

        let game_info: NexusGameInfo = response.json().await?;
        Ok(game_info)
    }
}

/// Resolves a ReadyOrNot mod category name from the game's category list.
/// Prefers the API-provided `category_name` on the mod itself, then falls back to
/// looking up `category_id` in the provided game-info categories.
/// Returns None for unknown/missing categories so callers can skip tagging.
pub(crate) fn resolve_nexus_category_name(
    categories: &[NexusGameCategory],
    category_id: Option<u64>,
    category_name: Option<&str>,
) -> Option<String> {
    if let Some(name) = category_name.filter(|n| !n.is_empty()) {
        return Some(name.to_string());
    }
    if let Some(id) = category_id {
        if let Some(cat) = categories.iter().find(|c| c.category_id == id) {
            return Some(cat.name.clone());
        }
    }
    None
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
    fn test_resolve_nexus_category_name_prefers_mod_field() {
        let categories = vec![
            NexusGameCategory {
                category_id: 11,
                name: "Maps".to_string(),
            },
            NexusGameCategory {
                category_id: 7,
                name: "Weapons".to_string(),
            },
        ];
        assert_eq!(
            resolve_nexus_category_name(&categories, Some(7), Some("Override")),
            Some("Override".to_string()),
        );
    }

    #[test]
    fn test_resolve_nexus_category_name_lookup_by_id() {
        let categories = vec![
            NexusGameCategory {
                category_id: 11,
                name: "Maps".to_string(),
            },
            NexusGameCategory {
                category_id: 7,
                name: "Weapons".to_string(),
            },
        ];
        assert_eq!(
            resolve_nexus_category_name(&categories, Some(11), None),
            Some("Maps".to_string()),
        );
        assert_eq!(
            resolve_nexus_category_name(&categories, Some(7), None),
            Some("Weapons".to_string()),
        );
    }

    #[test]
    fn test_resolve_nexus_category_name_unknown_id_returns_none() {
        let categories = vec![NexusGameCategory {
            category_id: 11,
            name: "Maps".to_string(),
        }];
        assert_eq!(
            resolve_nexus_category_name(&categories, Some(999), None),
            None,
        );
    }

    #[test]
    fn test_resolve_nexus_category_name_empty_name_skips_to_lookup() {
        let categories = vec![NexusGameCategory {
            category_id: 11,
            name: "Maps".to_string(),
        }];
        assert_eq!(
            resolve_nexus_category_name(&categories, Some(11), Some("")),
            Some("Maps".to_string()),
        );
    }

    #[test]
    fn test_resolve_nexus_category_name_no_inputs_returns_none() {
        let categories = vec![NexusGameCategory {
            category_id: 11,
            name: "Maps".to_string(),
        }];
        assert_eq!(resolve_nexus_category_name(&categories, None, None), None,);
    }

    #[test]
    fn test_resolve_nexus_category_name_empty_categories_returns_none() {
        let categories: Vec<NexusGameCategory> = vec![];
        assert_eq!(
            resolve_nexus_category_name(&categories, Some(11), None),
            None,
        );
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

    fn file_option(
        file_id: u64,
        category_id: Option<u32>,
        is_primary: Option<bool>,
        uploaded_timestamp: Option<u64>,
    ) -> NexusModFile {
        NexusModFile {
            file_id,
            file_name: format!("file-{file_id}.zip"),
            name: None,
            version: None,
            description: None,
            category_id,
            category_name: None,
            is_primary,
            uploaded_timestamp,
            size_in_bytes: None,
        }
    }

    #[test]
    fn test_get_file_options_drops_old_deleted_and_archived() {
        let files = vec![
            file_option(1, Some(1), None, None),
            file_option(2, Some(4), None, None),
            file_option(3, Some(6), None, None),
            file_option(4, Some(7), None, None),
        ];
        let ids: Vec<u64> = get_file_options(&files).iter().map(|f| f.file_id).collect();
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn test_get_file_options_keeps_only_main_when_present() {
        let files = vec![
            file_option(1, Some(1), None, None),
            file_option(2, Some(3), None, None),
        ];
        let ids: Vec<u64> = get_file_options(&files).iter().map(|f| f.file_id).collect();
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn test_get_file_options_keeps_all_active_without_main() {
        let files = vec![
            file_option(1, Some(3), None, None),
            file_option(2, Some(2), None, None),
            file_option(3, Some(7), None, None),
        ];
        let ids: Vec<u64> = get_file_options(&files).iter().map(|f| f.file_id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn test_get_file_options_treats_unknown_category_as_active() {
        let files = vec![
            file_option(1, None, None, None),
            file_option(2, None, None, None),
        ];
        assert_eq!(get_file_options(&files).len(), 2);
    }

    #[test]
    fn test_get_file_options_sorts_primary_first_then_newest() {
        let files = vec![
            file_option(1, Some(1), Some(false), Some(100)),
            file_option(2, Some(1), Some(true), Some(50)),
            file_option(3, Some(1), Some(false), Some(200)),
        ];
        let ids: Vec<u64> = get_file_options(&files).iter().map(|f| f.file_id).collect();
        assert_eq!(ids, vec![2, 3, 1]);
    }

    #[test]
    fn test_pick_primary_file_prefers_primary_then_newest_main() {
        let files = vec![
            file_option(1, Some(1), Some(false), Some(300)),
            file_option(2, Some(1), Some(true), Some(50)),
        ];
        assert_eq!(pick_primary_file(&files).map(|f| f.file_id), Some(2));

        let no_primary = vec![
            file_option(1, Some(1), Some(false), Some(100)),
            file_option(2, Some(1), Some(false), Some(200)),
        ];
        assert_eq!(pick_primary_file(&no_primary).map(|f| f.file_id), Some(2));
    }

    #[test]
    fn test_pick_primary_file_returns_none_when_nothing_active() {
        let files = vec![file_option(1, Some(4), None, None)];
        assert!(pick_primary_file(&files).is_none());
        let empty: Vec<NexusModFile> = vec![];
        assert!(pick_primary_file(&empty).is_none());
    }
}
