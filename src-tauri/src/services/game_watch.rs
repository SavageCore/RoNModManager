use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::game::sync_mod_links_for_game_path;
use crate::state::AppState;

/// How often the watcher polls for the game process.
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// How long to wait for the game to appear after a Steam-URI launch before
/// giving up and restoring the folder to stock anyway. Kept short so a
/// cancelled Steam launch/dialog does not leave mods linked at rest.
const APPEAR_TIMEOUT: Duration = Duration::from_secs(60);

/// Payload of the `game-running` frontend event.
#[derive(Debug, Clone, Serialize)]
struct GameRunningEvent {
    running: bool,
}

fn emit_game_running(app: &AppHandle, running: bool) {
    let _ = app.emit("game-running", &GameRunningEvent { running });
}

/// Substring match for the game process, case-insensitive. Covers the native
/// `ReadyOrNot.exe`, the `ReadyOrNot-Win64-Shipping.exe` shipping binary, and
/// Proton/wine processes whose `/proc` comm is truncated to 15 chars
/// (`ReadyOrNot-Win`).
pub(crate) fn process_name_matches(candidate: &str) -> bool {
    candidate.to_lowercase().contains("readyornot")
}

/// True while the game process is alive. No new dependencies: `/proc` scan on
/// Linux, `tasklist` on Windows, always false elsewhere.
pub fn is_game_running() -> bool {
    #[cfg(target_os = "linux")]
    {
        is_game_running_linux()
    }
    #[cfg(target_os = "windows")]
    {
        is_game_running_windows()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

/// True inside a Flatpak sandbox, where our own `/proc` only shows sandboxed
/// processes and the host game is invisible without `flatpak-spawn`.
#[cfg(target_os = "linux")]
fn running_in_flatpak_sandbox() -> bool {
    std::path::Path::new("/.flatpak-info").exists() || std::env::var("FLATPAK_ID").is_ok()
}

/// Whether the watcher can actually observe the game process. Inside a
/// Flatpak sandbox this requires `flatpak-spawn`; without it the host is
/// invisible and a watcher would wrongly conclude the game never runs (and
/// then unlink mods from under a live game on its appear-timeout).
pub fn host_process_scan_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        if !running_in_flatpak_sandbox() {
            return true;
        }
        std::process::Command::new("flatpak-spawn")
            .args(["--host", "true"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

/// pgrep exit codes: 0 = match, 1 = no match, anything else = error.
/// `None` means "unknown, try another method".
pub(crate) fn interpret_pgrep_result(code: Option<i32>, stdout_empty: bool) -> Option<bool> {
    match code {
        Some(0) => Some(!stdout_empty),
        Some(1) => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn is_game_running_linux() -> bool {
    if running_in_flatpak_sandbox() {
        return is_game_running_host();
    }
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str() else { continue };
        if pid.bytes().any(|b| !b.is_ascii_digit()) {
            continue;
        }
        // `comm` is truncated to 15 chars but still contains "readyornot".
        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) {
            if process_name_matches(comm.trim()) {
                return true;
            }
        }
        // `cmdline` catches wine/proton wrappers launched with the exe path.
        if let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) {
            if process_name_matches(&String::from_utf8_lossy(&cmdline)) {
                return true;
            }
        }
    }
    false
}

/// Query host processes from inside the Flatpak sandbox. The `[r]` bracket
/// trick stops pgrep/grep from matching their own command lines (which
/// contain the pattern text).
#[cfg(target_os = "linux")]
fn is_game_running_host() -> bool {
    if let Some(running) = run_host_pgrep() {
        return running;
    }
    is_game_running_host_grep_fallback()
}

#[cfg(target_os = "linux")]
fn run_host_pgrep() -> Option<bool> {
    let output = std::process::Command::new("flatpak-spawn")
        .args(["--host", "pgrep", "-i", "-f", "[r]eadyornot"])
        .output()
        .ok()?;
    interpret_pgrep_result(output.status.code(), output.stdout.is_empty())
}

#[cfg(target_os = "linux")]
fn is_game_running_host_grep_fallback() -> bool {
    let Ok(output) = std::process::Command::new("flatpak-spawn")
        .args([
            "--host",
            "sh",
            "-c",
            "grep -lis '[r]eadyornot' /proc/[0-9]*/comm /proc/[0-9]*/cmdline 2>/dev/null",
        ])
        .output()
    else {
        return false;
    };
    output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
}

#[cfg(target_os = "windows")]
fn is_game_running_windows() -> bool {
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    else {
        return false;
    };
    process_name_matches(&String::from_utf8_lossy(&output.stdout))
}

/// Watch the game process in the background and restore the game folder to
/// stock once it exits. Only one watcher runs at a time (guarded by
/// `AppState::game_watcher_running`); extra spawn requests are ignored since
/// the active watcher already covers the current links.
///
/// Callers must only spawn this when link-on-launch-only mode is enabled:
///
/// - Phase 1: wait up to `APPEAR_TIMEOUT` for the game to appear after launch.
///   If it never does, restore to stock anyway so the folder does not stay
///   modded at rest.
/// - Phase 2: once seen, poll until the process disappears, then restore to
///   stock and emit `game-running { running: false }`.
///
/// Does nothing when the host processes are not observable (sandboxed without
/// `flatpak-spawn`): a blind watcher would time out and unlink mods from
/// under a live game.
pub fn spawn_game_exit_watcher(app: AppHandle, game_path: PathBuf) {
    if !host_process_scan_available() {
        log::warn!(
            "game watcher: host processes not visible (sandboxed without flatpak-spawn); not watching"
        );
        return;
    }
    let state = app.state::<AppState>();
    if state
        .game_watcher_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::debug!("game watcher already running; skipping duplicate");
        return;
    }

    std::thread::spawn(move || {
        // Reset the single-watcher guard on every exit path.
        let _guard = WatcherGuard { app: &app };

        // Phase 1: wait for the game to appear.
        let mut waited = Duration::ZERO;
        let mut seen = is_game_running();
        while !seen && waited < APPEAR_TIMEOUT {
            std::thread::sleep(POLL_INTERVAL);
            waited += POLL_INTERVAL;
            seen = is_game_running();
        }

        if seen {
            log::info!("game watcher: game process detected");
            emit_game_running(&app, true);
            // Phase 2: wait for exit.
            while is_game_running() {
                std::thread::sleep(POLL_INTERVAL);
            }
            log::info!("game watcher: game exited, restoring stock folder");
        } else {
            log::warn!("game watcher: game never appeared after launch, restoring stock folder");
        }

        if let Err(e) = sync_mod_links_for_game_path(&game_path, Vec::new()) {
            log::warn!("game watcher: failed to restore stock folder: {e}");
        }
        emit_game_running(&app, false);
    });

    struct WatcherGuard<'a> {
        app: &'a AppHandle,
    }

    impl Drop for WatcherGuard<'_> {
        fn drop(&mut self) {
            self.app
                .state::<AppState>()
                .game_watcher_running
                .store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_native_and_shipping_exe_names() {
        assert!(process_name_matches("ReadyOrNot.exe"));
        assert!(process_name_matches("ReadyOrNot-Win64-Shipping.exe"));
        assert!(process_name_matches(
            "\"C:\\Games\\Ready Or Not\\ReadyOrNot.exe\""
        ));
    }

    #[test]
    fn matches_truncated_proc_comm_and_wine_cmdlines() {
        // /proc comm is truncated to 15 chars.
        assert!(process_name_matches("ReadyOrNot-Win"));
        assert!(process_name_matches(
            "Z:\\home\\user\\.steam\\steam\\steamapps\\common\\Ready Or Not\\ReadyOrNot\\Binaries\\Win64\\ReadyOrNot-Win64-Shipping.exe"
        ));
    }

    #[test]
    fn rejects_unrelated_processes() {
        assert!(!process_name_matches("steam.exe"));
        assert!(!process_name_matches("explorer.exe"));
        assert!(!process_name_matches("ronmodmanager"));
        assert!(!process_name_matches(""));
    }

    #[test]
    fn pgrep_exit_codes_interpreted() {
        assert_eq!(interpret_pgrep_result(Some(0), false), Some(true));
        assert_eq!(interpret_pgrep_result(Some(0), true), Some(false));
        assert_eq!(interpret_pgrep_result(Some(1), true), Some(false));
        assert_eq!(interpret_pgrep_result(Some(2), true), None);
        assert_eq!(interpret_pgrep_result(None, true), None);
    }
}
