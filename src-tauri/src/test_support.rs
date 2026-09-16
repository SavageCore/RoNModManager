//! Test-only helpers shared by the command and service test modules.
//!
//! Command tests need two things the production code takes for granted: a
//! Tauri `App` to hand out `State<AppState>`, and config/data directories that
//! are not the developer's real ones (a command test must never touch the
//! user's profiles or write an `Engine.ini` into a real Proton prefix).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::App;

use crate::models::config::AppConfig;
use crate::state::AppState;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Root of the throwaway tree this test process reads and writes.
///
/// The first call redirects the platform's config/data/home environment
/// variables - everything the app resolves through `dirs` or `std::env::var`
/// (`state::app_data_root`, `state::app_config_root`, Steam discovery, the
/// download and desktop directories) - into a leaked temp directory, and seeds
/// the handful of files those lookups expect to find:
///
/// * `user-dirs.dirs`, so `dirs::download_dir` / `dirs::desktop_dir` resolve
///   instead of returning `None` on a machine without a desktop session.
/// * A fake `~/.steam/steam/steamapps`, so Steam lookups land in the tree
///   rather than the developer's real Steam library.
///
/// The directory is leaked deliberately: it has to outlive every test in the
/// binary, and tearing it down at the end of a run buys nothing.
///
/// Note this is process-global, so tests that need the real environment must
/// not run alongside the ones that call this.
pub fn isolated_root() -> &'static Path {
    ROOT.get_or_init(|| {
        let root = tempfile::TempDir::new().unwrap().keep();

        #[cfg(unix)]
        {
            std::env::set_var("HOME", &root);
            std::env::set_var("XDG_DATA_HOME", root.join("data"));
            std::env::set_var("XDG_CONFIG_HOME", root.join("config"));

            fs::create_dir_all(root.join("config")).unwrap();
            fs::write(
                root.join("config").join("user-dirs.dirs"),
                "XDG_DOWNLOAD_DIR=\"$HOME/downloads\"\nXDG_DESKTOP_DIR=\"$HOME/desktop\"\n",
            )
            .unwrap();
            fs::create_dir_all(root.join("downloads")).unwrap();
            fs::create_dir_all(root.join("desktop")).unwrap();
            fs::create_dir_all(root.join(".steam/steam/steamapps")).unwrap();
        }
        #[cfg(windows)]
        {
            std::env::set_var("APPDATA", root.join("data"));
            std::env::set_var("LOCALAPPDATA", root.join("data"));
            std::env::set_var("USERPROFILE", &root);
        }

        root
    })
}

/// A `TempDir` under the isolated root, for tests that want their own corner
/// of the tree.
pub fn scratch_dir(label: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(label)
        .tempdir_in(isolated_root())
        .unwrap()
}

/// An `AppState` that writes its config inside the isolated root instead of
/// the real user config file.
///
/// Each call gets its own config file: `update_config` writes a temp file and
/// renames it into place, so two tests sharing one path would race.
pub fn test_state(config: AppConfig) -> AppState {
    isolated_root();
    let config_path = tempfile::TempDir::new_in(isolated_root())
        .unwrap()
        .keep()
        .join("config.json");
    AppState {
        config: RwLock::new(config),
        client: reqwest::Client::new(),
        nexus_base_url: None,
        config_path,
        nexus_cancel: Arc::new(Mutex::new(HashMap::new())),
        nexus_wait_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        nexus_open_throttle: Arc::new(Mutex::new(HashMap::new())),
        suppress_exit_cleanup: AtomicBool::new(false),
        game_watcher_running: AtomicBool::new(false),
    }
}

/// A mock Tauri app managing `test_state(config)`, for exercising command
/// functions that take `State<'_, AppState>`.
pub fn mock_app_with(config: AppConfig) -> TestApp {
    mock_app_with_state(test_state(config))
}

/// A mock Tauri app managing a caller-built `AppState`.
pub fn mock_app_with_state(state: AppState) -> TestApp {
    isolated_root();
    mock_builder()
        .manage(state)
        .build(mock_context(noop_assets()))
        .unwrap()
}

/// Shorthand for the app type the mock builders hand back.
pub type TestApp = App<MockRuntime>;

/// Serialises tests that read or write the process-wide isolated tree.
///
/// `isolated_root` pins `$HOME`/`$XDG_*` for the whole process, so the app data
/// dir, the staged tree, `addon_map.json` and the fake Steam tree are shared by
/// every test in the binary. Tests that mutate them take this guard, and so do
/// the tests that assert a given file or install is absent, so the two groups
/// never overlap. Nothing guarded here does network I/O, so serialising costs
/// little.
///
/// Async tests hold the guard across awaits on purpose (that is the point:
/// the whole test body is the critical section), which trips
/// `clippy::await_holding_lock`. Each `#[tokio::test]` gets its own
/// current-thread runtime and no guarded test waits on another, so there is
/// nothing to deadlock against - those tests carry a local allow.
pub fn shared_tree_guard() -> std::sync::MutexGuard<'static, ()> {
    static SHARED_TREE_LOCK: Mutex<()> = Mutex::new(());
    SHARED_TREE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
