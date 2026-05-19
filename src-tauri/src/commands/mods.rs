#[derive(Debug, Serialize)]
pub struct ModioRemoteMd5Result {
    pub remote_md5: Option<String>,
    pub archive_name: String,
}

#[tauri::command]
pub async fn get_modio_remote_info(
    state: tauri::State<'_, crate::state::AppState>,
    input: String,
) -> crate::models::Result<ModioRemoteMd5Result> {
    let config = state.get_config()?;
    let token = config.oauth_token.ok_or_else(|| {
        crate::models::AppError::Validation(
            "mod.io token is not configured. Set token in Settings first.".to_string(),
        )
    })?;
    let modio_service = crate::services::modio_api::ModioApiService::new(
        state.client.clone(),
        config.modio_game_id,
    );
    let (explicit_id, slug) = parse_modio_input_to_slug_or_id(&input)?;
    let mod_id = match explicit_id {
        Some(id) => id,
        None => {
            let slug_value = slug.ok_or_else(|| {
                crate::models::AppError::Validation("Could not determine mod.io stub".to_string())
            })?;
            modio_service
                .resolve_slug_to_mod_id(&token, &slug_value)
                .await?
        }
    };
    let mod_details = modio_service.get_mod_download_info(&token, mod_id).await?;
    Ok(ModioRemoteMd5Result {
        remote_md5: mod_details.remote_md5,
        archive_name: mod_details.filename,
    })
}
#[tauri::command]
pub fn read_manifest_for_archive(archive_name: String) -> Result<Option<serde_json::Value>> {
    let manager = manifest::ManifestManager::new(&get_staging_root()?);
    let manifest_opt = manager.load_manifest(&archive_name)?;
    if let Some(manifest) = manifest_opt {
        let json = serde_json::to_value(manifest)
            .map_err(|e| AppError::Validation(format!("Failed to serialize manifest: {}", e)))?;
        Ok(Some(json))
    } else {
        Ok(None)
    }
}
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::game;
use crate::models::{
    AppError, InstalledModFile, InstalledModGroup, ModInfo, ModSource, ModStatus, ProgressEvent,
    Result,
};
use crate::services::{
    downloader, hasher, installer, manifest, modio_api::ModioApiService,
    modpack as modpack_service, nexus_api, profiles, steam,
};
use crate::state::{app_data_root, app_temp_root, AppState};

fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn get_local_store_root() -> Result<PathBuf> {
    app_data_root()
}

fn get_staging_root() -> Result<PathBuf> {
    let root = get_local_store_root()?.join("staged");
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn get_archives_root() -> Result<PathBuf> {
    let root = get_staging_root()?.join("archives");
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

fn parse_modio_input_to_slug_or_id(input: &str) -> Result<(Option<u64>, Option<String>)> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "mod.io input cannot be empty".to_string(),
        ));
    }

    if let Ok(mod_id) = trimmed.parse::<u64>() {
        return Ok((Some(mod_id), None));
    }

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
        return Err(AppError::Validation(
            "Could not extract mod.io stub from input".to_string(),
        ));
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
    content_hash_fallback: Option<&str>,
) -> Result<()> {
    let manager = manifest::ManifestManager::new(&get_staging_root()?);
    let manifest_opt = manager.load_manifest(archive_name)?;
    let mut manifest_data = if let Some(m) = manifest_opt {
        m
    } else if let Some(hash) = content_hash_fallback {
        manager.find_by_content_hash(hash)?.ok_or_else(|| {
            AppError::Validation(format!("Archive manifest not found: {}", archive_name))
        })?
    } else {
        return Err(AppError::Validation(format!(
            "Archive manifest not found: {}",
            archive_name
        )));
    };

    manifest_data.display_name = Some(display_name.to_string());
    manifest_data.source_url = Some(source_url.to_string());

    manager.save_manifest(&manifest_data)?;
    Ok(())
}

fn remove_mod_from_active_profile(state: &State<'_, AppState>, mod_name: &str) -> Result<()> {
    let config = state.get_config()?;
    let Some(active_profile_name) = config.active_profile else {
        return Ok(());
    };

    let Some(mut profile) = profiles::get_profile(&active_profile_name)? else {
        return Ok(());
    };

    let before_len = profile.installed_mod_names.len();
    profile.installed_mod_names.retain(|name| name != mod_name);

    if profile.installed_mod_names.len() != before_len {
        profiles::save_profile(&profile)?;
        sync_active_profile_links(state)?;
    }

    Ok(())
}

fn add_mod_to_active_profile(state: &State<'_, AppState>, mod_name: &str) -> Result<()> {
    let config = state.get_config()?;
    let Some(active_profile_name) = config.active_profile else {
        return Ok(());
    };

    let Some(mut profile) = profiles::get_profile(&active_profile_name)? else {
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
    profiles::save_profile(&profile)?;
    Ok(())
}

fn is_mod_used_by_any_profile(mod_name: &str) -> Result<bool> {
    let all_profiles = profiles::list_profiles()?;
    Ok(all_profiles.iter().any(|profile| {
        profile
            .installed_mod_names
            .iter()
            .any(|name| name == mod_name)
    }))
}

fn sync_active_profile_links(state: &State<'_, AppState>) -> Result<()> {
    let config = state.get_config()?;
    let Some(game_path) = config.game_path.as_ref() else {
        return Ok(());
    };

    let enabled_groups = if let Some(active_profile_name) = config.active_profile {
        if let Some(profile) = profiles::get_profile(&active_profile_name)? {
            profile.installed_mod_names
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    game::sync_mod_links_for_game_path(game_path, enabled_groups).map_err(AppError::Validation)
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
            Err(_) => break,
        }
    }
}

fn cleanup_empty_install_dirs(file_path: &Path, staging_root: &Path, live_mods_root: &Path) {
    let parent = match file_path.parent() {
        Some(p) => p,
        None => return,
    };

    let staged_mods_root = staging_root.join("mods");
    let staged_savegames_root = staging_root.join("savegames");
    let staged_backups_root = staging_root.join("backups");

    let parent_path: &Path = parent;

    if parent_path.starts_with(&staged_mods_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), &staged_mods_root);
    } else if parent_path.starts_with(&staged_savegames_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), &staged_savegames_root);
    } else if parent_path.starts_with(&staged_backups_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), &staged_backups_root);
    } else if parent_path.starts_with(live_mods_root) {
        remove_empty_parent_dirs(parent.to_path_buf(), live_mods_root);
    }
}

fn cleanup_mod_staging_directories(archive_name: &str, staging_root: &Path) {
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

    let _ = fs::remove_dir(staging_root.join("mods").join(&install_key));
    let _ = fs::remove_dir(staging_root.join("savegames").join(&install_key));
    let _ = fs::remove_dir(staging_root.join("backups").join(&install_key));
}

#[tauri::command]
pub async fn get_mod_list(state: State<'_, AppState>) -> Result<Vec<ModInfo>> {
    let config = state.get_config()?;
    let game_path = config
        .game_path
        .ok_or_else(|| AppError::Validation("Game path is not configured".to_string()))?;
    let mods_path = steam::get_mods_path(&game_path);

    if !mods_path.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    for entry in fs::read_dir(&mods_path)? {
        let entry = entry?;
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
) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config
        .game_path
        .ok_or_else(|| AppError::Validation("Game path is not configured".to_string()))?;
    let modpack_url = config
        .modpack_url
        .ok_or_else(|| AppError::Validation("Modpack URL is not configured".to_string()))?;

    let staging_root = get_staging_root()?;
    let mods_path = staging_root.join("mods");
    let savegames_path = staging_root.join("savegames");
    let backup_path = staging_root.join("backups");

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

    let manifest = modpack_service::fetch_manifest_if_exists(&state.client, &modpack_url).await?;
    let manifest =
        manifest.ok_or_else(|| AppError::Validation("No modpack manifest found.".to_string()))?;

    let mut filtered_manifest = manifest.clone();
    if let Some(ref enabled_cols) = enabled_collections {
        let modpack = modpack_service::fetch_modpack(&state.client, &modpack_url)
            .await
            .ok();
        if let Some(modpack) = modpack {
            filtered_manifest.files.retain(|file| {
                for (collection_name, collection) in &modpack.collections {
                    if enabled_cols.contains(collection_name)
                        && collection.mods.iter().any(|m| file.path.contains(m))
                    {
                        return true;
                    }
                }
                file.path.contains("_manual/")
                    || enabled_cols
                        .iter()
                        .any(|c| file.path.contains(&format!("{}/", c)))
            });
        }
    }

    let download_root = app_temp_root().join("modpack_downloads");
    fs::create_dir_all(&download_root)?;

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
    .await?;

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
        let progress_pct = 50.0 + (index as f32 / downloaded_files.len() as f32) * 49.0;
        let _ = app.emit(
            "install_progress",
            &ProgressEvent::new_install(&file_name, progress_pct),
        );
        install_downloaded_file(file, &install_context, &app)?;
    }

    let _ = app.emit("install_progress", &ProgressEvent::new_complete());
    Ok(())
}

#[tauri::command]
pub async fn uninstall_mods(_app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config
        .game_path
        .ok_or_else(|| AppError::Validation("Game path is not configured".to_string()))?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root()?;
    let manager = manifest::ManifestManager::new(&staging_root);

    // List ALL manifests to find every installed mod
    let all_manifests = manager.list_all_manifests().unwrap_or_default();

    // 1. Clear all installed_mod_names from all profiles
    let all_profiles = profiles::list_profiles()?;
    for mut profile in all_profiles {
        if !profile.installed_mod_names.is_empty() {
            profile.installed_mod_names.clear();
            profiles::save_profile(&profile)?;
        }
    }

    // 2. Delete files and manifests
    for manifest_data in all_manifests.into_values() {
        let mod_name = manifest_data.source_archive.clone();

        for file_path in &manifest_data.installed_files {
            if file_path.exists() && fs::remove_file(file_path).is_ok() {
                cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
            }
        }
        cleanup_mod_staging_directories(&mod_name, &staging_root);
        let _ = manager.delete_manifest(&mod_name);
    }

    // 3. Clear archives
    let archives_root = get_archives_root()?;
    if archives_root.exists() {
        let _ = fs::remove_dir_all(&archives_root);
        let _ = fs::create_dir_all(&archives_root);
    }

    // 4. Sync links (should result in empty mods folder)
    game::sync_mod_links_for_game_path(&game_path, Vec::new()).map_err(AppError::Validation)?;

    Ok(())
}

#[tauri::command]
pub async fn add_modio_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
) -> Result<AddModioResult> {
    let config = state.get_config()?;
    let token = config.oauth_token.ok_or_else(|| {
        AppError::Validation(
            "mod.io token is not configured. Set token in Settings first.".to_string(),
        )
    })?;

    let (explicit_id, slug) = parse_modio_input_to_slug_or_id(&input)?;
    let modio_service = ModioApiService::new(state.client.clone(), config.modio_game_id);

    let mod_id = match explicit_id {
        Some(id) => id,
        None => {
            let slug_value = slug.ok_or_else(|| {
                AppError::Validation("Could not determine mod.io stub".to_string())
            })?;
            modio_service
                .resolve_slug_to_mod_id(&token, &slug_value)
                .await?
        }
    };

    let mod_details = modio_service.get_mod_download_info(&token, mod_id).await?;

    let download_root = app_temp_root().join("modio_downloads");
    fs::create_dir_all(&download_root)?;

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
    .await?;

    let archive_content_hash = hasher::md5_file(&archive_path).ok();
    install_local_mod(
        app,
        state.clone(),
        archive_path.to_string_lossy().to_string(),
    )
    .await?;

    let _ = update_manifest_metadata(
        &archive_name,
        &mod_details.name,
        &mod_details.profile_url,
        archive_content_hash.as_deref(),
    );

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshModMetadataResult {
    pub checked: usize,
    pub refreshed: usize,
    pub skipped: usize,
    pub failed: usize,
}

async fn resolve_display_name_from_source_url(
    source_url: &str,
    api_key: Option<&str>,
    oauth_token: Option<&str>,
    nexus_service: &nexus_api::NexusApiService,
    modio_service: &ModioApiService,
) -> Result<Option<String>> {
    let trimmed = source_url.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if trimmed.contains("nexusmods.com") {
        let Some(key) = api_key else {
            return Ok(None);
        };
        let mod_id = nexus_api::parse_nexus_url_to_mod_id(trimmed)?;
        let mod_info = nexus_service.get_mod_info(key, mod_id).await?;
        return Ok(Some(mod_info.name));
    }

    if trimmed.contains("mod.io") {
        let Some(token) = oauth_token else {
            return Ok(None);
        };
        let (explicit_id, slug) = parse_modio_input_to_slug_or_id(trimmed)?;
        let mod_id = match explicit_id {
            Some(id) => id,
            None => {
                let slug_value = slug.ok_or_else(|| {
                    AppError::Validation("Could not determine mod.io stub".to_string())
                })?;
                modio_service
                    .resolve_slug_to_mod_id(token, &slug_value)
                    .await?
            }
        };
        let mod_details = modio_service.get_mod_download_info(token, mod_id).await?;
        return Ok(Some(mod_details.name));
    }

    Ok(None)
}

#[tauri::command]
pub async fn refresh_mod_metadata(state: State<'_, AppState>) -> Result<RefreshModMetadataResult> {
    let config = state.get_config()?;
    let api_key = config
        .nexus_api_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let oauth_token = config
        .oauth_token
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let staging_root = get_staging_root()?;
    let manager = manifest::ManifestManager::new(&staging_root);
    let manifests = manager.list_all_manifests()?;

    let nexus_service = nexus_api::NexusApiService::new(state.client.clone());
    let modio_service = ModioApiService::new(state.client.clone(), config.modio_game_id);

    let mut result = RefreshModMetadataResult {
        checked: 0,
        refreshed: 0,
        skipped: 0,
        failed: 0,
    };

    for mut manifest_data in manifests.into_values() {
        result.checked += 1;
        let Some(source_url) = manifest_data.source_url.as_ref() else {
            result.skipped += 1;
            continue;
        };
        let source_url = source_url.trim().to_string();
        if source_url.is_empty() {
            result.skipped += 1;
            continue;
        }

        match resolve_display_name_from_source_url(
            &source_url,
            api_key,
            oauth_token,
            &nexus_service,
            &modio_service,
        )
        .await
        {
            Ok(Some(name)) => {
                manifest_data.display_name = Some(name);
                if manager.save_manifest(&manifest_data).is_err() {
                    result.failed += 1;
                } else {
                    result.refreshed += 1;
                }
            }
            Ok(None) => result.skipped += 1,
            Err(_) => result.failed += 1,
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn fetch_nexus_mod_info(
    state: State<'_, AppState>,
    input: String,
) -> Result<NexusModInfoResult> {
    let config = state.get_config()?;
    let api_key = config
        .nexus_api_key
        .ok_or_else(|| AppError::Validation("Nexus Mods API key is not configured.".to_string()))?;
    let mod_id = nexus_api::parse_nexus_url_to_mod_id(&input)?;
    let nexus_service = nexus_api::NexusApiService::new(state.client.clone());
    let mod_info = nexus_service.get_mod_info(&api_key, mod_id).await?;
    Ok(NexusModInfoResult {
        mod_id: mod_info.mod_id,
        name: mod_info.name,
        summary: mod_info.summary,
        mod_url: format!("https://www.nexusmods.com/readyornot/mods/{}", mod_id),
    })
}

#[tauri::command]
pub async fn uninstall_archive(state: State<'_, AppState>, archive_name: String) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config
        .game_path
        .ok_or_else(|| AppError::Validation("Game path is not configured".to_string()))?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root()?;
    let manager = manifest::ManifestManager::new(&staging_root);

    remove_mod_from_active_profile(&state, &archive_name)?;
    if is_mod_used_by_any_profile(&archive_name)? {
        return Ok(());
    }

    let manifest_data = manager.load_manifest(&archive_name)?.ok_or_else(|| {
        AppError::Validation(format!("Archive manifest not found: {}", archive_name))
    })?;
    for file_path in &manifest_data.installed_files {
        if file_path.exists() && fs::remove_file(file_path).is_ok() {
            cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
        }
    }
    cleanup_mod_staging_directories(&archive_name, &staging_root);
    manager.delete_manifest(&archive_name)?;

    // Delete the archive if it exists
    let archives_root = get_archives_root()?;
    let archive_path = archives_root.join(&archive_name);
    if archive_path.exists() {
        let _ = fs::remove_file(archive_path);
    }

    Ok(())
}

#[tauri::command]
pub async fn update_mod_display_name(
    _state: State<'_, AppState>,
    archive_name: String,
    display_name: String,
) -> Result<()> {
    let staging_root = get_staging_root()?;
    let manager = manifest::ManifestManager::new(&staging_root);
    let mut manifest_data = manager.load_manifest(&archive_name)?.ok_or_else(|| {
        AppError::Validation(format!("Archive manifest not found: {}", archive_name))
    })?;
    manifest_data.display_name = if display_name.trim().is_empty() {
        None
    } else {
        Some(display_name.trim().to_string())
    };
    manager.save_manifest(&manifest_data)?;
    Ok(())
}

#[tauri::command]
pub async fn update_mod_source_url(
    state: State<'_, AppState>,
    archive_name: String,
    source_url: String,
) -> Result<()> {
    let config = state.get_config()?;
    let staging_root = get_staging_root()?;
    let manager = manifest::ManifestManager::new(&staging_root);
    let nexus_service = nexus_api::NexusApiService::new(state.client.clone());
    let modio_service = ModioApiService::new(state.client.clone(), config.modio_game_id);

    let mut manifest_data = manager.load_manifest(&archive_name)?.ok_or_else(|| {
        AppError::Validation(format!("Archive manifest not found: {}", archive_name))
    })?;
    manifest_data.source_url = if source_url.trim().is_empty() {
        None
    } else {
        let trimmed = source_url.trim();
        let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
        let clean_url = without_fragment
            .split('?')
            .next()
            .unwrap_or(without_fragment);
        Some(clean_url.to_string())
    };

    let api_key = config
        .nexus_api_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let oauth_token = config
        .oauth_token
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    if let Some(ref url) = manifest_data.source_url {
        if let Ok(Some(name)) = resolve_display_name_from_source_url(
            url,
            api_key,
            oauth_token,
            &nexus_service,
            &modio_service,
        )
        .await
        {
            manifest_data.display_name = Some(name);
        }
    }

    manager.save_manifest(&manifest_data)?;
    Ok(())
}

#[tauri::command]
pub async fn get_installed_mod_groups(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledModGroup>> {
    let config = state.get_config()?;
    let game_path = config
        .game_path
        .ok_or_else(|| AppError::Validation("Game path is not configured".to_string()))?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root()?;

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

    if mods_path.exists() {
        for entry in fs::read_dir(&mods_path)? {
            let entry = entry?;
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
    }

    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(groups)
}

#[tauri::command]
pub async fn uninstall_mod(state: State<'_, AppState>, filename: String) -> Result<()> {
    let config = state.get_config()?;
    let game_path = config
        .game_path
        .ok_or_else(|| AppError::Validation("Game path is not configured".to_string()))?;
    let mods_path = steam::get_mods_path(&game_path);
    let staging_root = get_staging_root()?;

    let manager = manifest::ManifestManager::new(&staging_root);
    if let Ok(Some((archive_name, manifest))) = manager.get_manifest_for_pak(&filename) {
        remove_mod_from_active_profile(&state, &archive_name)?;
        if is_mod_used_by_any_profile(&archive_name)? {
            return Ok(());
        }
        for file_path in &manifest.installed_files {
            if file_path.exists() {
                if let Err(e) = fs::remove_file(file_path) {
                    eprintln!("Failed to remove {:?}: {}", file_path, e);
                } else {
                    cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
                }
            }
        }
        cleanup_mod_staging_directories(&archive_name, &staging_root);
        let _ = manager.delete_manifest(&archive_name);
        return Ok(());
    }

    remove_mod_from_active_profile(&state, &filename)?;
    if is_mod_used_by_any_profile(&filename)? {
        return Ok(());
    }
    let mod_path = mods_path.join(&filename);
    if !mod_path.exists() {
        return Err(AppError::Validation(format!(
            "Mod file not found: {}",
            filename
        )));
    }
    fs::remove_file(&mod_path)?;
    cleanup_empty_install_dirs(&mod_path, &staging_root, &mods_path);
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModInstallResult {
    pub was_duplicate: bool,
}

#[tauri::command]
pub async fn install_local_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    #[allow(non_snake_case)] filePath: String,
) -> Result<LocalModInstallResult> {
    let config = state.get_config()?;
    let game_path = config
        .game_path
        .ok_or_else(|| AppError::Validation("Game path is not configured".to_string()))?;

    let staging_root = get_staging_root()?;
    let paks_path = staging_root.join("mods");
    let savegames_path = staging_root.join("savegames");
    let backup_path = staging_root.join("backups");

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "install".to_string(),
            file: filePath.clone(),
            percent: 0.0,
            message: "Starting installation...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let path = PathBuf::from(&filePath);
    let context = installer::InstallContext {
        game_path: game_path.clone(),
        mods_path: paks_path,
        savegames_path,
        backup_path,
    };

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "install".to_string(),
            file: filePath.clone(),
            percent: 4.0,
            message: "Preparing file for install...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    match install_downloaded_file(&path, &context, &app) {
        Ok(is_duplicate) => {
            if let Some(installed_mod_name) = path.file_name().and_then(|n| n.to_str()) {
                let _ = add_mod_to_active_profile(&state, installed_mod_name);
            }
            sync_active_profile_links(&state)?;
            let _ = app.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "complete".to_string(),
                    file: filePath,
                    percent: 100.0,
                    message: "Installation complete!".to_string(),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );
            Ok(LocalModInstallResult {
                was_duplicate: is_duplicate,
            })
        }
        Err(e) => {
            let error_msg = format!("Failed to install mod: {}", e);
            let _ = app.emit("install_progress", &ProgressEvent::new_error(error_msg));
            Err(e)
        }
    }
}

fn save_install_manifest(
    archive_path: &Path,
    report: &installer::InstallReport,
    _context: &installer::InstallContext,
    content_hash: Option<String>,
) -> Result<()> {
    if report.installed_files.is_empty() {
        return Ok(());
    }
    let archive_name = archive_path
        .file_name()
        .and_then(|n: &std::ffi::OsStr| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let manager = manifest::ManifestManager::new(&get_staging_root()?);
    let manifest = manifest::InstallManifest {
        source_archive: archive_name.clone(),
        display_name: None,
        source_url: None,
        installed_files: report.installed_files.clone(),
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        content_hash,
    };

    if let Some(file_name) = archive_path.file_name() {
        let archives_root = get_archives_root()?;
        let dest = archives_root.join(file_name);
        if !dest.exists() {
            let _ = fs::copy(archive_path, &dest);
        }
    }

    manager.save_manifest(&manifest)?;
    Ok(())
}

fn install_downloaded_file(
    path: &PathBuf,
    context: &installer::InstallContext,
    app: &AppHandle,
) -> Result<bool> {
    let hash_start = Instant::now();
    let mut hash_last_emit = Instant::now() - Duration::from_millis(500);

    let content_hash = hasher::md5_file_with_progress(path, |processed_bytes, total_bytes| {
        let now = Instant::now();
        let done = total_bytes > 0 && processed_bytes >= total_bytes;
        if now.duration_since(hash_last_emit) < Duration::from_millis(120) && !done {
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
                mib_per_sec
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
    .map_err(|e| AppError::Validation(format!("Failed to hash file: {}", e)))?;

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

    let manager = manifest::ManifestManager::new(&get_staging_root()?);
    if manager.find_by_content_hash(&content_hash)?.is_some() {
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
        return Ok(true);
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
    if extension.eq_ignore_ascii_case("zip") {
        let start = Instant::now();
        let mut last_emit = Instant::now() - Duration::from_millis(500);
        let report = installer::install_archive_with_progress(path, &staged_context, |progress| {
            let now = Instant::now();
            if now.duration_since(last_emit) < Duration::from_millis(120)
                && progress.processed_bytes < progress.total_bytes
            {
                return;
            }
            last_emit = now;
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let mib_per_sec = (progress.processed_bytes as f64 / elapsed) / (1024.0 * 1024.0);
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
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(false);
    }

    if extension.eq_ignore_ascii_case("rar") {
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
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(false);
    }

    if extension.eq_ignore_ascii_case("7z") {
        return Err(AppError::Validation("7Z archives are not directly supported. Please extract the archive first, then drag the .pak or .zip file.".to_string()));
    }

    if extension.eq_ignore_ascii_case("pak") {
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
        let file_name = path
            .file_name()
            .ok_or_else(|| AppError::Validation("invalid pak file path".to_string()))?;
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
        return Ok(false);
    }

    if extension.eq_ignore_ascii_case("sav") {
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
        let file_name = path
            .file_name()
            .ok_or_else(|| AppError::Validation("invalid sav file path".to_string()))?;
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
        return Ok(false);
    }

    Ok(false)
}
