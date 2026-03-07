use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SubscribedMod {
    pub md5: String,
    pub filename: String,
    pub download_url: String,
    pub contents: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub game_path: Option<PathBuf>,
    pub modpack_url: Option<String>,
    pub modpack_version: Option<String>,
    pub oauth_token: Option<String>,
    pub nexus_api_key: Option<String>,
    pub subscribed_mods: HashMap<String, SubscribedMod>,
    pub collections: HashMap<String, bool>,
    pub enabled_collections: Vec<String>,
    pub active_profile: Option<String>,
    pub theme: ThemeMode,
    pub window_width: Option<f64>,
    pub window_height: Option<f64>,
    pub window_x: Option<f64>,
    pub window_y: Option<f64>,
    pub last_update_check: Option<DateTime<Utc>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            game_path: None,
            modpack_url: None,
            modpack_version: None,
            oauth_token: None,
            nexus_api_key: None,
            subscribed_mods: HashMap::new(),
            collections: HashMap::new(),
            enabled_collections: Vec::new(),
            active_profile: None,
            theme: ThemeMode::System,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            last_update_check: None,
        }
    }
}
