use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallManifest {
    /// Name of the source archive
    pub source_archive: String,
    /// Optional display name (user-editable)
    #[serde(default)]
    pub display_name: Option<String>,
    /// Optional source URL (user-editable)
    #[serde(default)]
    pub source_url: Option<String>,
    /// List of installed files with their full paths
    pub installed_files: Vec<PathBuf>,
    /// Timestamp of installation
    pub installed_at: u64,
    /// Optional MD5 hash of the source archive for deduplication
    #[serde(default)]
    pub content_hash: Option<String>,
}

pub struct ManifestManager {
    manifests_dir: PathBuf,
}

impl ManifestManager {
    pub fn new(mods_path: &Path) -> Self {
        let manifests_dir = mods_path.join(".manifests");
        Self { manifests_dir }
    }

    pub fn ensure_manifest_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.manifests_dir)?;
        Ok(())
    }

    pub fn save_manifest(&self, manifest: &InstallManifest) -> Result<()> {
        self.ensure_manifest_dir()?;

        // Use source archive name as manifest filename (sanitized)
        let manifest_name = sanitize_filename(&manifest.source_archive);
        let manifest_path = self.manifests_dir.join(format!("{}.json", manifest_name));

        let json = serde_json::to_string_pretty(manifest)
            .map_err(|e| AppError::Validation(format!("Failed to serialize manifest: {}", e)))?;

        fs::write(&manifest_path, json)?;
        Ok(())
    }

    pub fn load_manifest(&self, archive_name: &str) -> Result<Option<InstallManifest>> {
        let manifest_name = sanitize_filename(archive_name);
        let manifest_path = self.manifests_dir.join(format!("{}.json", manifest_name));

        if !manifest_path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&manifest_path)?;
        let manifest: InstallManifest = serde_json::from_str(&json)
            .map_err(|e| AppError::Validation(format!("Failed to deserialize manifest: {}", e)))?;

        Ok(Some(manifest))
    }

    pub fn get_manifest_for_pak(
        &self,
        pak_filename: &str,
    ) -> Result<Option<(String, InstallManifest)>> {
        self.ensure_manifest_dir()?;

        // Search through all manifests to find which one contains this pak file
        for entry in fs::read_dir(&self.manifests_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let json = fs::read_to_string(&path)?;
            if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&json) {
                for installed_file in &manifest.installed_files {
                    if let Some(filename) = installed_file.file_name() {
                        if filename.to_string_lossy() == pak_filename {
                            return Ok(Some((manifest.source_archive.clone(), manifest)));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn delete_manifest(&self, archive_name: &str) -> Result<()> {
        let manifest_name = sanitize_filename(archive_name);
        let manifest_path = self.manifests_dir.join(format!("{}.json", manifest_name));

        if manifest_path.exists() {
            fs::remove_file(&manifest_path)?;
        }
        Ok(())
    }

    pub fn list_all_manifests(&self) -> Result<HashMap<String, InstallManifest>> {
        self.ensure_manifest_dir()?;

        let mut manifests = HashMap::new();

        for entry in fs::read_dir(&self.manifests_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let json = fs::read_to_string(&path)?;
            if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&json) {
                manifests.insert(manifest.source_archive.clone(), manifest);
            }
        }

        Ok(manifests)
    }

    /// Find the first manifest with matching content hash
    pub fn find_by_content_hash(&self, content_hash: &str) -> Result<Option<InstallManifest>> {
        self.ensure_manifest_dir()?;

        for entry in fs::read_dir(&self.manifests_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let json = fs::read_to_string(&path)?;
            if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&json) {
                if let Some(hash) = &manifest.content_hash {
                    if hash == content_hash {
                        return Ok(Some(manifest));
                    }
                }
            }
        }

        Ok(None)
    }
}

fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test.zip"), "test.zip");
        assert_eq!(sanitize_filename("test/file.zip"), "test_file.zip");
        assert_eq!(sanitize_filename("C:\\test\\file.zip"), "C__test_file.zip");
        assert_eq!(sanitize_filename("file*name?.zip"), "file_name_.zip");
    }
}
