use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::models::{ModInfo, ModSource, ModStatus};
use crate::services::{installer, modpack as modpack_service, steam};
use crate::state::AppState;

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
pub async fn install_mods(_app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let modpack_url = config
        .modpack_url
        .ok_or_else(|| "Modpack URL is not configured".to_string())?;

    let mods_path = steam::get_mods_path(&game_path);
    let savegames_path = steam::get_savegames_path().map_err(String::from)?;
    let backup_path = game_path.join(".ronmod_backups");

    let manifest = modpack_service::fetch_manifest_if_exists(&state.client, &modpack_url)
        .await
        .map_err(String::from)?;
    let manifest = manifest.ok_or_else(|| {
        "ronmod.manifest not found on modpack host (HTML crawl fallback not implemented yet)"
            .to_string()
    })?;

    let download_root = std::env::temp_dir()
        .join("ronmodmanager")
        .join("modpack_downloads");
    fs::create_dir_all(&download_root).map_err(|error| error.to_string())?;

    let downloaded_files = modpack_service::download_manifest_files(
        &state.client,
        &modpack_url,
        &download_root,
        &manifest,
    )
    .await
    .map_err(String::from)?;

    let install_context = installer::InstallContext {
        game_path: game_path.clone(),
        mods_path: mods_path.clone(),
        savegames_path,
        backup_path,
    };

    for file in downloaded_files {
        install_downloaded_file(&file, &install_context).map_err(String::from)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn uninstall_mods(_app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(String::from)?;
    let game_path = config
        .game_path
        .ok_or_else(|| "Game path is not configured".to_string())?;
    let mods_path = steam::get_mods_path(&game_path);

    if !mods_path.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&mods_path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pak"))
            .unwrap_or(false)
        {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn install_downloaded_file(
    path: &PathBuf,
    context: &installer::InstallContext,
) -> crate::models::Result<()> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();

    if extension.eq_ignore_ascii_case("zip") {
        installer::install_archive(path, context)?;
        return Ok(());
    }

    if extension.eq_ignore_ascii_case("pak") {
        let file_name = path.file_name().ok_or_else(|| {
            crate::models::AppError::Validation("invalid pak file path".to_string())
        })?;
        fs::create_dir_all(&context.mods_path)?;
        fs::copy(path, context.mods_path.join(file_name))?;
        return Ok(());
    }

    if extension.eq_ignore_ascii_case("sav") {
        let file_name = path.file_name().ok_or_else(|| {
            crate::models::AppError::Validation("invalid sav file path".to_string())
        })?;
        fs::create_dir_all(&context.savegames_path)?;
        fs::copy(path, context.savegames_path.join(file_name))?;
    }

    Ok(())
}
