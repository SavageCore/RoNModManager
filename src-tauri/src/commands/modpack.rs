use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};

use crate::models::{AppError, Collection, ModEntry, ModPack, Result};
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

    Ok(ModPack {
        schema_version: 1,
        name: format!("{} ModPack", active_profile_name),
        version: "0.1.0".to_string(),
        description: format!("Exported from profile: {}", active_profile_name),
        author: None,
        subscriptions,
        mods,
        collections,
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

    let mod_list: Vec<_> = modpack.mods.keys().cloned().collect();
    let total = mod_list.len();

    for (current, archive_name) in mod_list.into_iter().enumerate() {
        let src = archives_root.join(&archive_name);
        let dst = mods_dir.join(&archive_name);
        let percent = if total > 0 {
            (current as f32) / (total as f32) * 100.0
        } else {
            100.0
        };

        let _ = app.emit(
            "export_progress",
            ProgressEvent {
                operation: "export".to_string(),
                file: archive_name.clone(),
                percent,
                message: format!("Copying archive {archive_name}..."),
                total_bytes: None,
                processed_bytes: None,
            },
        );

        if src.exists() {
            fs::copy(&src, &dst).map_err(|e| {
                AppError::Validation(format!("Failed to copy archive {archive_name}: {e}"))
            })?;
        }
    }

    let _ = app.emit("export_complete", dir_path.to_string_lossy().to_string());
    Ok(())
}
