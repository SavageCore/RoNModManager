use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

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

pub fn app_temp_root() -> Result<PathBuf> {
    app_data_root().map(|p| p.join("tmp"))
}

pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub client: Client,
    /// Overrides the Nexus API endpoint. `None` in production; tests point it
    /// at a local mock server.
    pub nexus_base_url: Option<String>,
    pub config_path: PathBuf,
    /// Cancel flags for in-progress free Nexus manual-download waits, keyed by
    /// a per-invocation wait id. The old single global flag meant starting
    /// mod2 cleared a cancel meant for mod1 (and vice versa); per-wait flags
    /// keep concurrent manual downloads independent.
    /// `cancel_nexus_download` with no id cancels all active waits
    /// (used by the "close app while waiting" dialog).
    /// Map value is the cancel signal: false = waiting, true = cancelled.
    pub nexus_cancel: Arc<Mutex<HashMap<u64, bool>>>,
    pub nexus_wait_id: Arc<AtomicU64>,
    /// Timestamps of the last `open_url` for a Nexus mod's `?tab=files` page,
    /// used to suppress duplicate browser-tab opens when several files of the
    /// same mod are installed at once (e.g. a Part 1 + Part 2 multipart mod).
    pub nexus_open_throttle: Arc<Mutex<HashMap<u64, Instant>>>,
    /// When true, the next app exit skips stock cleanup. Set during the
    /// launch-close path so the Steam-URI launch is not raced by an immediate
    /// unlink of the mods it is about to load.
    pub suppress_exit_cleanup: AtomicBool,
    /// True while a game-exit watcher thread is active. Ensures only one
    /// watcher runs at a time across repeated launches.
    pub game_watcher_running: AtomicBool,
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
            nexus_base_url: None,
            config_path,
            nexus_cancel: Arc::new(Mutex::new(HashMap::new())),
            nexus_wait_id: Arc::new(AtomicU64::new(1)),
            nexus_open_throttle: Arc::new(Mutex::new(HashMap::new())),
            suppress_exit_cleanup: AtomicBool::new(false),
            game_watcher_running: AtomicBool::new(false),
        })
    }

    pub fn get_config(&self) -> Result<AppConfig> {
        self.config
            .read()
            .map(|guard| guard.clone())
            .map_err(|_| AppError::Validation("failed to lock config for read".to_string()))
    }

    /// Allocate a fresh id for a Nexus manual-download wait and register it
    /// as active (not cancelled). Ids are unique per invocation so concurrent
    /// waits never share cancel state.
    pub fn register_nexus_wait(&self) -> u64 {
        let wait_id = self.nexus_wait_id.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut flags) = self.nexus_cancel.lock() {
            flags.insert(wait_id, false);
        }
        wait_id
    }

    /// True when the given wait has been cancelled. Unknown ids (already
    /// cleaned up, or never registered) read as "not cancelled" - a lock
    /// failure or missing entry must never wedge a download wait loop.
    pub fn is_nexus_wait_cancelled(&self, wait_id: u64) -> bool {
        self.nexus_cancel
            .lock()
            .map(|flags| flags.get(&wait_id).copied().unwrap_or(false))
            .unwrap_or(false)
    }

    /// Remove a wait's cancel flag once the wait finishes (found, timed out,
    /// errored). Keeps the map from growing across many installs.
    pub fn clear_nexus_wait(&self, wait_id: u64) {
        if let Ok(mut flags) = self.nexus_cancel.lock() {
            flags.remove(&wait_id);
        }
    }

    /// Should we open a browser tab for the given Nexus mod id? Returns true
    /// once per mod within a short throttle window so that multipart installs
    /// (one `add_nexus_mod` call per file) do not spawn N identical `?tab=files`
    /// tabs. Callers pass the resolved `mod_id`.
    pub fn should_open_nexus_url(&self, mod_id: u64) -> bool {
        const THROTTLE: std::time::Duration = std::time::Duration::from_secs(15);
        let now = std::time::Instant::now();
        if let Ok(mut map) = self.nexus_open_throttle.lock() {
            match map.get(&mod_id) {
                Some(last) if now.duration_since(*last) < THROTTLE => return false,
                _ => {
                    map.insert(mod_id, now);
                }
            }
        }
        true
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

    /// Nexus API service for this session, honouring the endpoint override so
    /// tests can point the client at a local mock server.
    pub fn nexus(&self) -> crate::services::nexus_api::NexusApiService {
        match self.nexus_base_url.as_deref() {
            Some(base) => crate::services::nexus_api::NexusApiService::with_base_url(
                self.client.clone(),
                base.to_string(),
            ),
            None => crate::services::nexus_api::NexusApiService::new(self.client.clone()),
        }
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
                    nexus_base_url: None,
                    config_path,
                    nexus_cancel: Arc::new(Mutex::new(HashMap::new())),
                    nexus_wait_id: Arc::new(AtomicU64::new(1)),
                    nexus_open_throttle: Arc::new(Mutex::new(HashMap::new())),
                    suppress_exit_cleanup: AtomicBool::new(false),
                    game_watcher_running: AtomicBool::new(false),
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

pub fn default_config_path() -> Result<PathBuf> {
    Ok(app_config_root()?.join("config.json"))
}

pub fn load_config_fallback() -> Result<AppConfig> {
    load_config_from_path(&default_config_path()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::LogLevel;
    use tempfile::TempDir;

    /// AppState pointed at a throwaway config file, so `update_config`
    /// exercises the real file round-trip without touching the user profile.
    fn test_state() -> (TempDir, AppState) {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");
        let state = AppState {
            config: RwLock::new(AppConfig::default()),
            client: Client::new(),
            nexus_base_url: None,
            config_path,
            nexus_cancel: Arc::new(Mutex::new(HashMap::new())),
            nexus_wait_id: Arc::new(AtomicU64::new(1)),
            nexus_open_throttle: Arc::new(Mutex::new(HashMap::new())),
            suppress_exit_cleanup: AtomicBool::new(false),
            game_watcher_running: AtomicBool::new(false),
        };
        (dir, state)
    }

    #[test]
    fn app_directory_name_is_debug_suffixed_in_tests() {
        assert_eq!(app_directory_name(), "ronmodmanager-dev");
    }

    #[test]
    fn update_config_persists_and_reloads() {
        let (_dir, state) = test_state();
        assert_eq!(state.get_config().unwrap().sync_remote_host, None);

        let updated = state
            .update_config(|config| {
                config.sync_remote_host = Some("deploy@example.com".to_string());
                config.active_profile = Some("main".to_string());
            })
            .unwrap();
        assert_eq!(
            updated.sync_remote_host,
            Some("deploy@example.com".to_string())
        );

        let reloaded = load_config_from_path(&state.config_path).unwrap();
        assert_eq!(
            reloaded.sync_remote_host,
            Some("deploy@example.com".to_string())
        );
        assert_eq!(reloaded.active_profile, Some("main".to_string()));
    }

    #[test]
    fn load_config_from_path_missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let config = load_config_from_path(&dir.path().join("missing.json")).unwrap();
        assert_eq!(config.active_profile, None);
    }

    #[test]
    fn load_config_from_path_rejects_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, b"{not json").unwrap();
        assert!(load_config_from_path(&path).is_err());
    }

    #[test]
    fn nexus_wait_lifecycle() {
        let (_dir, state) = test_state();
        // Unknown waits read as not cancelled.
        assert!(!state.is_nexus_wait_cancelled(999));

        let wait_id = state.register_nexus_wait();
        assert!(!state.is_nexus_wait_cancelled(wait_id));

        // Simulate cancellation, then cleanup.
        state.nexus_cancel.lock().unwrap().insert(wait_id, true);
        assert!(state.is_nexus_wait_cancelled(wait_id));
        state.clear_nexus_wait(wait_id);
        assert!(!state.is_nexus_wait_cancelled(wait_id));
    }

    #[test]
    fn nexus_wait_ids_are_unique() {
        let (_dir, state) = test_state();
        assert_ne!(state.register_nexus_wait(), state.register_nexus_wait());
    }

    #[test]
    fn nexus_open_throttle_dedups_within_window() {
        let (_dir, state) = test_state();
        assert!(state.should_open_nexus_url(5933));
        // Within the throttle window: suppressed.
        assert!(!state.should_open_nexus_url(5933));
        // A different mod id is independent.
        assert!(state.should_open_nexus_url(5934));
    }

    #[test]
    fn app_roots_live_under_the_isolated_home() {
        let root = crate::test_support::isolated_root();

        assert!(app_config_root().unwrap().starts_with(root));
        assert!(app_data_root().unwrap().starts_with(root));
        assert!(app_temp_root().unwrap().ends_with("tmp"));
        assert!(default_config_path().unwrap().ends_with("config.json"));
    }

    #[test]
    fn config_file_failures_fall_back_to_defaults() {
        let root = crate::test_support::isolated_root();
        let path = default_config_path().unwrap();
        let _ = fs::remove_file(&path);

        // No config file yet: nothing to load, so defaults apply and the app
        // keeps working on a fresh install.
        let state = AppState::load().unwrap();
        assert!(state.config_path.starts_with(root));
        assert_eq!(state.get_config().unwrap().active_profile, None);
        assert!(load_config_fallback().unwrap().nexus_api_key.is_none());

        // A corrupt file surfaces from `load`, and `Default` still yields a
        // usable state so startup never fails over a bad config.
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"{ not json").unwrap();
        assert!(AppState::load().is_err());

        let fallback = AppState::default();
        assert!(fallback.config_path.starts_with(root));
        assert_eq!(fallback.get_config().unwrap().log_level, LogLevel::Info);

        let _ = fs::remove_file(&path);
    }
}
