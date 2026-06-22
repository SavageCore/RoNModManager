use tauri::State;
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
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize; // This line is kept as it is needed
use tauri::{AppHandle, Emitter}; // Removed duplicate State import

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

fn list_archive_paks(path: &Path) -> Result<Vec<PakFileInfo>> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    if extension.eq_ignore_ascii_case("zip") {
        let file = fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| AppError::Validation(format!("invalid zip archive: {e}")))?;
        let mut paks = Vec::new();
        for i in 0..archive.len() {
            let entry = archive
                .by_index(i)
                .map_err(|e| AppError::Validation(format!("zip entry error: {e}")))?;
            if entry.is_dir() {
                continue;
            }
            let entry_path = PathBuf::from(entry.name());
            if installer::classify_archive_entry(&entry_path) == installer::ModFileType::PakMod {
                let name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                paks.push(PakFileInfo {
                    name,
                    path: entry.name().to_string(),
                    size: entry.size(),
                });
            }
        }
        return Ok(paks);
    }

    if extension.eq_ignore_ascii_case("rar") {
        use unrar::Archive as RarArchive;
        let mut paks = Vec::new();
        let archive = RarArchive::new(path)
            .open_for_listing()
            .map_err(|e| AppError::Validation(format!("failed to open RAR: {e:?}")))?;
        for entry_result in archive {
            let entry = entry_result
                .map_err(|e| AppError::Validation(format!("RAR entry error: {e:?}")))?;
            if entry.is_directory() {
                continue;
            }
            let entry_path = &entry.filename;
            if installer::classify_archive_entry(entry_path) == installer::ModFileType::PakMod {
                let name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                paks.push(PakFileInfo {
                    name,
                    path: entry.filename.to_string_lossy().to_string(),
                    size: entry.unpacked_size,
                });
            }
        }
        return Ok(paks);
    }

    if extension.eq_ignore_ascii_case("7z") {
        let mut paks = Vec::new();
        let archive = sevenz_rust2::Archive::open(path)
            .map_err(|e| AppError::Validation(format!("failed to open 7z: {e}")))?;
        for entry in &archive.files {
            if entry.is_directory {
                continue;
            }
            let entry_path = PathBuf::from(&entry.name);
            if installer::classify_archive_entry(&entry_path) == installer::ModFileType::PakMod {
                let name = entry_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                paks.push(PakFileInfo {
                    name,
                    path: entry.name.clone(),
                    size: entry.size,
                });
            }
        }
        return Ok(paks);
    }

    Ok(vec![])
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddModioResult {
    pub mod_id: u64,
    pub name: String,
    pub archive_name: String,
    pub source_url: String,
    pub archive_path: String,
    pub content_hash: Option<String>,
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

fn remove_mod_from_active_profile(state: &State<'_, AppState>, mod_name: &str) -> Result<()> {
    let config = state.get_config()?;
    let Some(active_profile_name) = config.active_profile else {
        return Ok(());
    };

    let Some(mut profile) = profiles::get_profile(&active_profile_name)? else {
        return Ok(());
    };

    let before_installed_len = profile.installed_mod_names.len();
    profile.installed_mod_names.retain(|name| name != mod_name);
    let installed_changed = profile.installed_mod_names.len() != before_installed_len;

    // Remove mod from tag member lists; keep empty tag keys so they're ready to re-assign
    let mut meta_changed = false;
    for members in profile.tags.values_mut() {
        let before = members.len();
        members.retain(|name| name != mod_name);
        if members.len() != before {
            meta_changed = true;
        }
    }

    // Remove mod from collection member lists; drop collections that become empty
    for members in profile.collections.values_mut() {
        let before = members.len();
        members.retain(|name| name != mod_name);
        if members.len() != before {
            meta_changed = true;
        }
    }
    let empty_collections: Vec<String> = profile
        .collections
        .iter()
        .filter(|(_, members)| members.is_empty())
        .map(|(name, _)| name.clone())
        .collect();
    for col in &empty_collections {
        profile.collections.remove(col);
        profile.collection_colors.remove(col);
        profile.enabled_collections.retain(|name| name != col);
    }

    if installed_changed || meta_changed {
        profiles::save_profile(&profile)?;
    }
    if installed_changed {
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

pub(crate) fn archive_install_key(archive_name: &str) -> String {
    PathBuf::from(archive_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

fn cleanup_mod_staging_directories(archive_name: &str, staging_root: &Path) {
    let install_key = archive_install_key(archive_name);
    let _ = fs::remove_dir_all(staging_root.join("mods").join(&install_key));
    let _ = fs::remove_dir_all(staging_root.join("savegames").join(&install_key));
    let _ = fs::remove_dir_all(staging_root.join("backups").join(&install_key));
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

/// Download a modpack archive from Nexus by matching its filename against the mod's file list.
/// Requires a Premium account (caller must have already confirmed `is_premium`).
/// Falls back gracefully - callers should catch the error and use the self-hosted URL instead.
async fn nexus_download_by_filename(
    client: &reqwest::Client,
    nexus_service: &nexus_api::NexusApiService,
    api_key: &str,
    mod_id: u64,
    archive_name: &str,
    dest: &Path,
) -> Result<String> {
    let files = nexus_service.list_mod_files(api_key, mod_id).await?;
    let matched = files
        .iter()
        .find(|f| f.file_name.eq_ignore_ascii_case(archive_name))
        .ok_or_else(|| {
            AppError::Validation(format!(
                "File '{}' not found in Nexus mod {}",
                archive_name, mod_id
            ))
        })?;
    let links = nexus_service
        .get_download_links(api_key, mod_id, matched.file_id)
        .await?;
    let url = links
        .first()
        .map(|l| l.uri.clone())
        .ok_or_else(|| AppError::Validation("No download links returned".to_string()))?;
    downloader::download_file(client, &url, dest).await
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

    // Fetch the pack once - used for collection filtering, Nexus source map, and addon merge.
    let pack = modpack_service::fetch_modpack(&state.client, &modpack_url)
        .await
        .ok();

    // Build archive_name -> Nexus URL map from mods that have a nexusmods.com source.
    let nexus_source_map: HashMap<String, String> = pack
        .as_ref()
        .map(|p| {
            p.mods
                .iter()
                .filter_map(|(archive, entry)| {
                    entry.source_url.as_ref().and_then(|url| {
                        if url.to_lowercase().contains("nexusmods.com") {
                            Some((archive.clone(), url.clone()))
                        } else {
                            None
                        }
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let mut filtered_manifest = manifest.clone();
    if let Some(ref enabled_cols) = enabled_collections {
        if let Some(ref modpack) = pack {
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

    let download_root = app_temp_root()?.join("modpack_downloads");
    fs::create_dir_all(&download_root)?;

    // Check Nexus premium status once if there are Nexus-sourced mods and an API key.
    let nexus_premium_ctx: Option<(nexus_api::NexusApiService, String)> =
        if !nexus_source_map.is_empty() {
            if let Some(api_key) = config.nexus_api_key.clone() {
                let svc = nexus_api::NexusApiService::new(state.client.clone());
                match svc.get_user_info(&api_key).await {
                    Ok(user) if user.is_premium => Some((svc, api_key)),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };

    let total = filtered_manifest.files.len();
    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "download_start".to_string(),
            file: format!("{} files", total),
            percent: 10.0,
            message: format!("Downloading {} files...", total),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let mut downloaded_files: Vec<(PathBuf, Option<String>)> = Vec::with_capacity(total);

    for (idx, entry) in filtered_manifest.files.iter().enumerate() {
        let local_path = download_root.join(&entry.path);
        let archive_name = Path::new(&entry.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&entry.path)
            .to_string();

        // Re-use a cached download if the size matches.
        if local_path.exists() {
            let actual_size = std::fs::metadata(&local_path).map(|m| m.len()).unwrap_or(0);
            if entry.size == 0 || actual_size == entry.size {
                downloaded_files.push((local_path, None));
                continue;
            }
        }

        // Re-use a matching file the user has already downloaded, validated by MD5
        // (from the modpack entry) where available, otherwise by byte size.
        let expected_md5 = pack
            .as_ref()
            .and_then(|p| p.mods.get(&archive_name))
            .and_then(|m| m.content_hash.as_deref());
        let pct = 10.0 + (idx as f32 / total as f32) * 38.0;
        let hash_started = Instant::now();
        if let Some(found) = downloader::find_in_downloads(
            &archive_name,
            Some(entry.size),
            expected_md5,
            |processed, total_bytes| {
                let elapsed = hash_started.elapsed().as_secs_f64().max(0.001);
                let mib_per_sec = bytes_to_mib(processed) / elapsed;
                let _ = app.emit(
                    "install_progress",
                    &ProgressEvent {
                        operation: "hash".to_string(),
                        file: archive_name.clone(),
                        percent: pct,
                        message: format!(
                            "Hashing {} from Downloads... {:.1} MiB/s",
                            archive_name, mib_per_sec
                        ),
                        total_bytes: if total_bytes > 0 {
                            Some(total_bytes)
                        } else {
                            None
                        },
                        processed_bytes: Some(processed),
                    },
                );
            },
        ) {
            if let Some(parent) = local_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&found, &local_path)?;
            let _ = app.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "download".to_string(),
                    file: archive_name.clone(),
                    percent: pct,
                    message: format!("Found {} in Downloads, using local copy", archive_name),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );
            // Pass the validated MD5 through so install does not re-hash.
            downloaded_files.push((local_path, expected_md5.map(|h| h.to_string())));
            continue;
        }

        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "download".to_string(),
                file: archive_name.clone(),
                percent: pct,
                message: format!("{}/{} Downloading {}...", idx + 1, total, archive_name),
                total_bytes: None,
                processed_bytes: None,
            },
        );

        // Prefer Nexus CDN for premium users when a source URL is recorded.
        let mut download_hash: Option<String> = None;
        if let Some((ref svc, ref api_key)) = nexus_premium_ctx {
            if let Some(nexus_url) = nexus_source_map.get(&archive_name) {
                if let Ok(mod_id) = nexus_api::parse_nexus_url_to_mod_id(nexus_url) {
                    match nexus_download_by_filename(
                        &state.client,
                        svc,
                        api_key,
                        mod_id,
                        &archive_name,
                        &local_path,
                    )
                    .await
                    {
                        Ok(hash) => download_hash = Some(hash),
                        Err(e) => log::warn!(
                            "Nexus download failed for {archive_name}, \
                             falling back to self-hosted: {e}"
                        ),
                    }
                }
            }
        }

        if download_hash.is_none() {
            let remote = format!("{}/mods/{}", modpack_url.trim_end_matches('/'), entry.path);
            download_hash =
                Some(downloader::download_file(&state.client, &remote, &local_path).await?);
        }

        if entry.size > 0 {
            let actual_size = std::fs::metadata(&local_path).map(|m| m.len()).unwrap_or(0);
            if actual_size != entry.size {
                return Err(AppError::Validation(format!(
                    "Size mismatch for {}: expected {} bytes, got {}",
                    entry.path, entry.size, actual_size
                )));
            }
        }

        downloaded_files.push((local_path, download_hash));
    }

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

    for (index, (file, download_hash)) in downloaded_files.iter().enumerate() {
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
        install_downloaded_file(
            file,
            &install_context,
            &app,
            &download_root,
            None,
            download_hash.clone(),
        )?;

        if let Some(ref pack_data) = pack {
            if !pack_data.addons.is_empty() {
                let is_parent = pack_data.addons.contains_key(&file_name);
                let is_addon =
                    !is_parent && pack_data.addons.values().any(|v| v.contains(&file_name));
                if is_parent || is_addon {
                    let mut local_map = addon_map::read_addon_map().unwrap_or_default();
                    if is_parent {
                        if let Some(addon_archives) = pack_data.addons.get(&file_name) {
                            let entry = local_map.entry(file_name.clone()).or_default();
                            for a in addon_archives {
                                if !entry.contains(a) {
                                    entry.push(a.clone());
                                }
                            }
                        }
                    } else {
                        for (parent, addons) in &pack_data.addons {
                            if addons.contains(&file_name) {
                                let entry = local_map.entry(parent.clone()).or_default();
                                if !entry.contains(&file_name) {
                                    entry.push(file_name.clone());
                                }
                            }
                        }
                    }
                    let _ = addon_map::write_addon_map(&local_map);
                }
            }
        }
    }

    let _ = app.emit("install_progress", &ProgressEvent::new_complete());

    // Merge broken-mod flags from the modpack into the active profile.
    // Existing local notes are preserved (don't overwrite).
    if let Some(ref pack_data) = pack {
        if !pack_data.broken.is_empty() || !pack_data.no_world_gen.is_empty() {
            if let Ok(Some(profile_name)) = state.get_config().map(|c| c.active_profile) {
                if let Ok(Some(mut profile)) = profiles::get_profile(&profile_name) {
                    for (archive, note) in &pack_data.broken {
                        profile
                            .broken_mods
                            .entry(archive.clone())
                            .and_modify(|existing| {
                                if existing.is_empty() && !note.is_empty() {
                                    *existing = note.clone();
                                }
                            })
                            .or_insert_with(|| note.clone());
                    }
                    for archive in &pack_data.no_world_gen {
                        if !profile.no_world_gen.contains(archive) {
                            profile.no_world_gen.push(archive.clone());
                        }
                    }
                    profiles::save_profile(&profile)?;
                }
            }
        }
    }

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

    // 2. Remove game-folder files while manifests still exist so the path
    //    mapping is available. On Windows, files may be hard links or copies
    //    (not symlinks), so remove_orphan_symlinks() won't catch them later.
    game::sync_mod_links_for_game_path(&game_path, Vec::new()).map_err(AppError::Validation)?;

    // 3. Delete files and manifests
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

    // 3b. Remove any orphaned per-mod subdirectories in staging (not caught by manifests)
    for subdir in &["mods", "savegames", "backups"] {
        let parent = staging_root.join(subdir);
        if let Ok(entries) = fs::read_dir(&parent) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let _ = fs::remove_dir_all(&path);
                }
            }
        }
    }

    // 4. Clear archives
    let archives_root = get_archives_root()?;
    if archives_root.exists() {
        let _ = fs::remove_dir_all(&archives_root);
        let _ = fs::create_dir_all(&archives_root);
    }

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

    let download_root = app_temp_root()?.join("modio_downloads");
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

    let content_hash = downloader::download_file_with_progress(
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

    if let Some(remote_md5) = &mod_details.remote_md5 {
        if content_hash.to_lowercase() != remote_md5.to_lowercase() {
            return Err(AppError::Validation(format!(
                "Hash mismatch for {}: download may be corrupt",
                archive_name
            )));
        }
    }

    Ok(AddModioResult {
        mod_id: mod_details.id,
        name: mod_details.name,
        archive_name,
        source_url: mod_details.profile_url,
        archive_path: archive_path.to_string_lossy().to_string(),
        content_hash: Some(content_hash),
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFileOption {
    pub file_id: u64,
    pub file_name: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[tauri::command]
pub async fn list_nexus_file_options(
    state: State<'_, AppState>,
    input: String,
) -> Result<Vec<NexusFileOption>> {
    let config = state.get_config()?;
    let api_key = config.nexus_api_key.ok_or_else(|| {
        AppError::Validation(
            "Nexus Mods API key is not configured. Add it in Settings first.".to_string(),
        )
    })?;

    let mod_id = nexus_api::parse_nexus_url_to_mod_id(&input)?;
    let nexus_service = nexus_api::NexusApiService::new(state.client.clone());
    let files = nexus_service.list_mod_files(&api_key, mod_id).await?;

    let options = nexus_api::get_file_options(&files)
        .into_iter()
        .map(|f| NexusFileOption {
            file_id: f.file_id,
            file_name: f.file_name.clone(),
            name: f.name.clone(),
            version: f.version.clone(),
            description: f.description.clone(),
        })
        .collect();

    Ok(options)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddNexusResult {
    pub mod_id: u64,
    pub name: String,
    pub archive_name: String,
    pub source_url: String,
    pub archive_path: String,
    pub file_id: u64,
    pub file_pretty_name: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NexusFreeDownloadWaitingPayload {
    pretty_name: Option<String>,
    file_name: String,
    mod_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NexusFreeDownloadCompletePayload {
    file_name: String,
}

#[tauri::command]
pub async fn add_nexus_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    file_id: Option<u64>,
) -> Result<AddNexusResult> {
    state
        .nexus_cancel
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let config = state.get_config()?;
    let api_key = config.nexus_api_key.ok_or_else(|| {
        AppError::Validation(
            "Nexus Mods API key is not configured. Add it in Settings first.".to_string(),
        )
    })?;

    let mod_id = nexus_api::parse_nexus_url_to_mod_id(&input)?;
    let nexus_service = nexus_api::NexusApiService::new(state.client.clone());

    let user_info = nexus_service.get_user_info(&api_key).await?;
    let is_premium = user_info.is_premium;

    let mod_info = nexus_service.get_mod_info(&api_key, mod_id).await?;
    let mod_name = mod_info.name.clone();

    let files = nexus_service.list_mod_files(&api_key, mod_id).await?;
    let primary_file = if let Some(fid) = file_id {
        files.iter().find(|f| f.file_id == fid).ok_or_else(|| {
            AppError::Validation(format!("File ID {} not found for mod {}", fid, mod_id))
        })?
    } else {
        nexus_api::pick_primary_file(&files).ok_or_else(|| {
            AppError::Validation(format!("No downloadable files found for mod {}", mod_id))
        })?
    };
    let expected_filename = primary_file.file_name.clone();
    let expected_size = primary_file.size_in_bytes;
    let file_pretty_name = primary_file.name.clone();
    let file_id = primary_file.file_id;

    let source_url = format!("https://www.nexusmods.com/readyornot/mods/{}", mod_id);
    let archive_name = sanitize_filename_for_download(&expected_filename);

    let (install_path, content_hash) = if is_premium {
        // Premium: download directly via API without opening a browser
        let links = nexus_service
            .get_download_links(&api_key, mod_id, file_id)
            .await?;
        let download_url = links.first().map(|l| l.uri.clone()).ok_or_else(|| {
            AppError::Validation("No download links returned by Nexus API".to_string())
        })?;

        let download_root = app_temp_root()?.join("nexus_downloads");
        fs::create_dir_all(&download_root)?;
        let archive_path = download_root.join(&archive_name);

        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "download".to_string(),
                file: archive_name.clone(),
                percent: 10.0,
                message: format!("Downloading {}...", mod_name),
                total_bytes: None,
                processed_bytes: None,
            },
        );

        let app_for_progress = app.clone();
        let archive_for_progress = archive_name.clone();
        let mod_name_for_progress = mod_name.clone();
        let download_started = Instant::now();

        let hash = downloader::download_file_with_progress(
            &state.client,
            &download_url,
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

        (archive_path, Some(hash))
    } else {
        // Non-premium: watch ~/Downloads, only open browser if the file isn't already there.
        // Uses find_in_downloads for tolerant name matching (whitespace/case/browser dedup suffix).
        let files_url = format!("{}?tab=files", source_url);

        // Fail fast with a meaningful error if the OS has no Downloads directory.
        let downloads_dir = dirs::download_dir()
            .ok_or_else(|| AppError::Validation("Cannot locate Downloads directory".to_string()))?;

        let timeout = std::time::Duration::from_secs(1800);
        let poll_interval = std::time::Duration::from_secs(2);
        let started = std::time::Instant::now();

        let mut found_path: Option<PathBuf> = None;

        // Pre-check: file may already be present from a previous attempt.
        if let Some(path) =
            downloader::find_in_downloads(&expected_filename, expected_size, None, |_, _| {})
        {
            let s1 = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let s2 = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if s1 > 0 && s1 == s2 {
                let _ = app.emit(
                    "nexus_free_download_complete",
                    &NexusFreeDownloadCompletePayload {
                        file_name: expected_filename.clone(),
                    },
                );
                found_path = Some(path);
            }
        }

        if found_path.is_none() {
            let _ = app.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "download".to_string(),
                    file: expected_filename.clone(),
                    percent: 5.0,
                    message: format!("Opening Nexus download page for {}...", mod_name),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );

            let _ =
                tauri_plugin_opener::OpenerExt::opener(&app).open_url(&files_url, None::<String>);

            let _ = app.emit(
                "nexus_free_download_waiting",
                &NexusFreeDownloadWaitingPayload {
                    pretty_name: file_pretty_name.clone(),
                    file_name: expected_filename.clone(),
                    mod_url: files_url.clone(),
                },
            );

            let _ = app.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "download".to_string(),
                    file: expected_filename.clone(),
                    percent: 10.0,
                    message: format!(
                        "Waiting for {} in {}...",
                        expected_filename,
                        downloads_dir.display()
                    ),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );

            loop {
                if state.nexus_cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = app.emit(
                        "install_progress",
                        &ProgressEvent {
                            operation: "error".to_string(),
                            file: expected_filename.clone(),
                            percent: 0.0,
                            message: "Download cancelled.".to_string(),
                            total_bytes: None,
                            processed_bytes: None,
                        },
                    );
                    return Err(AppError::Validation(
                        "CANCELLED: Download cancelled by user".to_string(),
                    ));
                }

                if started.elapsed() >= timeout {
                    return Err(AppError::Validation(format!(
                        "Timed out waiting for {} in Downloads. Download the file manually and use 'Local File' to install it.",
                        expected_filename
                    )));
                }

                if let Some(path) = downloader::find_in_downloads(
                    &expected_filename,
                    expected_size,
                    None,
                    |_, _| {},
                ) {
                    let size_first = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    tokio::time::sleep(poll_interval).await;
                    let size_second = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    if size_first > 0 && size_first == size_second {
                        let _ = app.emit(
                            "nexus_free_download_complete",
                            &NexusFreeDownloadCompletePayload {
                                file_name: expected_filename.clone(),
                            },
                        );
                        found_path = Some(path);
                        break;
                    }
                } else {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }

        let install_path = found_path
            .ok_or_else(|| AppError::Validation("Download not found after waiting".to_string()))?;
        (install_path, None)
    };

    Ok(AddNexusResult {
        mod_id,
        name: mod_name,
        archive_name,
        source_url,
        archive_path: install_path.to_string_lossy().to_string(),
        file_id,
        file_pretty_name,
        content_hash,
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
    let archives_root = get_archives_root()?;

    // Cascade-delete any addon archives linked to this parent
    let mut addon_map_data = addon_map::read_addon_map().unwrap_or_default();
    if let Some(addon_archives) = addon_map_data.remove(&archive_name) {
        for addon_archive in &addon_archives {
            remove_mod_from_active_profile(&state, addon_archive).ok();
            if let Ok(Some(addon_manifest)) = manager.load_manifest(addon_archive) {
                for file_path in &addon_manifest.installed_files {
                    if file_path.exists() && fs::remove_file(file_path).is_ok() {
                        cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
                    }
                }
            }
            cleanup_mod_staging_directories(addon_archive, &staging_root);
            let _ = manager.delete_manifest(addon_archive);
            let addon_archive_path = archives_root.join(addon_archive);
            if addon_archive_path.exists() {
                let _ = fs::remove_file(addon_archive_path);
            }
        }
        // Also remove the parent from any other addon_map entry where it appears as an addon
        for entry in addon_map_data.values_mut() {
            entry.retain(|a| a != &archive_name);
        }
        let _ = addon_map::write_addon_map(&addon_map_data);
    } else {
        // Not a parent - still clean up: remove it from any entry where it appears as an addon
        for entry in addon_map_data.values_mut() {
            entry.retain(|a| a != &archive_name);
        }
        let _ = addon_map::write_addon_map(&addon_map_data);
    }

    remove_mod_from_active_profile(&state, &archive_name)?;
    if is_mod_used_by_any_profile(&archive_name)? {
        return Ok(());
    }

    let manifest_data = manager.load_manifest(&archive_name)?.ok_or_else(|| {
        AppError::Validation(format!("Archive manifest not found: {}", archive_name))
    })?;

    // Restore original bank files before deleting the backup copies.
    {
        let install_key = archive_install_key(&archive_name);
        let backup_dir = staging_root.join("backups").join(&install_key);
        let fmod_path = steam::get_fmod_desktop_path(&game_path);
        for file_path in &manifest_data.installed_files {
            let is_bank = file_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("bank"))
                .unwrap_or(false);
            if !is_bank {
                continue;
            }
            if let Some(file_name) = file_path.file_name() {
                let backup = backup_dir.join(file_name);
                let game_dest = fmod_path.join(file_name);
                if game_dest.is_symlink() || game_dest.exists() {
                    let _ = fs::remove_file(&game_dest);
                }
                if backup.exists() {
                    let _ = fs::copy(&backup, &game_dest);
                }
            }
        }
    }

    // Restore original override files and remove their game-path symlinks.
    {
        let install_key = archive_install_key(&archive_name);
        let staged_mods_key_root = staging_root.join("mods").join(&install_key);
        let overrides_backup_dir = staging_root
            .join("backups")
            .join(&install_key)
            .join("overrides");
        for file_path in &manifest_data.installed_files {
            if !file_path.starts_with(&staged_mods_key_root) {
                continue;
            }
            let relative = match file_path.strip_prefix(&staged_mods_key_root) {
                Ok(r) => r,
                Err(_) => continue,
            };
            // Skip flat files (pak, bank) - only process nested override files
            if relative
                .parent()
                .map(|p| p.as_os_str().is_empty())
                .unwrap_or(true)
            {
                continue;
            }
            let game_dest = game_path.join(relative);
            if game_dest.exists() || game_dest.is_symlink() {
                let _ = fs::remove_file(&game_dest);
            }
            let backup = overrides_backup_dir.join(relative);
            if backup.exists() {
                if let Some(parent) = game_dest.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::copy(&backup, &game_dest);
            }
        }
    }

    // Restore original config files before deleting backups.
    {
        let install_key = archive_install_key(&archive_name);
        let backup_dir = staging_root.join("backups").join(&install_key);
        let config_path = steam::get_config_path()?;
        for file_path in &manifest_data.installed_files {
            let is_ini = file_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ini"))
                .unwrap_or(false);
            if !is_ini {
                continue;
            }
            if let Some(file_name) = file_path.file_name() {
                let backup = backup_dir.join(file_name);
                let game_dest = config_path.join(file_name);
                if game_dest.is_symlink() || game_dest.exists() {
                    let _ = fs::remove_file(&game_dest);
                }
                if backup.exists() {
                    let _ = fs::copy(&backup, &game_dest);
                }
            }
        }
    }

    for file_path in &manifest_data.installed_files {
        if file_path.exists() && fs::remove_file(file_path).is_ok() {
            cleanup_empty_install_dirs(file_path, &staging_root, &mods_path);
        }
    }
    cleanup_mod_staging_directories(&archive_name, &staging_root);
    manager.delete_manifest(&archive_name)?;

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

    let mut manifest_data = match manager.load_manifest(&archive_name)? {
        Some(m) => m,
        None => manifest::InstallManifest {
            source_archive: archive_name.clone(),
            display_name: None,
            source_url: None,
            installed_files: vec![],
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            content_hash: None,
            nexus_file_id: None,
        },
    };
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
pub async fn update_nexus_file_id(
    _state: State<'_, AppState>,
    archive_name: String,
    file_id: u64,
) -> Result<()> {
    let staging_root = get_staging_root()?;
    let manager = manifest::ManifestManager::new(&staging_root);
    if let Some(mut manifest_data) = manager.load_manifest(&archive_name)? {
        manifest_data.nexus_file_id = Some(file_id);
        manager.save_manifest(&manifest_data)?;
    }
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
                    archive_name: None,
                }
            })
            .collect();
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let staging_mods_root = staging_root.join("mods");
        let has_override_files = manifest_data.installed_files.iter().any(|path| {
            let is_bank = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("bank"))
                .unwrap_or(false);
            if is_bank {
                return true;
            }
            // Override files nest into subdirs: staging/mods/{key}/ReadyOrNot/Content/... (>1)
            // Pak files sit flat: staging/mods/{key}/file.pak (1 component after key)
            // Sav files are in staging/savegames/{key}/... and won't match staging_mods_root
            if let Ok(rel) = path.strip_prefix(&staging_mods_root) {
                let mut components = rel.components();
                let _ = components.next(); // skip install key
                let after_key: PathBuf = components.collect();
                after_key.components().count() > 1
            } else {
                false
            }
        });
        groups.push(InstalledModGroup {
            name: manifest_data.source_archive.clone(),
            display_name: manifest_data.display_name.clone(),
            source_url: manifest_data.source_url.clone(),
            managed_by_manifest: true,
            installed_at: Some(manifest_data.installed_at),
            files,
            addon_files: Vec::new(),
            has_override_files,
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
                    archive_name: None,
                }],
                addon_files: Vec::new(),
                has_override_files: false,
            });
        }
    }

    // Apply addon relationships: filter addon groups from the top-level list
    // and attach their files to their parent group's addon_files.
    let addon_map_data = addon_map::read_addon_map().unwrap_or_default();
    let addon_archive_set: HashSet<String> = addon_map_data.values().flatten().cloned().collect();

    let groups_by_name: std::collections::HashMap<String, InstalledModGroup> =
        groups.into_iter().map(|g| (g.name.clone(), g)).collect();

    let mut result: Vec<InstalledModGroup> = groups_by_name
        .iter()
        .filter(|(archive_name, _)| !addon_archive_set.contains(*archive_name))
        .map(|(archive_name, group)| {
            let mut group = group.clone();
            if let Some(addon_archives) = addon_map_data.get(archive_name) {
                group.addon_files = addon_archives
                    .iter()
                    .filter_map(|addon_archive| groups_by_name.get(addon_archive))
                    .flat_map(|addon_group| {
                        addon_group.files.iter().map(|f| InstalledModFile {
                            archive_name: Some(addon_group.name.clone()),
                            ..f.clone()
                        })
                    })
                    .collect();
            }
            group
        })
        .collect();

    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PakFileInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
}

#[tauri::command]
pub async fn get_archive_pak_files(
    #[allow(non_snake_case)] filePath: String,
) -> Result<Vec<PakFileInfo>> {
    let path = PathBuf::from(&filePath);
    list_archive_paks(&path)
}

#[tauri::command]
pub async fn install_local_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    #[allow(non_snake_case)] filePath: String,
    #[allow(non_snake_case)] selectedPakFiles: Option<Vec<String>>,
    #[allow(non_snake_case)] precomputedHash: Option<String>,
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

    let temp_root = crate::state::app_temp_root()?;
    let pak_filter_set: Option<HashSet<String>> = selectedPakFiles.map(|v| v.into_iter().collect());
    match install_downloaded_file(
        &path,
        &context,
        &app,
        &temp_root,
        pak_filter_set.as_ref(),
        precomputedHash,
    ) {
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

fn backup_bank_files_from_report(
    report: &installer::InstallReport,
    game_path: &Path,
    backup_path: &Path,
) -> Result<()> {
    let fmod_path = steam::get_fmod_desktop_path(game_path);
    for installed_file in &report.installed_files {
        let is_bank = installed_file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("bank"))
            .unwrap_or(false);
        if !is_bank {
            continue;
        }
        if let Some(file_name) = installed_file.file_name() {
            let original = fmod_path.join(file_name);
            if original.exists() {
                fs::create_dir_all(backup_path)?;
                fs::copy(&original, backup_path.join(file_name))?;
            }
        }
    }
    Ok(())
}

fn backup_config_files_from_report(
    report: &installer::InstallReport,
    backup_path: &Path,
) -> Result<()> {
    let config_path = steam::get_config_path()?;
    for installed_file in &report.installed_files {
        let is_ini = installed_file
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("ini"))
            .unwrap_or(false);
        if !is_ini {
            continue;
        }
        if let Some(file_name) = installed_file.file_name() {
            let original = config_path.join(file_name);
            if original.exists() {
                fs::create_dir_all(backup_path)?;
                fs::copy(&original, backup_path.join(file_name))?;
            }
        }
    }
    Ok(())
}

fn backup_override_files_from_report(
    report: &installer::InstallReport,
    game_path: &Path,
    staged_context: &installer::InstallContext,
) -> Result<()> {
    for staged_path in &report.installed_files {
        if !staged_path.starts_with(&staged_context.mods_path) {
            continue;
        }
        let relative = match staged_path.strip_prefix(&staged_context.mods_path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        // Override files are nested (e.g. ReadyOrNot/Content/Movies/foo.mp4).
        // Flat files like .pak and .bank sit directly in mods_path with no parent dir.
        if relative
            .parent()
            .map(|p| p.as_os_str().is_empty())
            .unwrap_or(true)
        {
            continue;
        }
        let real_game_file = game_path.join(relative);
        if real_game_file.exists() {
            let backup_dest = staged_context.backup_path.join("overrides").join(relative);
            if let Some(parent) = backup_dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&real_game_file, &backup_dest)?;
        }
    }
    Ok(())
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
        nexus_file_id: None,
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
    temp_root: &Path,
    pak_filter: Option<&HashSet<String>>,
    precomputed_hash: Option<String>,
) -> Result<bool> {
    let content_hash = if let Some(hash) = precomputed_hash {
        hash
    } else {
        let hash_start = Instant::now();
        let mut hash_last_emit = Instant::now() - Duration::from_millis(500);

        hasher::md5_file_with_progress(path, |processed_bytes, total_bytes| {
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
        .map_err(|e| AppError::Validation(format!("Failed to hash file: {}", e)))?
    };

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

    let install_key = archive_install_key(
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown"),
    );
    let staged_context = installer::InstallContext {
        game_path: context.mods_path.join(&install_key),
        mods_path: context.mods_path.join(&install_key),
        savegames_path: context.savegames_path.join(&install_key),
        backup_path: context.backup_path.join(&install_key),
    };

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("zip") {
        let archive_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 50.0,
                message: format!("Installing archive: {}...", archive_name),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let start = Instant::now();
        let mut last_emit = Instant::now() - Duration::from_millis(500);
        let report = installer::install_archive_with_progress(
            path,
            &staged_context,
            |progress| {
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
            },
            pak_filter,
        )?;
        backup_bank_files_from_report(&report, &context.game_path, &staged_context.backup_path)?;
        backup_config_files_from_report(&report, &staged_context.backup_path)?;
        backup_override_files_from_report(&report, &context.game_path, &staged_context)?;
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(false);
    }

    if extension.eq_ignore_ascii_case("rar") {
        let archive_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 50.0,
                message: format!("Installing archive: {}...", archive_name),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let report = installer::install_rar_archive(path, &staged_context, temp_root, pak_filter)?;
        backup_bank_files_from_report(&report, &context.game_path, &staged_context.backup_path)?;
        backup_config_files_from_report(&report, &staged_context.backup_path)?;
        backup_override_files_from_report(&report, &context.game_path, &staged_context)?;
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(false);
    }

    if extension.eq_ignore_ascii_case("7z") {
        let archive_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 50.0,
                message: format!("Installing archive: {}...", archive_name),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let report = installer::install_7z_archive(path, &staged_context, temp_root, pak_filter)?;
        backup_bank_files_from_report(&report, &context.game_path, &staged_context.backup_path)?;
        backup_config_files_from_report(&report, &staged_context.backup_path)?;
        backup_override_files_from_report(&report, &context.game_path, &staged_context)?;
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(false);
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

    if extension.eq_ignore_ascii_case("bank") {
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 75.0,
                message: "Installing FMOD bank file...".to_string(),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let file_name = path
            .file_name()
            .ok_or_else(|| AppError::Validation("invalid bank file path".to_string()))?;
        fs::create_dir_all(&staged_context.mods_path)?;
        let destination = staged_context.mods_path.join(file_name);
        fs::copy(path, &destination)?;
        // Backup the original game bank file so it can be restored when the mod is toggled off.
        let game_bank = steam::get_fmod_desktop_path(&context.game_path).join(file_name);
        if game_bank.exists() {
            fs::create_dir_all(&staged_context.backup_path)?;
            fs::copy(&game_bank, staged_context.backup_path.join(file_name))?;
        }
        let report = installer::InstallReport {
            installed: 1,
            skipped: 0,
            overrides_backed_up: 0,
            installed_files: vec![destination],
        };
        save_install_manifest(path, &report, &staged_context, Some(content_hash))?;
        return Ok(false);
    }

    if extension.eq_ignore_ascii_case("ini") {
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "install".to_string(),
                file: path.to_string_lossy().to_string(),
                percent: 75.0,
                message: "Installing config file...".to_string(),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        let file_name = path
            .file_name()
            .ok_or_else(|| AppError::Validation("invalid ini file path".to_string()))?;
        fs::create_dir_all(&staged_context.mods_path)?;
        let destination = staged_context.mods_path.join(file_name);
        fs::copy(path, &destination)?;
        let config_path = steam::get_config_path()?;
        let game_config = config_path.join(file_name);
        if game_config.exists() {
            fs::create_dir_all(&staged_context.backup_path)?;
            fs::copy(&game_config, staged_context.backup_path.join(file_name))?;
        }
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

use crate::services::addon_map;

#[tauri::command]
pub fn get_addon_map(
    state: tauri::State<'_, crate::state::AppState>,
) -> crate::models::Result<std::collections::HashMap<String, Vec<String>>> {
    let _ = state.get_config()?; // Ensure config is loaded
    addon_map::read_addon_map()
}

#[tauri::command]
pub fn set_addon_map(
    state: tauri::State<'_, crate::state::AppState>,
    map: std::collections::HashMap<String, Vec<String>>,
) -> crate::models::Result<()> {
    let _ = state.get_config()?;
    addon_map::write_addon_map(&map)
}

#[tauri::command]
pub async fn cancel_nexus_download(state: State<'_, AppState>) -> Result<()> {
    state
        .nexus_cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn check_nexus_premium(state: State<'_, AppState>) -> Result<bool> {
    let config = state.get_config()?;
    let Some(api_key) = config.nexus_api_key else {
        return Ok(false);
    };
    let svc = nexus_api::NexusApiService::new(state.client.clone());
    match svc.get_user_info(&api_key).await {
        Ok(user) => Ok(user.is_premium),
        Err(_) => Ok(false),
    }
}
