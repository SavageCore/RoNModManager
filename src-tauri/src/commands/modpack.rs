use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn format_speed(bytes_per_sec: u64) -> String {
    let bps = bytes_per_sec as f64;
    if bps >= 1_073_741_824.0 {
        format!("{:.1} GiB/s", bps / 1_073_741_824.0)
    } else if bps >= 1_048_576.0 {
        format!("{:.1} MiB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.1} KiB/s", bps / 1024.0)
    } else {
        format!("{} B/s", bytes_per_sec)
    }
}

use tauri::{AppHandle, Emitter, State};

use crate::models::{AppError, Collection, ModEntry, ModPack, Result};
use crate::services::addon_map;
use crate::services::manifest::ManifestManager;
use crate::services::{modpack as modpack_service, profiles};
use crate::state::AppState;

#[tauri::command]
pub async fn set_modpack_url(url: String, state: State<'_, AppState>) -> Result<()> {
    state
        .update_config(|config| {
            config.modpack_url = Some(url);
        })
        .map(|_| ())
}

#[tauri::command]
pub async fn sync_modpack(_app: AppHandle, state: State<'_, AppState>) -> Result<ModPack> {
    let config = state.get_config()?;
    let url = config
        .modpack_url
        .ok_or_else(|| AppError::Validation("Modpack URL is not configured".to_string()))?;

    let pack = modpack_service::fetch_modpack(&state.client, &url).await?;

    let version = pack.version.clone();
    let _subscriptions = pack.subscriptions.clone();
    state.update_config(|cfg| {
        cfg.modpack_version = Some(version);
    })?;

    Ok(pack)
}

#[tauri::command]
pub async fn get_modpack_collections(
    state: State<'_, AppState>,
) -> Result<HashMap<String, Collection>> {
    let config = state.get_config()?;
    let url = config
        .modpack_url
        .ok_or_else(|| AppError::Validation("Modpack URL is not configured".to_string()))?;

    let pack = modpack_service::fetch_modpack(&state.client, &url).await?;

    let mut collections = HashMap::new();
    for (k, v) in pack.collections {
        collections.insert(k, v);
    }
    Ok(collections)
}

#[tauri::command]
pub async fn build_modpack_from_installed(state: State<'_, AppState>) -> Result<ModPack> {
    let config = state.get_config()?;

    let active_profile_name = config
        .active_profile
        .clone()
        .unwrap_or_else(|| "Default".to_string());

    let profile = profiles::get_profile(&active_profile_name)?.ok_or_else(|| {
        AppError::Validation(format!("Profile '{}' not found", active_profile_name))
    })?;

    let staging_root = crate::state::app_data_root()?.join("staged");
    let manifest_manager = ManifestManager::new(&staging_root);

    let mut subscriptions = BTreeMap::new();
    let mut mods = BTreeMap::new();
    let mut collections = BTreeMap::new();

    let all_manifests = manifest_manager.list_all_manifests()?;
    for (archive_name, manifest) in all_manifests {
        let is_enabled = profile.installed_mod_names.contains(&archive_name);

        let source_url = manifest.source_url.as_ref().map(|url| {
            url.split('#')
                .next()
                .unwrap_or(url)
                .split('?')
                .next()
                .unwrap_or(url)
                .to_string()
        });

        let is_modio = source_url
            .as_ref()
            .map(|url| url.to_lowercase().contains("mod.io"))
            .unwrap_or(false);

        if is_modio {
            if let Some(url) = source_url {
                subscriptions.insert(url, is_enabled);
            }
            continue;
        }

        let entry = ModEntry {
            enabled: is_enabled,
            source_url,
        };

        mods.insert(archive_name, entry);
    }

    for (name, mod_archives) in &profile.collections {
        let is_collection_enabled = profile.enabled_collections.contains(name);
        let mut sorted_mods = mod_archives.clone();
        sorted_mods.sort_by_key(|a| a.to_lowercase());

        collections.insert(
            name.clone(),
            Collection {
                default_enabled: is_collection_enabled,
                mods: sorted_mods,
            },
        );
    }

    let local_addon_map = addon_map::read_addon_map().unwrap_or_default();
    let mut addons: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (parent, addon_archives) in local_addon_map {
        if mods.contains_key(&parent) {
            addons.insert(parent, addon_archives);
        }
    }

    Ok(ModPack {
        schema_version: 1,
        name: format!("{} ModPack", active_profile_name),
        version: "0.1.0".to_string(),
        description: format!("Exported from profile: {}", active_profile_name),
        author: None,
        subscriptions,
        mods,
        collections,
        addons,
    })
}

#[tauri::command]
pub async fn export_modpack_to_file(
    app: AppHandle,
    modpack: ModPack,
    dir_path: String,
    _state: State<'_, AppState>,
) -> Result<()> {
    use crate::models::ProgressEvent;
    use crate::services::hasher;
    use std::fs::File;
    use std::io::Write;

    let mut dir_path = PathBuf::from(&dir_path);

    if dir_path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            if let Ok(stripped) = dir_path.strip_prefix("~") {
                dir_path = home.join(stripped);
            }
        }
    }

    if dir_path.exists() {
        fs::remove_dir_all(&dir_path).map_err(|e| {
            AppError::Validation(format!("Failed to clear existing directory: {e}"))
        })?;
    }

    let mods_dir = dir_path.join("mods");
    fs::create_dir_all(&mods_dir)
        .map_err(|e| AppError::Validation(format!("Failed to create mods dir: {e}")))?;

    let manifest_path = dir_path.join("modpack.json");
    let data = serde_json::to_vec_pretty(&modpack)
        .map_err(|e| AppError::Validation(format!("Failed to serialize manifest: {e}")))?;

    let mut file = File::create(&manifest_path).map_err(|e| {
        AppError::Validation(format!(
            "Failed to create manifest at {:?}: {e}",
            manifest_path
        ))
    })?;

    file.write_all(&data)
        .map_err(|e| AppError::Validation(format!("Failed to write manifest: {e}")))?;

    let staging_root = crate::state::app_data_root()?.join("staged");
    let archives_root = staging_root.join("archives");
    let manifest_manager = ManifestManager::new(&staging_root);

    let mod_list: Vec<_> = modpack.mods.keys().cloned().collect();

    // Compute addon archives upfront so total count is accurate from the start
    let mut extra_archives: Vec<String> = modpack
        .addons
        .values()
        .flatten()
        .filter(|a| !modpack.mods.contains_key(*a))
        .cloned()
        .collect();
    extra_archives.dedup();

    let total = mod_list.len() + extra_archives.len();
    let mut current = 0usize;

    // Combine into one list; addons start at index `mod_list_len`
    let mod_list_len = mod_list.len();
    let all_archives: Vec<String> = mod_list.into_iter().chain(extra_archives).collect();

    // Takes `cur` as a plain parameter so it never borrows the mutable `current`.
    let slot_percent = |cur: usize, frac: f32| -> f32 {
        if total > 0 {
            (cur as f32 + frac) / total as f32 * 100.0
        } else {
            100.0
        }
    };

    for archive_name in &all_archives {
        let is_addon = current >= mod_list_len;
        let src = archives_root.join(archive_name);
        let dst = mods_dir.join(archive_name);

        let label = if is_addon { "addon" } else { "archive" };
        let _ = app.emit(
            "export_progress",
            ProgressEvent {
                operation: "export_copy".to_string(),
                file: archive_name.clone(),
                percent: slot_percent(current, 0.0),
                message: format!("Copying {label} {archive_name}..."),
                total_bytes: None,
                processed_bytes: None,
            },
        );

        if src.exists() {
            // ── COPY PHASE (slot 0.0 → 0.5) ─────────────────────────────────
            let copy_start = Instant::now();
            let mut last_copy_emit = Instant::now();

            hasher::copy_file_with_progress(&src, &dst, |processed, file_size| {
                let now = Instant::now();
                if now.duration_since(last_copy_emit).as_millis() < 50 {
                    return;
                }
                last_copy_emit = now;
                let elapsed = copy_start.elapsed().as_secs_f64();
                let speed = if elapsed > 0.001 {
                    (processed as f64 / elapsed) as u64
                } else {
                    0
                };
                let frac = if file_size > 0 {
                    processed as f32 / file_size as f32 * 0.5
                } else {
                    0.0
                };
                let _ = app.emit(
                    "export_progress",
                    ProgressEvent {
                        operation: "export_copy".to_string(),
                        file: archive_name.clone(),
                        percent: slot_percent(current, frac),
                        message: format!("Copying {archive_name}... @ {}", format_speed(speed)),
                        total_bytes: Some(file_size),
                        processed_bytes: Some(processed),
                    },
                );
            })
            .map_err(|e| AppError::Validation(format!("Failed to copy {archive_name}: {e}")))?;

            // ── VERIFY PHASE (slot 0.5 → 1.0) ────────────────────────────────
            let cached_hash = manifest_manager
                .load_manifest(archive_name)
                .ok()
                .flatten()
                .and_then(|m| m.content_hash);

            let _ = app.emit(
                "export_progress",
                ProgressEvent {
                    operation: "export_verify".to_string(),
                    file: archive_name.clone(),
                    percent: slot_percent(current, 0.5),
                    message: format!("Verifying {archive_name}..."),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );

            let verify_start = Instant::now();
            let mut last_verify_emit = Instant::now();

            let dst_hash = hasher::md5_file_with_progress(&dst, |processed, file_size| {
                let now = Instant::now();
                if now.duration_since(last_verify_emit).as_millis() < 50 {
                    return;
                }
                last_verify_emit = now;
                let elapsed = verify_start.elapsed().as_secs_f64();
                let speed = if elapsed > 0.001 {
                    (processed as f64 / elapsed) as u64
                } else {
                    0
                };
                let frac = if file_size > 0 {
                    0.5 + processed as f32 / file_size as f32 * 0.5
                } else {
                    0.5
                };
                let _ = app.emit(
                    "export_progress",
                    ProgressEvent {
                        operation: "export_verify".to_string(),
                        file: archive_name.clone(),
                        percent: slot_percent(current, frac),
                        message: format!("Verifying {archive_name}... @ {}", format_speed(speed)),
                        total_bytes: Some(file_size),
                        processed_bytes: Some(processed),
                    },
                );
            })
            .map_err(|e| {
                AppError::Validation(format!("Failed to hash copy of {archive_name}: {e}"))
            })?;

            let expected = match cached_hash {
                Some(h) => h,
                None => hasher::md5_file(&src).map_err(|e| {
                    AppError::Validation(format!("Failed to hash source {archive_name}: {e}"))
                })?,
            };

            if dst_hash != expected {
                return Err(AppError::Validation(format!(
                    "Hash mismatch for {archive_name}: copy is corrupt"
                )));
            }
        }

        current += 1;
        // Emit plain "Verified" at the completed slot boundary (no byte details)
        let _ = app.emit(
            "export_progress",
            ProgressEvent {
                operation: "export".to_string(),
                file: archive_name.clone(),
                percent: slot_percent(current, 0.0),
                message: format!("Verified {archive_name}"),
                total_bytes: None,
                processed_bytes: None,
            },
        );
    }

    // Emit complete to trigger status bar auto-hide
    let _ = app.emit(
        "export_progress",
        ProgressEvent {
            operation: "complete".to_string(),
            file: String::new(),
            percent: 100.0,
            message: "Export complete!".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let _ = app.emit("export_complete", dir_path.to_string_lossy().to_string());
    Ok(())
}
