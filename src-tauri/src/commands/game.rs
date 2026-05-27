use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tauri::State;

use crate::commands::mods::archive_install_key;
use crate::models::AppError;
use crate::services::manifest;
use crate::services::steam;
use crate::state::{app_data_root, AppState};

fn get_local_store_root() -> Result<PathBuf, String> {
    app_data_root().map_err(|e| e.to_string())
}

fn get_staging_root() -> Result<PathBuf, String> {
    let root = get_local_store_root()?.join("staged");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn create_file_link(src: &Path, dst: &Path) -> Result<(), String> {
    // Remove any existing file or symlink at the destination so re-installing
    // the same mod doesn't fail with "File exists (os error 17)".
    if dst.exists() || dst.is_symlink() {
        fs::remove_file(dst).map_err(|e| format!("Failed to remove existing link: {}", e))?;
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst).map_err(|e| e.to_string())
    }

    // On Windows, symlinks require SeCreateSymbolicLinkPrivilege (or Developer
    // Mode). Fall back to a hard link (same volume) or plain copy if that
    // privilege is absent (error 1314).
    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_file(src, dst) {
            Ok(()) => Ok(()),
            Err(e) if e.raw_os_error() == Some(1314) => fs::hard_link(src, dst)
                .or_else(|_| fs::copy(src, dst).map(|_| ()))
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn target_path_for_staged_file(
    staged_path: &Path,
    live_mods_path: &Path,
    live_savegames_path: &Path,
) -> Option<PathBuf> {
    let file_name = staged_path.file_name()?;
    let ext = staged_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    if ext.eq_ignore_ascii_case("pak") {
        return Some(live_mods_path.join(file_name));
    }
    if ext.eq_ignore_ascii_case("sav") {
        return Some(live_savegames_path.join(file_name));
    }
    None
}

/// If `staged_path` is a staged override file, returns `(game_target, backup_path)`.
/// Override files live under `staged/mods/<key>/` with subdirectory structure preserved
/// (e.g. `staged/mods/<key>/ReadyOrNot/Content/Movies/foo.mp4`). Flat files like .pak
/// and .bank sit directly under `<key>/` with no parent directory, so they are excluded.
fn override_paths(
    staged_path: &Path,
    staging_root: &Path,
    game_path: &Path,
) -> Option<(PathBuf, PathBuf)> {
    let staged_mods_root = staging_root.join("mods");
    if !staged_path.starts_with(&staged_mods_root) {
        return None;
    }
    let relative_with_key = staged_path.strip_prefix(&staged_mods_root).ok()?;
    let mut components = relative_with_key.components();
    let key = components.next()?;
    let game_relative: PathBuf = components.collect();
    // Flat files (pak, bank) have no parent dir - only nested paths are overrides
    if game_relative
        .parent()
        .map(|p| p.as_os_str().is_empty())
        .unwrap_or(true)
    {
        return None;
    }
    let game_target = game_path.join(&game_relative);
    let backup = staging_root
        .join("backups")
        .join(key)
        .join("overrides")
        .join(&game_relative);
    Some((game_target, backup))
}

fn launch_game_internal(game_path: &Path, intro_skip_enabled: bool) -> Result<(), String> {
    // Re-apply intro skip if enabled in config, in case a game update restored the files
    if intro_skip_enabled {
        let _ = crate::services::config_tweaks::apply_intro_skip(game_path);
    }

    #[cfg(target_os = "windows")]
    {
        let exe_path = game_path.join("ReadyOrNot.exe");
        if !exe_path.exists() {
            return Err(format!(
                "Game executable not found at {}",
                exe_path.display()
            ));
        }
        Command::new("cmd")
            .args(&["/C", &format!("start \"\" \"{}\"", exe_path.display())])
            .spawn()
            .map_err(|e| format!("Failed to launch game: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Use xdg-open so the steam:// URI is handled via D-Bus portals, which
        // works both natively and inside a Flatpak sandbox (where `steam` is not
        // on the sandboxed PATH).
        Command::new("xdg-open")
            .arg("steam://rungameid/1144200")
            .spawn()
            .map_err(|e| format!("Failed to launch game via Steam: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn detect_game_path(_state: State<'_, AppState>) -> Result<Option<String>, String> {
    match steam::detect_game_path() {
        Ok(path) => Ok(Some(path.to_string_lossy().to_string())),
        Err(AppError::NotFound(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub async fn set_game_path(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err("Game path does not exist".to_string());
    }

    state
        .update_config(|config| {
            config.game_path = Some(path_buf);
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn launch_game(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(|e: AppError| e.to_string())?;

    let game_path = config
        .game_path
        .ok_or_else(|| "Game path not configured".to_string())?;

    launch_game_internal(&game_path, config.intro_skip_enabled)
}

fn remove_orphan_symlinks(live_mods_path: &Path, live_savegames_path: &Path) -> Result<(), String> {
    // Remove broken symlinks in mods folder
    if let Ok(entries) = fs::read_dir(live_mods_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Check if it's a symlink and the target doesn't exist
            if path.is_symlink() {
                if let Ok(target) = fs::read_link(&path) {
                    if !target.exists() {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // Remove broken symlinks in savegames folder
    if let Ok(entries) = fs::read_dir(live_savegames_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Check if it's a symlink and the target doesn't exist
            if path.is_symlink() {
                if let Ok(target) = fs::read_link(&path) {
                    if !target.exists() {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn sync_mod_links_for_game_path(
    game_path: &Path,
    enabled_groups: Vec<String>,
) -> Result<(), String> {
    let live_mods_path = steam::get_mods_path(game_path);
    let live_savegames_path = steam::get_savegames_path().map_err(|e| e.to_string())?;
    let live_fmod_path = steam::get_fmod_desktop_path(game_path);
    let live_config_path = steam::get_config_path().map_err(|e| e.to_string())?;
    fs::create_dir_all(&live_mods_path).map_err(|e| e.to_string())?;
    fs::create_dir_all(&live_savegames_path).map_err(|e| e.to_string())?;

    let enabled: HashSet<String> = enabled_groups.into_iter().collect();
    let staging_root = get_staging_root()?;
    let manager = manifest::ManifestManager::new(&staging_root);
    let manifests = manager.list_all_manifests().unwrap_or_default();

    // First remove all managed live links/files for tracked staged files.
    // For .bank and .ini files, restore the original from the backup rather than just deleting.
    // For override files, remove the symlink and restore the backed-up original.
    for manifest_data in manifests.values() {
        for staged_path in &manifest_data.installed_files {
            let ext = staged_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            let is_bank = ext.eq_ignore_ascii_case("bank");
            let is_ini = ext.eq_ignore_ascii_case("ini");

            if is_bank {
                let Some(file_name) = staged_path.file_name() else {
                    continue;
                };
                let game_dest = live_fmod_path.join(file_name);
                let install_key = archive_install_key(&manifest_data.source_archive);
                let backup = staging_root
                    .join("backups")
                    .join(&install_key)
                    .join(file_name);
                if game_dest.is_symlink() || game_dest.exists() {
                    let _ = fs::remove_file(&game_dest);
                }
                if backup.exists() {
                    let _ = fs::copy(&backup, &game_dest);
                }
            } else if is_ini {
                let Some(file_name) = staged_path.file_name() else {
                    continue;
                };
                let game_dest = live_config_path.join(file_name);
                let install_key = archive_install_key(&manifest_data.source_archive);
                let backup = staging_root
                    .join("backups")
                    .join(&install_key)
                    .join(file_name);
                if game_dest.is_symlink() || game_dest.exists() {
                    let _ = fs::remove_file(&game_dest);
                }
                if backup.exists() {
                    let _ = fs::copy(&backup, &game_dest);
                }
            } else if let Some((game_target, backup)) =
                override_paths(staged_path, &staging_root, game_path)
            {
                if game_target.exists() || game_target.is_symlink() {
                    let _ = fs::remove_file(&game_target);
                }
                if backup.exists() {
                    if let Some(parent) = game_target.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::copy(&backup, &game_target);
                }
            } else if let Some(target) =
                target_path_for_staged_file(staged_path, &live_mods_path, &live_savegames_path)
            {
                if target.exists() {
                    let _ = fs::remove_file(&target);
                }
            }
        }
    }

    // Then link/copy only enabled groups.
    for manifest_data in manifests.values() {
        if !enabled.contains(&manifest_data.source_archive) {
            continue;
        }
        for staged_path in &manifest_data.installed_files {
            if !staged_path.exists() {
                continue;
            }

            let ext = staged_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            let is_bank = ext.eq_ignore_ascii_case("bank");
            let is_ini = ext.eq_ignore_ascii_case("ini");

            if is_bank {
                let Some(file_name) = staged_path.file_name() else {
                    continue;
                };
                fs::create_dir_all(&live_fmod_path).map_err(|e| e.to_string())?;
                let game_dest = live_fmod_path.join(file_name);
                // Remove the original (already backed up at install time) then symlink.
                if game_dest.exists() || game_dest.is_symlink() {
                    let _ = fs::remove_file(&game_dest);
                }
                create_file_link(staged_path, &game_dest)?;
            } else if is_ini {
                let Some(file_name) = staged_path.file_name() else {
                    continue;
                };
                fs::create_dir_all(&live_config_path).map_err(|e| e.to_string())?;
                let game_dest = live_config_path.join(file_name);
                // Remove the original (already backed up at install time) then symlink.
                if game_dest.exists() || game_dest.is_symlink() {
                    let _ = fs::remove_file(&game_dest);
                }
                create_file_link(staged_path, &game_dest)?;
            } else if let Some((game_target, _)) =
                override_paths(staged_path, &staging_root, game_path)
            {
                if let Some(parent) = game_target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                if game_target.exists() || game_target.is_symlink() {
                    let _ = fs::remove_file(&game_target);
                }
                create_file_link(staged_path, &game_target)?;
            } else {
                let Some(target) =
                    target_path_for_staged_file(staged_path, &live_mods_path, &live_savegames_path)
                else {
                    continue;
                };
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                create_file_link(staged_path, &target)?;
            }
        }
    }

    // Clean up any orphaned symlinks (pointing to non-existent files)
    remove_orphan_symlinks(&live_mods_path, &live_savegames_path)?;

    Ok(())
}

#[tauri::command]
pub async fn sync_mod_links(
    state: State<'_, AppState>,
    enabled_groups: Vec<String>,
) -> Result<(), String> {
    let config = state.get_config().map_err(|e: AppError| e.to_string())?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path not configured".to_string())?;

    sync_mod_links_for_game_path(&game_path, enabled_groups)
}

#[tauri::command]
pub async fn launch_game_with_groups(
    state: State<'_, AppState>,
    enabled_groups: Vec<String>,
) -> Result<(), String> {
    let config = state.get_config().map_err(|e: AppError| e.to_string())?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path not configured".to_string())?;

    sync_mod_links_for_game_path(&game_path, enabled_groups)?;
    launch_game_internal(&game_path, config.intro_skip_enabled)
}
