use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata last used when exporting a modpack (persisted per-profile)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackMeta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
}

/// Remote sync target details resolved for the active profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDetails {
    pub host: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: Option<String>,
    /// Archive names of mods installed to this profile
    #[serde(default)]
    pub installed_mod_names: Vec<String>,
    #[serde(default)]
    pub enabled_collections: Vec<String>,
    #[serde(default)]
    pub collections: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub tags: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub collection_colors: HashMap<String, String>,
    #[serde(default)]
    pub created_at: String,
    /// Archive name → reason note (empty string = no note)
    #[serde(default)]
    pub broken_mods: HashMap<String, String>,
    /// Archive names of map mods exempt from the missing-world-gen warning
    #[serde(default)]
    pub no_world_gen: Vec<String>,
    /// Last used Export Modpack details for this profile
    #[serde(default)]
    pub modpack_meta: Option<ModpackMeta>,
    /// Remote sync target (SFTP user@host) for this profile; falls back to legacy global config when None
    #[serde(default)]
    pub sync_remote_host: Option<String>,
    /// Remote sync destination path for this profile; falls back to legacy global config when None
    #[serde(default)]
    pub sync_remote_path: Option<String>,
}

impl Profile {
    pub fn new(name: String, installed_mod_names: Vec<String>) -> Self {
        Self {
            name,
            description: None,
            installed_mod_names,
            enabled_collections: Vec::new(),
            collections: HashMap::new(),
            tags: HashMap::new(),
            collection_colors: HashMap::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            broken_mods: HashMap::new(),
            no_world_gen: Vec::new(),
            modpack_meta: None,
            sync_remote_host: None,
            sync_remote_path: None,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}
