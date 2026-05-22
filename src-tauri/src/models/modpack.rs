use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModEntry {
    pub enabled: bool,
    pub source_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Collection {
    pub default_enabled: bool,
    pub mods: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModPack {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub mods: BTreeMap<String, ModEntry>,
    pub collections: BTreeMap<String, Collection>,
    #[serde(default)]
    pub addons: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub tags: BTreeMap<String, Vec<String>>,
    /// Archive name → reason note; travels with the pack so recipients know not to re-enable
    #[serde(default)]
    pub broken: BTreeMap<String, String>,
}
