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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub game_path: Option<PathBuf>,
    #[serde(default)]
    pub modpack_url: Option<String>,
    #[serde(default)]
    pub modpack_version: Option<String>,
    #[serde(default)]
    pub oauth_token: Option<String>,
    #[serde(default)]
    pub nexus_api_key: Option<String>,
    #[serde(default)]
    pub active_profile: Option<String>,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub window_width: Option<f64>,
    #[serde(default)]
    pub window_height: Option<f64>,
    #[serde(default)]
    pub window_x: Option<f64>,
    #[serde(default)]
    pub window_y: Option<f64>,
    #[serde(default)]
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
