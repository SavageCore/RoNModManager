use std::path::PathBuf;

use chrono::{DateTime, Utc};
use log::LevelFilter;
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
#[serde(rename_all = "lowercase")]
pub enum OnGameLaunchAction {
    #[default]
    Nothing,
    Minimize,
    Close,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum CloseAction {
    #[default]
    Quit,
    Minimize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum MinimizeTarget {
    #[default]
    Taskbar,
    Tray,
}

/// Backend log verbosity. Maps directly to `log::LevelFilter`, is persisted in
/// config.json, and is overridable at launch with the standard `RUST_LOG`
/// environment variable (which wins over the stored setting).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Trace => LevelFilter::Trace,
        }
    }
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
    pub modio_api_key: Option<String>,
    #[serde(default)]
    pub modio_game_id: Option<u32>,
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
    #[serde(default)]
    pub intro_skip_enabled: bool,
    #[serde(default)]
    pub last_export_dir: Option<String>,
    #[serde(default)]
    pub sync_remote_host: Option<String>,
    #[serde(default)]
    pub sync_remote_path: Option<String>,
    #[serde(default)]
    pub on_game_launch: OnGameLaunchAction,
    #[serde(default)]
    pub close_action: CloseAction,
    #[serde(default)]
    pub minimize_target: MinimizeTarget,
    #[serde(default)]
    pub asked_close_preference: bool,
    #[serde(default)]
    pub setup_wizard_complete: bool,
    #[serde(default)]
    pub tutorial_complete: bool,
    #[serde(default)]
    pub optimization_enabled: bool,
    #[serde(default)]
    pub optimization_profile: Option<String>,
    #[serde(default)]
    pub log_level: LogLevel,
    #[serde(default)]
    pub link_on_launch_only: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            game_path: None,
            modpack_url: None,
            modpack_version: None,
            oauth_token: None,
            modio_api_key: None,
            modio_game_id: None,
            nexus_api_key: None,
            active_profile: None,
            theme: ThemeMode::System,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            last_update_check: None,
            intro_skip_enabled: false,
            last_export_dir: None,
            sync_remote_host: None,
            sync_remote_path: None,
            on_game_launch: OnGameLaunchAction::Nothing,
            close_action: CloseAction::Quit,
            minimize_target: MinimizeTarget::Taskbar,
            asked_close_preference: false,
            setup_wizard_complete: false,
            tutorial_complete: false,
            optimization_enabled: false,
            optimization_profile: None,
            log_level: LogLevel::Info,
            link_on_launch_only: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_maps_to_filter() {
        assert_eq!(LevelFilter::from(LogLevel::Error), LevelFilter::Error);
        assert_eq!(LevelFilter::from(LogLevel::Warn), LevelFilter::Warn);
        assert_eq!(LevelFilter::from(LogLevel::Info), LevelFilter::Info);
        assert_eq!(LevelFilter::from(LogLevel::Debug), LevelFilter::Debug);
        assert_eq!(LevelFilter::from(LogLevel::Trace), LevelFilter::Trace);
    }

    #[test]
    fn log_level_serde_round_trip() {
        for level in [
            LogLevel::Error,
            LogLevel::Warn,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Trace,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            assert_eq!(serde_json::from_str::<LogLevel>(&json).unwrap(), level);
        }
    }

    #[test]
    fn app_config_deserializes_sparse_json_with_defaults() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.active_profile, None);
        assert_eq!(config.sync_remote_host, None);
        assert_eq!(config.log_level, LogLevel::Info);
        assert!(!config.setup_wizard_complete);
        assert!(!config.tutorial_complete);
    }

    #[test]
    fn app_config_round_trip() {
        let config = AppConfig {
            sync_remote_host: Some("deploy@example.com".to_string()),
            active_profile: Some("main".to_string()),
            ..AppConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.sync_remote_host,
            Some("deploy@example.com".to_string())
        );
        assert_eq!(back.active_profile, Some("main".to_string()));
    }
}
