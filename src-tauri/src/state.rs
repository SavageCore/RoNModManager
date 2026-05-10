use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use reqwest::Client;

use crate::models::{AppConfig, AppError, Result};

pub fn app_directory_name() -> &'static str {
    if cfg!(debug_assertions) {
        "ronmodmanager-dev"
    } else {
        "ronmodmanager"
    }
}

pub fn app_data_root() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| AppError::NotFound("LOCALAPPDATA is not set".to_string()))?;
        return Ok(PathBuf::from(local_app_data).join(app_directory_name()));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let home =
            std::env::var("HOME").map_err(|_| AppError::NotFound("HOME is not set".to_string()))?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(app_directory_name()))
    }
}

pub fn app_config_root() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| AppError::NotFound("APPDATA is not set".to_string()))?;
        return Ok(PathBuf::from(appdata).join(app_directory_name()));
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(xdg_config).join(app_directory_name()));
        }

        let home =
            std::env::var("HOME").map_err(|_| AppError::NotFound("HOME is not set".to_string()))?;

        Ok(PathBuf::from(home)
            .join(".config")
            .join(app_directory_name()))
    }
}

pub fn app_temp_root() -> PathBuf {
    std::env::temp_dir().join(app_directory_name())
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub client: Client,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn load() -> Result<Self> {
        let config_path = default_config_path()?;
        let config = load_config_from_path(&config_path)?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            client: Client::new(),
            config_path,
        })
    }

    pub fn get_config(&self) -> Result<AppConfig> {
        self.config
            .read()
            .map(|guard| guard.clone())
            .map_err(|_| AppError::Validation("failed to lock config for read".to_string()))
    }

    pub fn update_config<F>(&self, update_fn: F) -> Result<AppConfig>
    where
        F: FnOnce(&mut AppConfig),
    {
        let mut guard = self
            .config
            .write()
            .map_err(|_| AppError::Validation("failed to lock config for write".to_string()))?;

        update_fn(&mut guard);
        save_config_to_path(&self.config_path, &guard)?;

        Ok(guard.clone())
    }
}

impl Default for AppState {
    fn default() -> Self {
        match Self::load() {
            Ok(state) => state,
            Err(_) => {
                let fallback_path = fallback_config_path();
                Self {
                    config: Arc::new(RwLock::new(AppConfig::default())),
                    client: Client::new(),
                    config_path: fallback_path,
                }
            }
        }
    }
}

pub fn load_config_from_path(path: &PathBuf) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let contents = fs::read(path)?;
    let config = serde_json::from_slice::<AppConfig>(&contents)?;
    Ok(config)
}

pub fn save_config_to_path(path: &PathBuf, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let data = serde_json::to_vec_pretty(config)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, data)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn default_config_path() -> Result<PathBuf> {
    Ok(app_config_root()?.join("config.json"))
}

fn fallback_config_path() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join("ronmodmanager.config.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_config_returns_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let loaded = load_config_from_path(&config_path).unwrap();

        assert!(loaded.game_path.is_none());
        assert!(loaded.subscribed_mods.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nested").join("config.json");

        let config = AppConfig {
            modpack_url: Some("https://mods.example.com/savagepack".to_string()),
            modpack_version: Some("1.2.0".to_string()),
            ..Default::default()
        };

        save_config_to_path(&config_path, &config).unwrap();
        let loaded = load_config_from_path(&config_path).unwrap();

        assert_eq!(loaded.modpack_url, config.modpack_url);
        assert_eq!(loaded.modpack_version, config.modpack_version);
    }

    #[test]
    fn save_config_is_atomic_tmp_then_rename() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        save_config_to_path(&config_path, &AppConfig::default()).unwrap();

        assert!(config_path.exists());
        assert!(!config_path.with_extension("json.tmp").exists());
    }
}
