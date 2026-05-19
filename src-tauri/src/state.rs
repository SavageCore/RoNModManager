use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use reqwest::Client;

use crate::models::config::AppConfig;
use crate::models::{AppError, Result};

pub fn app_directory_name() -> &'static str {
    if cfg!(debug_assertions) {
        "ronmodmanager-dev"
    } else {
        "ronmodmanager"
    }
}

pub fn app_config_root() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|p| p.join(app_directory_name()))
        .ok_or_else(|| AppError::Validation("Could not find config directory".to_string()))
}

pub fn app_data_root() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|p| p.join(app_directory_name()))
        .ok_or_else(|| AppError::Validation("Could not find data directory".to_string()))
}

pub fn app_temp_root() -> PathBuf {
    std::env::temp_dir().join(app_directory_name())
}

pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub client: Client,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn load() -> Result<Self> {
        let config_path = default_config_path()?;
        println!("[AppState] Loading config from {:?}", config_path);

        let mut config = load_config_from_path(&config_path).map_err(|e| {
            eprintln!("[AppState] Error loading config: {}", e);
            e
        })?;

        let client = Client::builder()
            .user_agent("RoNModManager/0.1.0")
            .build()
            .map_err(|e| AppError::Validation(format!("failed to create http client: {}", e)))?;

        // If modio_game_id is missing but modio_api_key is present, look up and cache it
        if config.modio_game_id.is_none() {
            if let Some(api_key) = config.modio_api_key.clone() {
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| AppError::Validation(format!("tokio runtime error: {}", e)))?;
                match rt.block_on(async {
                    let service = crate::services::modio_api::ModioApiService::new(
                        client.clone(),
                        config.modio_game_id,
                    );
                    service.lookup_game_id(&api_key, "readyornot").await
                }) {
                    Ok(game_id) => {
                        println!("[AppState] Looked up mod.io game_id: {}", game_id);
                        config.modio_game_id = Some(game_id);
                        // Save to config file
                        if let Err(e) = save_config_to_path(&config_path, &config) {
                            eprintln!("[AppState] Failed to save config with game_id: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("[AppState] Failed to look up mod.io game_id: {}", e);
                    }
                }
            }
        }

        Ok(Self {
            config: RwLock::new(config),
            client,
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
            Err(e) => {
                let config_path = default_config_path().unwrap_or_else(|_| {
                    let fallback = PathBuf::from("config.json");
                    eprintln!("[AppState] CRITICAL: Could not determine system config path. Falling back to {:?}", fallback);
                    fallback
                });

                eprintln!(
                    "[AppState] Warning: Failed to load config: {}. Using default config at {:?}",
                    e, config_path
                );

                AppState {
                    config: RwLock::new(AppConfig::default()),
                    client: Client::new(),
                    config_path,
                }
            }
        }
    }
}

pub fn load_config_from_path(path: &PathBuf) -> Result<AppConfig> {
    if !path.exists() {
        println!(
            "[AppState] Config file does not exist at {:?}, using default",
            path
        );
        return Ok(AppConfig::default());
    }

    let contents = fs::read(path).map_err(|e| {
        AppError::Validation(format!("Failed to read config file at {:?}: {}", path, e))
    })?;

    let config = serde_json::from_slice::<AppConfig>(&contents).map_err(|e| {
        AppError::Validation(format!("Failed to parse config file at {:?}: {}", path, e))
    })?;

    Ok(config)
}

pub fn save_config_to_path(path: &PathBuf, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| {
                AppError::Validation(format!(
                    "Failed to create config directory at {:?}: {}",
                    parent, e
                ))
            })?;
        }
    }

    let data = serde_json::to_vec_pretty(config)
        .map_err(|e| AppError::Validation(format!("Failed to serialize config: {}", e)))?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, data).map_err(|e| {
        AppError::Validation(format!(
            "Failed to write temporary config file at {:?}: {}",
            tmp_path, e
        ))
    })?;

    fs::rename(&tmp_path, path).map_err(|e| {
        AppError::Validation(format!(
            "Failed to rename config file from {:?} to {:?}: {}",
            tmp_path, path, e
        ))
    })?;

    Ok(())
}

fn default_config_path() -> Result<PathBuf> {
    Ok(app_config_root()?.join("config.json"))
}
