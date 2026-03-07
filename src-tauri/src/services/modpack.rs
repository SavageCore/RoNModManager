use std::path::{Path, PathBuf};

use reqwest::Client;
use semver::Version;
use serde::Deserialize;

use crate::models::{AppError, ModPack, Result};
use crate::services::downloader;

#[derive(Debug, Clone, Deserialize)]
pub struct ModpackManifest {
    pub files: Vec<ManifestFileEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestFileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

pub fn normalize_modpack_url(base_url: &str) -> String {
    format!("{}/ronmod.pack", base_url.trim_end_matches('/'))
}

pub fn normalize_manifest_url(base_url: &str) -> String {
    format!("{}/ronmod.manifest", base_url.trim_end_matches('/'))
}

pub async fn fetch_modpack(client: &Client, base_url: &str) -> Result<ModPack> {
    let url = normalize_modpack_url(base_url);
    let response = client.get(url).send().await?;
    let response = response.error_for_status()?;
    let bytes = response.bytes().await?;
    parse_modpack_json(&bytes)
}

pub fn parse_modpack_json(input: &[u8]) -> Result<ModPack> {
    serde_json::from_slice::<ModPack>(input).map_err(AppError::from)
}

pub fn is_newer_version(incoming: &str, current: &str) -> Result<bool> {
    let incoming = Version::parse(incoming)
        .map_err(|error| AppError::Validation(format!("invalid incoming version: {error}")))?;
    let current = Version::parse(current)
        .map_err(|error| AppError::Validation(format!("invalid current version: {error}")))?;
    Ok(incoming > current)
}

pub async fn fetch_manifest_if_exists(
    client: &Client,
    base_url: &str,
) -> Result<Option<ModpackManifest>> {
    let url = normalize_manifest_url(base_url);
    let response = client.get(url).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return fetch_manifest_from_html(client, base_url).await;
    }

    let response = response.error_for_status()?;
    let manifest = response.json::<ModpackManifest>().await?;
    Ok(Some(manifest))
}

/// Fallback: Try to discover mod files from HTML directory listing
async fn fetch_manifest_from_html(
    client: &Client,
    base_url: &str,
) -> Result<Option<ModpackManifest>> {
    let mods_url = format!("{}/mods", base_url.trim_end_matches('/'));

    let response = client.get(&mods_url).send().await?;

    // If mods directory doesn't exist, no fallback possible
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let response = response.error_for_status()?;
    let html = response.text().await?;

    // Parse HTML directory listing for mod files
    // Look for href attributes pointing to .pak, .zip, .sav files
    let files = extract_files_from_html(&html, &mods_url);

    if files.is_empty() {
        return Ok(None);
    }

    Ok(Some(ModpackManifest { files }))
}

/// Extract downloadable files from HTML directory listing
fn extract_files_from_html(html: &str, _base_url: &str) -> Vec<ManifestFileEntry> {
    let mut files = Vec::new();

    // Find all href attributes in HTML
    for line in html.lines() {
        // Look for href patterns in <a> tags
        if let Some(href_start) = line.find("href=\"") {
            if let Some(href_end) = line[href_start + 6..].find('"') {
                let href = &line[href_start + 6..href_start + 6 + href_end];

                // Filter for mod files (skip parent directory references)
                if (href.ends_with(".pak") || href.ends_with(".zip") || href.ends_with(".sav"))
                    && !href.starts_with('/')
                    && !href.contains("..")
                {
                    files.push(ManifestFileEntry {
                        path: href.to_string(),
                        sha256: String::new(), // unknown sha256 from HTML fallback
                        size: 0,               // unknown size from HTML fallback
                    });
                }
            }
        }
    }

    files
}

pub async fn download_manifest_files(
    client: &Client,
    base_url: &str,
    destination_root: &Path,
    manifest: &ModpackManifest,
) -> Result<Vec<PathBuf>> {
    let mut downloaded_paths = Vec::with_capacity(manifest.files.len());

    for entry in &manifest.files {
        let remote = format!("{}/mods/{}", base_url.trim_end_matches('/'), entry.path);
        let local_path = destination_root.join(&entry.path);

        if local_path.exists() {
            // Existing behavior: skip by filename presence for now.
            downloaded_paths.push(local_path);
            continue;
        }

        downloader::download_file(client, &remote, &local_path).await?;
        downloaded_paths.push(local_path);
    }

    Ok(downloaded_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use tempfile::TempDir;

    fn example_pack_json() -> &'static str {
        r#"{
			"schema_version": 1,
			"name": "SavagePack",
			"version": "1.2.0",
			"description": "Example",
			"author": "SavageCore",
			"subscriptions": ["fairfax-residence-remake"],
			"collections": {
				"Beat Cop": {
					"default_enabled": true,
					"description": "Realistic",
					"mods": ["A.pak"]
				}
			}
		}"#
    }

    #[test]
    fn parse_modpack_valid_json() {
        let pack = parse_modpack_json(example_pack_json().as_bytes()).unwrap();
        assert_eq!(pack.name, "SavagePack");
        assert_eq!(pack.version, "1.2.0");
        assert_eq!(pack.subscriptions.len(), 1);
    }

    #[test]
    fn version_comparison_works() {
        assert!(is_newer_version("1.2.0", "1.1.9").unwrap());
        assert!(!is_newer_version("1.2.0", "1.2.0").unwrap());
        assert!(!is_newer_version("1.1.9", "1.2.0").unwrap());
    }

    #[tokio::test]
    async fn fetch_modpack_uses_ronmod_pack_url() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/packs/ronmod.pack")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(example_pack_json())
            .create_async()
            .await;

        let client = Client::new();
        let base = format!("{}/packs", server.url());
        let pack = fetch_modpack(&client, &base).await.unwrap();

        assert_eq!(pack.name, "SavagePack");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_manifest_returns_none_on_404() {
        let mut server = Server::new_async().await;
        let _mock1 = server
            .mock("GET", "/packs/ronmod.manifest")
            .with_status(404)
            .create_async()
            .await;
        let _mock2 = server
            .mock("GET", "/packs/mods")
            .with_status(404)
            .create_async()
            .await;

        let client = Client::new();
        let base = format!("{}/packs", server.url());
        let manifest = fetch_manifest_if_exists(&client, &base).await.unwrap();

        assert!(manifest.is_none());
    }

    #[tokio::test]
    async fn download_manifest_files_downloads_to_destination() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/packs/mods/_manual/test.pak")
            .with_status(200)
            .with_body("abc")
            .create_async()
            .await;

        let manifest = ModpackManifest {
            files: vec![ManifestFileEntry {
                path: "_manual/test.pak".to_string(),
                sha256: "unused".to_string(),
                size: 3,
            }],
        };

        let temp_dir = TempDir::new().unwrap();
        let client = Client::new();
        let base = format!("{}/packs", server.url());
        let downloaded = download_manifest_files(&client, &base, temp_dir.path(), &manifest)
            .await
            .unwrap();

        assert_eq!(downloaded.len(), 1);
        assert!(downloaded[0].exists());
    }

    #[test]
    fn extract_files_from_html_directory_listing() {
        let html = r#"
            <html>
            <body>
            <a href="mod1.pak">mod1.pak</a>
            <a href="mod2.zip">mod2.zip</a>
            <a href="savegame.sav">savegame.sav</a>
            <a href="../parent.pak">../parent.pak</a>
            <a href="/absolute.pak">/absolute.pak</a>
            </body>
            </html>
        "#;

        let files = extract_files_from_html(html, "http://example.com/mods");
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "mod1.pak");
        assert_eq!(files[1].path, "mod2.zip");
        assert_eq!(files[2].path, "savegame.sav");
    }
}
