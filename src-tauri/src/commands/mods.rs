use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::game;
use crate::models::{
    InstalledModFile, InstalledModGroup, ModInfo, ModSource, ModStatus, ProgressEvent,
};
use crate::services::{
    downloader, hasher, installer, manifest, modio_api::ModioApiService,
    modpack as modpack_service, nexus_api, profiles, steam,
};
use crate::state::{app_data_root, app_temp_root, AppState};

fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn get_local_store_root() -> crate::models::Result<PathBuf> {
    app_data_root().map_err(|e| crate::models::AppError::Validation(e.to_string()))
}

fn get_staging_root() -> crate::models::Result<PathBuf> {
    let root = get_local_store_root()?.join("staged");
    fs::create_dir_all(&root)?;
    Ok(root)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddModioResult {
    pub mod_id: u64,
    pub name: String,
    pub archive_name: String,
    pub source_url: String,
}

fn parse_modio_input_to_slug_or_id(input: &str) -> Result<(Option<u64>, Option<String>), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("mod.io input cannot be empty".to_string());
    }

    if let Ok(mod_id) = trimmed.parse::<u64>() {
        return Ok((Some(mod_id), None));
    }

    // Accept full URLs like:
    // https://mod.io/g/readyornot/m/no-stress-for-swat#description
    // and plain slugs like:
    // no-stress-for-swat
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    let normalized = without_query.trim_end_matches('/');

    let slug = if let Some(idx) = normalized.find("/m/") {
        normalized[idx + 3..]
            .split('/')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        normalized
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    if slug.is_empty() {
        return Err("Could not extract mod.io stub from input".to_string());
    }

    Ok((None, Some(slug)))
}

fn sanitize_filename_for_download(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>();

    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "modio-download.zip".to_string()
    } else {
        trimmed.to_string()
    }
}

fn update_manifest_metadata(
    archive_name: &str,
    display_name: &str,
    source_url: &str,
) -> Result<(), String> {
    let manager = manifest::ManifestManager::new(&get_staging_root().map_err(|e| e.to_string())?);
    let mut manifest_data = manager
        .load_manifest(archive_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Archive manifest not found: {}", archive_name))?;

    manifest_data.display_name = Some(display_name.to_string());
    manifest_data.source_url = Some(source_url.to_string());

    manager
        .save_manifest(&manifest_data)
        .map_err(|e| e.to_string())
}

fn remove_mod_from_active_profile(
    state: &State<'_, AppState>,
    mod_name: &str,
) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let Some(active_profile_name) = config.active_profile else {
        return Ok(());
    };

    let Some(mut profile) =
        profiles::get_profile(&active_profile_name).map_err(|e| e.to_string())?
    else {
        return Ok(());
    };

    let before_len = profile.installed_mod_names.len();
    profile.installed_mod_names.retain(|name| name != mod_name);

    if profile.installed_mod_names.len() != before_len {
        profiles::save_profile(&profile).map_err(|e| e.to_string())?;
        // Refresh symlinks after removing mod from profile
        sync_active_profile_links(state)?;
    }

    Ok(())
}

fn add_mod_to_active_profile(state: &State<'_, AppState>, mod_name: &str) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let Some(active_profile_name) = config.active_profile else {
        return Ok(());
    };

    let Some(mut profile) =
        profiles::get_profile(&active_profile_name).map_err(|e| e.to_string())?
    else {
        return Ok(());
    };

    if profile
        .installed_mod_names
        .iter()
        .any(|name| name == mod_name)
    {
        return Ok(());
    }

    profile.installed_mod_names.push(mod_name.to_string());
    profiles::save_profile(&profile).map_err(|e| e.to_string())?;
    Ok(())
}

fn is_mod_used_by_any_profile(mod_name: &str) -> Result<bool, String> {
    let all_profiles = profiles::list_profiles().map_err(|e| e.to_string())?;
    Ok(all_profiles.iter().any(|profile| {
        profile
            .installed_mod_names
            .iter()
            .any(|name| name == mod_name)
    }))
}

fn sync_active_profile_links(state: &State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let Some(game_path) = config.game_path else {
        return Ok(());
    };

    let enabled_groups = if let Some(active_profile_name) = config.active_profile {
        if let Some(profile) =
            profiles::get_profile(&active_profile_name).map_err(|e| e.to_string())?
        {
            profile.installed_mod_names
        } else {
            config.enabled_collections
        }
    } else {
        config.enabled_collections
    };

    game::sync_mod_links_for_game_path(&game_path, enabled_groups)
}

fn remove_empty_parent_dirs(mut current_dir: PathBuf, stop_at: &Path) {
    while current_dir.starts_with(stop_at) && current_dir != stop_at {
        match fs::remove_dir(&current_dir) {
            Ok(_) => {
                if let Some(parent) = current_dir.parent() {
                    current_dir = parent.to_path_buf();
                } else {
                    break;
                }
            }
            Err(_) => {
                // Stop when directory is non-empty or cannot be removed.
                break;
            }
        }
    }
}

fn cleanup_empty_install_dirs(file_path: &Path, staging_root: &Path, live_mods_root: &Path) {
    let Some(parent) = file_path.parent() else {
        return;
    };

    let staged_mods_root = staging_root.join("mods");
    let staged_savegames_root = staging_root.join("savegames");
    let staged_backups_root = staging_root.join("backups");

    if parent.starts_with(&staged_mods_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), &staged_mods_root);
    } else if parent.starts_with(&staged_savegames_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), &staged_savegames_root);
    } else if parent.starts_with(&staged_backups_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), &staged_backups_root);
    } else if parent.starts_with(live_mods_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), live_mods_root);
    }
}

fn cleanup_mod_staging_directories(archive_name: &str, staging_root: &Path) {
    // Derive the install_key from the archive name (same logic as during installation)
    let install_key = PathBuf::from(archive_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>();

    // Try to remove the install_key directory from mods, savegames, and backups if empty
    let mod_install_dir = staging_root.join("mods").join(&install_key);
    let savegames_install_dir = staging_root.join("savegames").join(&install_key);
    let backups_install_dir = staging_root.join("backups").join(&install_key);

    let _ = fs::remove_dir(&mod_install_dir);
    let _ = fs::remove_dir(&savegames_install_dir);
    let _ = fs::remove_dir(&backups_install_dir);
}

#[tauri::command]
pub async fn get_mod_list(state: State<'_, AppState>) -> Result<Vec<ModInfo>, String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);

    if !mods_path.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    for entry in fs::read_dir(&mods_path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_pak = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pak"))
            .unwrap_or(false);
        if !is_pak {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        mods.push(ModInfo {
            name: file_name.clone(),
            source: ModSource::Manual,
            status: ModStatus::Installed,
            filename: file_name,
        });
    }

    mods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(mods)
}

#[tauri::command]
pub async fn install_mods(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled_collections: Option<Vec<String>>,
) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let modpack_url = config
        .modpack_url
        .ok_or_else(|| "Modpack URL is not configured".to_string())?;

    let staging_root = get_staging_root().map_err(|e| e.to_string())?;
    let mods_path = staging_root.join("mods");
    let savegames_path = staging_root.join("savegames");
    let backup_path = staging_root.join("backups");

    // Emit: fetching manifest
    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "fetch".to_string(),
            file: "ronmod.manifest".to_string(),
            percent: 5.0,
            message: "Fetching modpack manifest...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let manifest = modpack_service::fetch_manifest_if_exists(&state.client, &modpack_url)
        .await
        .map_err(|e| {
            let _ = app.emit("install_progress", &ProgressEvent::new_error(e.to_string()));
            e.to_string()
        })?;
    let manifest = manifest.ok_or_else(|| {
        let err_msg =
            "No modpack manifest found. Ensure ronmod.manifest is available at the modpack URL"
                .to_string();
        let _ = app.emit(
            "install_progress",
            &ProgressEvent::new_error(err_msg.clone()),
        );
        err_msg
    })?;

    // Filter manifest files based on enabled collections if provided
    let mut filtered_manifest = manifest.clone();
    if let Some(ref enabled_cols) = enabled_collections {
        // Fetch modpack metadata to understand collection structure
        let modpack = modpack_service::fetch_modpack(&state.client, &modpack_url)
            .await
            .ok();

        if let Some(modpack) = modpack {
            filtered_manifest.files.retain(|file| {
                // Check if file is in an enabled collection
                for (collection_name, collection) in &modpack.collections {
                    if enabled_cols.contains(collection_name)
                        && collection.mods.iter().any(|m| file.path.contains(m))
                    {
                        return true;
                    }
                }
                // If file path contains collection folder name or is in _manual, include it
                file.path.contains("_manual/")
                    || enabled_cols
                        .iter()
                        .any(|c| file.path.contains(&format!("{}/", c)))
            });
        }
    }

    let download_root = app_temp_root().join("modpack_downloads");
    fs::create_dir_all(&download_root).map_err(|error| {
        let _ = app.emit(
            "install_progress",
            &ProgressEvent::new_error(error.to_string()),
        );
        error.to_string()
    })?;

    // Emit: downloading files
    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "download_start".to_string(),
            file: format!("{} files", filtered_manifest.files.len()),
            percent: 10.0,
            message: format!("Downloading {} files...", filtered_manifest.files.len()),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let downloaded_files = modpack_service::download_manifest_files(
        &state.client,
        &modpack_url,
        &download_root,
        &filtered_manifest,
    )
    .await
    .map_err(|e| {
        let _ = app.emit("install_progress", &ProgressEvent::new_error(e.to_string()));
        e.to_string()
    })?;

    // Emit: installing files
    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "install_start".to_string(),
            file: String::new(),
            percent: 50.0,
            message: format!("Installing {} mods...", downloaded_files.len()),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let install_context = installer::InstallContext {
        game_path: game_path.clone(),
        mods_path: mods_path.clone(),
        savegames_path,
        backup_path,
    };

    for (index, file) in downloaded_files.iter().enumerate() {
        let file_name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Emit: processing current file
        let progress_pct = 50.0 + (index as f32 / downloaded_files.len() as f32) * 49.0;
        let _ = app.emit(
            "install_progress",
            &ProgressEvent::new_install(&file_name, progress_pct),
        );

        install_downloaded_file(file, &install_context, &app).map_err(|e| {
            let _ = app.emit(
                "install_progress",
                &ProgressEvent::new_error(format!("Failed to install {}: {}", file_name, e)),
            );
            e.to_string()
        })?;
    }

    // Emit: complete
    let _ = app.emit("install_progress", &ProgressEvent::new_complete());

    Ok(())
}

#[tauri::command]
pub async fn uninstall_mods(_app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root().map_err(|e| e.to_string())?;

    // Uninstall all mods from the active profile only; delete files only if unused by all profiles.
    let Some(active_profile_name) = config.active_profile else {
        return Ok(());
    };

    let Some(mut active_profile) =
        profiles::get_profile(&active_profile_name).map_err(|e| e.to_string())?
    else {
        return Ok(());
    };

    let mods_to_remove = active_profile.installed_mod_names.clone();
    if mods_to_remove.is_empty() {
        return Ok(());
    }

    active_profile.installed_mod_names.clear();
    profiles::save_profile(&active_profile).map_err(|e| e.to_string())?;

    let manager = manifest::ManifestManager::new(&staging_root);
    for mod_name in mods_to_remove {
        if is_mod_used_by_any_profile(&mod_name)? {
            continue;
        }

        if let Ok(Some(manifest_data)) = manager.load_manifest(&mod_name) {
            for file_path in &manifest_data.installed_files {
                if file_path.exists() && fs::remove_file(file_path).is_ok() {
                    cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
                }
            }
            cleanup_mod_staging_directories(&mod_name, &staging_root);
            let _ = manager.delete_manifest(&mod_name);
            continue;
        }

        let loose_mod_path = mods_path.join(&mod_name);
        if loose_mod_path.exists() && fs::remove_file(&loose_mod_path).is_ok() {
            cleanup_empty_install_dirs(&loose_mod_path, &staging_root, &mods_path);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn add_modio_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
) -> Result<AddModioResult, String> {
    let config = state.get_config().map_err(String::from)?;
    let token = config.oauth_token.ok_or_else(|| {
        "mod.io token is not configured. Set token in Settings first.".to_string()
    })?;

    let (explicit_id, slug) = parse_modio_input_to_slug_or_id(&input)?;
    let modio_service = ModioApiService::new(state.client.clone());

    let mod_id = match explicit_id {
        Some(id) => id,
        None => {
            let slug_value = slug.ok_or_else(|| "Could not determine mod.io stub".to_string())?;
            modio_service
                .resolve_slug_to_mod_id(&token, &slug_value)
                .await
                .map_err(String::from)?
        }
    };

    let mod_details = modio_service
        .get_mod_download_info(&token, mod_id)
        .await
        .map_err(String::from)?;

    let download_root = app_temp_root().join("modio_downloads");
    fs::create_dir_all(&download_root).map_err(|e| e.to_string())?;

    let archive_name = sanitize_filename_for_download(&mod_details.filename);
    let archive_path = download_root.join(&archive_name);

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "download".to_string(),
            file: archive_name.clone(),
            percent: 10.0,
            message: format!("Downloading {}...", mod_details.name),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let app_for_progress = app.clone();
    let archive_for_progress = archive_name.clone();
    let mod_name_for_progress = mod_details.name.clone();
    let download_started = Instant::now();

    downloader::download_file_with_progress(
        &state.client,
        &mod_details.download_url,
        &archive_path,
        Some(Box::new(move |downloaded, total| {
            let elapsed = download_started.elapsed().as_secs_f64().max(0.001);
            let mib_per_sec = (downloaded as f64 / elapsed) / (1024.0 * 1024.0);

            let percent = if total > 0 {
                5.0 + ((downloaded as f32 / total as f32) * 45.0)
            } else {
                25.0
            };

            let _ = app_for_progress.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "download".to_string(),
                    file: archive_for_progress.clone(),
                    percent: percent.min(50.0),
                    message: format!(
                        "Downloading {}... {:.1} MiB/s",
                        mod_name_for_progress, mib_per_sec
                    ),
                    total_bytes: if total > 0 { Some(total) } else { None },
                    processed_bytes: Some(downloaded),
                },
            );
        })),
    )
    .await
    .map_err(String::from)?;

    install_local_mod(app, state, archive_path.to_string_lossy().to_string()).await?;

    update_manifest_metadata(&archive_name, &mod_details.name, &mod_details.profile_url)?;

    Ok(AddModioResult {
        mod_id: mod_details.id,
        name: mod_details.name,
        archive_name,
        source_url: mod_details.profile_url,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusModInfoResult {
    pub mod_id: u64,
    pub name: String,
    pub summary: Option<String>,
    pub mod_url: String,
}

#[tauri::command]
pub async fn fetch_nexus_mod_info(
    state: State<'_, AppState>,
    input: String,
) -> Result<NexusModInfoResult, String> {
    let config = state.get_config().map_err(String::from)?;
    let api_key = config.nexus_api_key.ok_or_else(|| {
        "Nexus Mods API key is not configured. Set it in Settings first.".to_string()
    })?;

    let mod_id = nexus_api::parse_nexus_url_to_mod_id(&input).map_err(String::from)?;

    let nexus_service = nexus_api::NexusApiService::new(state.client.clone());
    let mod_info = nexus_service
        .get_mod_info(&api_key, mod_id)
        .await
        .map_err(String::from)?;

    Ok(NexusModInfoResult {
        mod_id: mod_info.mod_id,
        name: mod_info.name,
        summary: mod_info.summary,
        mod_url: format!("https://www.nexusmods.com/readyornot/mods/{}", mod_id),
    })
}

#[tauri::command]
pub async fn uninstall_archive(
    state: State<'_, AppState>,
    archive_name: String,
) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root().map_err(|e| e.to_string())?;
    let manager = manifest::ManifestManager::new(&staging_root);

    // Remove from active profile first.
    remove_mod_from_active_profile(&state, &archive_name)?;

    // Keep files if any profile still references this mod.
    if is_mod_used_by_any_profile(&archive_name)? {
        return Ok(());
    }

    let manifest_data = manager
        .load_manifest(&archive_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Archive manifest not found: {}", archive_name))?;

    for file_path in &manifest_data.installed_files {
        if file_path.exists() && fs::remove_file(file_path).is_ok() {
            cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
        }
    }

    cleanup_mod_staging_directories(&archive_name, &staging_root);

    manager
        .delete_manifest(&archive_name)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_mod_display_name(
    state: State<'_, AppState>,
    archive_name: String,
    display_name: String,
) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let _game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let staging_root = get_staging_root().map_err(|e| e.to_string())?;
    let manager = manifest::ManifestManager::new(&staging_root);

    let mut manifest_data = manager
        .load_manifest(&archive_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Archive manifest not found: {}", archive_name))?;

    manifest_data.display_name = if display_name.trim().is_empty() {
        None
    } else {
        Some(display_name.trim().to_string())
    };

    manager
        .save_manifest(&manifest_data)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn update_mod_source_url(
    state: State<'_, AppState>,
    archive_name: String,
    source_url: String,
) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let _game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let staging_root = get_staging_root().map_err(|e| e.to_string())?;
    let manager = manifest::ManifestManager::new(&staging_root);

    let mut manifest_data = manager
        .load_manifest(&archive_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Archive manifest not found: {}", archive_name))?;

    manifest_data.source_url = if source_url.trim().is_empty() {
        None
    } else {
        Some(source_url.trim().to_string())
    };

    manager
        .save_manifest(&manifest_data)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_installed_mod_groups(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledModGroup>, String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root().map_err(|e| e.to_string())?;

    if !mods_path.exists() && !staging_root.exists() {
        return Ok(Vec::new());
    }

    let manager = manifest::ManifestManager::new(&staging_root);
    let manifests = manager.list_all_manifests().unwrap_or_default();
    let mut tracked_pak_names: HashSet<String> = HashSet::new();
    let mut groups: Vec<InstalledModGroup> = Vec::new();

    for manifest_data in manifests.values() {
        let mut files: Vec<InstalledModFile> = manifest_data
            .installed_files
            .iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| path.to_string_lossy().to_string());

                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("pak"))
                    .unwrap_or(false)
                {
                    tracked_pak_names.insert(name.clone());
                }

                InstalledModFile {
                    name,
                    path: path.to_string_lossy().to_string(),
                    exists: path.exists(),
                }
            })
            .collect();

        files.sort_by(|a, b| a.name.cmp(&b.name));

        groups.push(InstalledModGroup {
            name: manifest_data.source_archive.clone(),
            display_name: manifest_data.display_name.clone(),
            source_url: manifest_data.source_url.clone(),
            managed_by_manifest: true,
            installed_at: Some(manifest_data.installed_at),
            files,
        });
    }

    for entry in fs::read_dir(&mods_path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_pak = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pak"))
            .unwrap_or(false);
        if !is_pak {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();

        if tracked_pak_names.contains(&file_name) {
            continue;
        }

        groups.push(InstalledModGroup {
            name: file_name.clone(),
            display_name: None,
            source_url: None,
            managed_by_manifest: false,
            installed_at: None,
            files: vec![InstalledModFile {
                name: file_name,
                path: path.to_string_lossy().to_string(),
                exists: path.exists(),
            }],
        });
    }

    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(groups)
}

#[tauri::command]
pub async fn uninstall_mod(state: State<'_, AppState>, filename: String) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root().map_err(|e| e.to_string())?;

    // Check if this file is part of an archive installation
    let manager = manifest::ManifestManager::new(&staging_root);
    if let Ok(Some((archive_name, manifest))) = manager.get_manifest_for_pak(&filename) {
        println!(
            "Found manifest for {}: {} files to uninstall",
            archive_name,
            manifest.installed_files.len()
        );

        // Remove from active profile first.
        remove_mod_from_active_profile(&state, &archive_name)?;

        // Keep files if any profile still references this mod.
        if is_mod_used_by_any_profile(&archive_name)? {
            return Ok(());
        }

        // Uninstall all files from the archive
        for file_path in &manifest.installed_files {
            if file_path.exists() {
                println!("Removing: {:?}", file_path);
                if let Err(e) = fs::remove_file(file_path) {
                    eprintln!("Failed to remove {:?}: {}", file_path, e);
                } else {
                    cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
                }
            }
        }

        cleanup_mod_staging_directories(&archive_name, &staging_root);

        // Delete the manifest
        if let Err(e) = manager.delete_manifest(&archive_name) {
            eprintln!("Failed to delete manifest: {}", e);
        }

        return Ok(());
    }

    // If no manifest found, treat this as a loose file mod.
    remove_mod_from_active_profile(&state, &filename)?;
    if is_mod_used_by_any_profile(&filename)? {
        return Ok(());
    }

    // If no manifest found, just uninstall the single file
    let mod_path = mods_path.join(&filename);
    if !mod_path.exists() {
        return Err(format!("Mod file not found: {}", filename));
    }

    fs::remove_file(&mod_path).map_err(|error| error.to_string())?;
    cleanup_empty_install_dirs(&mod_path, &staging_root, &mods_path);

    Ok(())
}

#[tauri::command]
pub async fn install_local_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    file_path: String,
) -> Result<(), String> {
    println!("install_local_mod called with path: {}", file_path);

    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;

    println!("Game path: {:?}", game_path);

    let staging_root = get_staging_root().map_err(|e| e.to_string())?;
    let paks_path = staging_root.join("mods");
    let savegames_path = staging_root.join("savegames");
    let backup_path = staging_root.join("backups");

    println!("Staged Paks path: {:?}", paks_path);
    println!("Staged SaveGames path: {:?}", savegames_path);

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "install".to_string(),
            file: file_path.clone(),
            percent: 0.0,
            message: "Starting installation...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let path = PathBuf::from(&file_path);
    let context = installer::InstallContext {
        game_path: game_path.clone(),
        mods_path: paks_path,
        savegames_path,
        backup_path,
    };

    println!("Calling install_downloaded_file...");

    // Emit preparation progress before analysis/hashing begins.
    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "install".to_string(),
            file: file_path.clone(),
            percent: 4.0,
            message: "Preparing file for install...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    match install_downloaded_file(&path, &context, &app) {
        Ok(_) => {
            if let Some(installed_mod_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Err(error) = add_mod_to_active_profile(&state, installed_mod_name) {
                    eprintln!(
                        "Failed to add '{}' to active profile after install: {}",
                        installed_mod_name, error
                    );
                }
            }

            sync_active_profile_links(&state)?;

            println!("Installation successful!");
            let _ = app.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "complete".to_string(),
                    file: file_path,
                    percent: 100.0,
                    message: "Installation complete!".to_string(),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to install mod: {}", e);
            println!("Installation failed: {}", error_msg);
            let _ = app.emit(
                "install_progress",
                &ProgressEvent::new_error(error_msg.clone()),
            );
            Err(error_msg)
        }
    }
}

fn save_install_manifest(
    archive_path: &Path,
    report: &installer::InstallReport,
    _context: &installer::InstallContext,
    content_hash: Option<String>,
) -> crate::models::Result<()> {
    // Only save manifest if files were actually installed
    if report.installed_files.is_empty() {
        return Ok(());
    }

    let archive_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let manager = manifest::ManifestManager::new(&get_staging_root()?);
    let manifest = manifest::InstallManifest {
        source_archive: archive_name,
        display_name: None,
        source_url: None,
        installed_files: report.installed_files.clone(),
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        content_hash,
    };

    manager.save_manifest(&manifest)?;
    println!("Saved manifest with {} files", report.installed_files.len());
    Ok(())
}

fn install_downloaded_file(
    path: &PathBuf,
    context: &installer::InstallContext,
    app: &AppHandle,
) -> crate::models::Result<()> {
    println!("install_downloaded_file: Processing file: {:?}", path);

    let hash_start = Instant::now();
    let mut hash_last_emit = Instant::now() - Duration::from_millis(500);

    // Compute content hash for deduplication with streaming progress.
    let content_hash = hasher::md5_file_with_progress(path, |processed_bytes, total_bytes| {
        let now = Instant::now();
        let done = total_bytes > 0 && processed_bytes >= total_bytes;
        let should_emit = now.duration_since(hash_last_emit) >= Duration::from_millis(120) || done;
        if !should_emit {
            return;
        }
        hash_last_emit = now;

        let elapsed = hash_start.elapsed().as_secs_f64().max(0.001);
        let mib_per_sec = bytes_to_mib(processed_bytes) / elapsed;
        let ratio = if total_bytes > 0 {
            (processed_bytes as f64 / total_bytes as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mapped_percent = (8.0 + (ratio * 30.0)) as f32;

        let message = if total_bytes > 0 {
            format!(
                "Hashing archive... {:.1}/{:.1} MiB ({:.1} MiB/s)",
                bytes_to_mib(processed_bytes),
                bytes_to_mib(total_bytes),
                mib_per_sec,
            )
        } else {
            format!("Hashing archive... {:.1} MiB/s", mib_per_sec)
        };

        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "hash".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: mapped_percent,
                message,
                total_bytes: Some(total_bytes),
                processed_bytes: Some(processed_bytes),
            },
        );
    })
    .map_err(|e| crate::models::AppError::Validation(format!("Failed to hash file: {}", e)))?;
    println!("Computed content hash: {}", content_hash);

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "hash".to_string(),
            file: path.to_string_lossy().to_string(),
            percent: 40.0,
            message: "Checking cache for matching content...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    // Check if this hash already exists.
    // If it does, reuse the existing staged files directly instead of creating
    // another per-profile duplicate folder.
    let manager = manifest::ManifestManager::new(&get_staging_root()?);
    if manager.find_by_content_hash(&content_hash)?.is_some() {
        println!("Found existing installation with same content hash; reusing staged files");

        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "dedupe".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 98.0,
                message: "Duplicate detected. Reusing existing staged files...".to_string(),
                total_bytes: None,
                processed_bytes: None,
            },
        );

        return Ok(());
    }

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "extract".to_string(),
            file: path.to_string_lossy().to_string(),
            percent: 48.0,
            message: "No duplicate found. Extracting archive...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let install_key = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>();

    // Every dropped file gets its own staging folder so similarly named files do not collide.
    let staged_context = installer::InstallContext {
        game_path: context
            .backup_path
            .join("staged_overrides")
            .join(&install_key),
        mods_path: context.mods_path.join(&install_key),
        savegames_path: context.savegames_path.join(&install_key),
        backup_path: context.backup_path.join(&install_key),
    };

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();

    println!("File extension: {}", extension);

    if extension.eq_ignore_ascii_case("zip") {
        println!("Detected ZIP archive, calling install_archive");
        let start = Instant::now();
        let mut last_emit = Instant::now() - Duration::from_millis(500);
        let report = installer::install_archive_with_progress(path, &staged_context, |progress| {
            let now = Instant::now();
            let should_emit = now.duration_since(last_emit) >= Duration::from_millis(120)
                || progress.processed_bytes >= progress.total_bytes;
            if !should_emit {
                return;
            }
            last_emit = now;

            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let bytes_per_sec = progress.processed_bytes as f64 / elapsed;
            let mib_per_sec = bytes_per_sec / (1024.0 * 1024.0);
            let mapped_percent = 55.0 + (progress.percent * 0.40);

            let _ = app.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "extract".to_string(),
                    file: progress.file,
                    percent: mapped_percent.min(95.0),
                    message: format!("Extracting files... {:.1} MiB/s", mib_per_sec),
                    total_bytes: Some(progress.total_bytes),
                    processed_bytes: Some(progress.processed_bytes),
                },
            );
        })?;

        // Save manifest for tracking installed files with content hash
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(());
    }

    if extension.eq_ignore_ascii_case("rar") {
        println!("Detected RAR archive, calling install_rar_archive");
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 50.0,
                message: "Installing RAR archive...".to_string(),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let report = installer::install_rar_archive(path, &staged_context)?;

        // Save manifest for tracking installed files with content hash
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(());
    }

    if extension.eq_ignore_ascii_case("7z") {
        return Err(crate::models::AppError::Validation(
            "7Z archives are not directly supported. Please extract the archive first, then drag the .pak or .zip file.".to_string()
        ));
    }

    if extension.eq_ignore_ascii_case("pak") {
        println!("Detected PAK file, installing directly");
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 75.0,
                message: "Installing PAK file...".to_string(),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let file_name = path.file_name().ok_or_else(|| {
            crate::models::AppError::Validation("invalid pak file path".to_string())
        })?;
        fs::create_dir_all(&staged_context.mods_path)?;
        let destination = staged_context.mods_path.join(file_name);
        fs::copy(path, &destination)?;
        let report = installer::InstallReport {
            installed: 1,
            skipped: 0,
            overrides_backed_up: 0,
            installed_files: vec![destination],
        };
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(());
    }

    if extension.eq_ignore_ascii_case("sav") {
        println!("Detected SAV file, installing to SaveGames");
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 75.0,
                message: "Installing save file...".to_string(),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let file_name = path.file_name().ok_or_else(|| {
            crate::models::AppError::Validation("invalid sav file path".to_string())
        })?;
        fs::create_dir_all(&staged_context.savegames_path)?;
        let destination = staged_context.savegames_path.join(file_name);
        fs::copy(path, &destination)?;
        let report = installer::InstallReport {
            installed: 1,
            skipped: 0,
            overrides_backed_up: 0,
            installed_files: vec![destination],
        };
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
    }

    Ok(())
}
