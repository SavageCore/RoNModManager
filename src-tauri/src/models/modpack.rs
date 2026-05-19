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
    pub subscriptions: BTreeMap<String, bool>,
    pub mods: BTreeMap<String, ModEntry>,
    pub collections: BTreeMap<String, Collection>,
}
