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

        let content_hash = manifest.content_hash.clone();

        let pak_filenames: Vec<String> = manifest
            .installed_files
            .iter()
            .filter_map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
            .filter(|name| name.to_lowercase().ends_with(".pak"))
            .collect();
        let selected_pak_files = if pak_filenames.is_empty() {
            None
        } else {
            Some(pak_filenames)
        };

        mods.insert(
            archive_name,
            ModEntry {
                enabled: is_enabled,
                source_url,
                content_hash,
                selected_pak_files,
                nexus_file_id: manifest.nexus_file_id,
            },
        );
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

    let mut tags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (tag_name, mod_archives) in &profile.tags {
        let mut sorted = mod_archives.clone();
        sorted.sort_by_key(|a| a.to_lowercase());
        tags.insert(tag_name.clone(), sorted);
    }

    let broken: BTreeMap<String, String> = profile
        .broken_mods
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let mut no_world_gen = profile.no_world_gen.clone();
    no_world_gen.sort_by_key(|a| a.to_lowercase());

    Ok(ModPack {
        schema_version: 1,
        name: format!("{} ModPack", active_profile_name),
        version: "0.1.0".to_string(),
        description: format!("Exported from profile: {}", active_profile_name),
        author: None,
        mods,
        collections,
        addons,
        tags,
        broken,
        no_world_gen,
    })
}

#[tauri::command]
pub async fn apply_modpack_profile_metadata(
    state: State<'_, AppState>,
    modpack: ModPack,
) -> Result<()> {
    let config = state.get_config()?;
    let Some(profile_name) = config.active_profile else {
        return Ok(());
    };
    let Some(mut profile) = profiles::get_profile(&profile_name)? else {
        return Ok(());
    };

    for (tag_name, archives) in &modpack.tags {
        let entry = profile.tags.entry(tag_name.clone()).or_default();
        for archive in archives {
            if !entry.contains(archive) {
                entry.push(archive.clone());
            }
        }
    }

    for (col_name, col) in &modpack.collections {
        let entry = profile.collections.entry(col_name.clone()).or_default();
        for mod_name in &col.mods {
            if !entry.contains(mod_name) {
                entry.push(mod_name.clone());
            }
        }
        if col.default_enabled && !profile.enabled_collections.contains(col_name) {
            profile.enabled_collections.push(col_name.clone());
        }
    }

    for (archive, note) in &modpack.broken {
        profile
            .broken_mods
            .entry(archive.clone())
            .or_insert_with(|| note.clone());
    }

    for archive in &modpack.no_world_gen {
        if !profile.no_world_gen.contains(archive) {
            profile.no_world_gen.push(archive.clone());
        }
    }

    let mut disabled_changed = false;
    for (archive, entry) in &modpack.mods {
        if !entry.enabled {
            let before = profile.installed_mod_names.len();
            profile.installed_mod_names.retain(|name| name != archive);
            if profile.installed_mod_names.len() != before {
                disabled_changed = true;
            }
        }
    }

    profiles::save_profile(&profile)?;

    if disabled_changed {
        if let Some(game_path) = config.game_path.as_ref() {
            let _ = super::game::sync_mod_links_for_game_path(
                game_path,
                profile.installed_mod_names.clone(),
            );
        }
    }

    if !modpack.addons.is_empty() {
        let mut local_map = addon_map::read_addon_map().unwrap_or_default();
        for (parent, addon_archives) in &modpack.addons {
            let entry = local_map.entry(parent.clone()).or_default();
            for a in addon_archives {
                if !entry.contains(a) {
                    entry.push(a.clone());
                }
            }
        }
        let _ = addon_map::write_addon_map(&local_map);
    }

    Ok(())
}

#[tauri::command]
pub async fn export_modpack_to_file(
    app: AppHandle,
    modpack: ModPack,
    dir_path: String,
    state: State<'_, AppState>,
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

    // mod.io mods are downloaded directly on import - don't bundle their archives
    let mod_list: Vec<_> = modpack
        .mods
        .iter()
        .filter(|(_, entry)| {
            entry
                .source_url
                .as_ref()
                .map(|u| !u.to_lowercase().contains("mod.io"))
                .unwrap_or(true)
        })
        .map(|(k, _)| k.clone())
        .collect();

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
        let n = current + 1;
        let _ = app.emit(
            "export_progress",
            ProgressEvent {
                operation: "export_copy".to_string(),
                file: archive_name.clone(),
                percent: slot_percent(current, 0.0),
                message: format!("{n}/{total} Copying {label} {archive_name}..."),
                total_bytes: None,
                processed_bytes: None,
            },
        );

        if src.exists() {
            let src_size = src.metadata().map(|m| m.len()).unwrap_or(0);
            let dst_size = dst.metadata().map(|m| m.len()).unwrap_or(0);
            let unchanged = dst.exists() && src_size > 0 && dst_size == src_size;

            if unchanged {
                current += 1;
                let _ = app.emit(
                    "export_progress",
                    ProgressEvent {
                        operation: "export".to_string(),
                        file: archive_name.clone(),
                        percent: slot_percent(current, 0.0),
                        message: format!("{n}/{total} Skipped {archive_name} (unchanged)"),
                        total_bytes: None,
                        processed_bytes: None,
                    },
                );
                continue;
            }

            let cached_hash = manifest_manager
                .load_manifest(archive_name)
                .ok()
                .flatten()
                .and_then(|m| m.content_hash);

            let copy_start = Instant::now();
            let mut last_emit = Instant::now();

            let (_, copy_hash) =
                hasher::copy_file_with_hash_and_progress(&src, &dst, |processed, file_size| {
                    let now = Instant::now();
                    if now.duration_since(last_emit).as_millis() < 50 {
                        return;
                    }
                    last_emit = now;
                    let elapsed = copy_start.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.001 {
                        (processed as f64 / elapsed) as u64
                    } else {
                        0
                    };
                    let frac = if file_size > 0 {
                        processed as f32 / file_size as f32
                    } else {
                        0.0
                    };
                    let _ = app.emit(
                        "export_progress",
                        ProgressEvent {
                            operation: "export_copy".to_string(),
                            file: archive_name.clone(),
                            percent: slot_percent(current, frac),
                            message: format!(
                                "{n}/{total} Copying {archive_name}... @ {}",
                                format_speed(speed)
                            ),
                            total_bytes: Some(file_size),
                            processed_bytes: Some(processed),
                        },
                    );
                })
                .map_err(|e| AppError::Validation(format!("Failed to copy {archive_name}: {e}")))?;

            if let Some(expected) = cached_hash {
                if copy_hash != expected {
                    return Err(AppError::Validation(format!(
                        "Hash mismatch for {archive_name}: copy is corrupt"
                    )));
                }
            }
        }

        current += 1;
        let _ = app.emit(
            "export_progress",
            ProgressEvent {
                operation: "export".to_string(),
                file: archive_name.clone(),
                percent: slot_percent(current, 0.0),
                message: format!("{n}/{total} Copied {archive_name}"),
                total_bytes: None,
                processed_bytes: None,
            },
        );
    }

    // Remove files in the export mods dir that are no longer in the modpack.
    let expected: std::collections::HashSet<&str> =
        all_archives.iter().map(String::as_str).collect();
    if let Ok(entries) = fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !expected.contains(name_str.as_ref()) {
                let _ = fs::remove_file(entry.path());
            }
        }
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

    let _ = state.update_config(|cfg| {
        cfg.last_export_dir = Some(dir_path.to_string_lossy().to_string());
    });

    let _ = app.emit("export_complete", dir_path.to_string_lossy().to_string());
    Ok(())
}
