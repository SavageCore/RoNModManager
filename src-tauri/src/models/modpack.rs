use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Collection {
    pub default_enabled: bool,
    pub description: Option<String>,
    pub mods: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModPack {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub subscriptions: Vec<String>,
    pub collections: HashMap<String, Collection>,
}
