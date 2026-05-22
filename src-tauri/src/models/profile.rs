use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}
