use std::path::Path;

use tauri::{AppHandle, Emitter};

use crate::models::{ProgressEvent, Result};
use crate::services::{downloader, installer};
use crate::state::app_data_root;

/// Pinned: mod 8300 (and the wider Ready or Not Lua-modding scene) targets this
/// UE4SS release. Bump both consts together to move to a newer version.
const UE4SS_VERSION: &str = "v3.0.1";
const UE4SS_ARCHIVE_NAME: &str = "UE4SS_v3.0.1.zip";
const UE4SS_URL: &str =
    "https://github.com/UE4SS-RE/RE-UE4SS/releases/download/v3.0.1/UE4SS_v3.0.1.zip";

/// True if UE4SS's core DLL is already present in the live game's Win64 folder
/// (as a real file, or a managed symlink pointing into the staged install).
pub fn is_installed(game_path: &Path) -> bool {
    game_path
        .join("ReadyOrNot/Binaries/Win64/UE4SS.dll")
        .exists()
}

/// Download and install the pinned UE4SS release if it isn't already present,
/// and enable it in the active profile so it links in on the next sync. Safe
/// to call unconditionally before installing a UE4SS-dependent mod - a no-op
/// once UE4SS is on disk.
pub async fn ensure_installed(
    app: &AppHandle,
    client: &reqwest::Client,
    game_path: &Path,
    temp_root: &Path,
) -> Result<()> {
    if is_installed(game_path) {
        return Ok(());
    }

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "download".to_string(),
            file: UE4SS_ARCHIVE_NAME.to_string(),
            percent: 0.0,
            message: format!("Installing UE4SS {UE4SS_VERSION} (required by this mod)..."),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let staging_root = app_data_root()?.join("staged");
    let archives_root = staging_root.join("archives");
    std::fs::create_dir_all(&archives_root)?;
    let archive_path = archives_root.join(UE4SS_ARCHIVE_NAME);

    let content_hash = downloader::download_file(client, UE4SS_URL, &archive_path).await?;

    let context = installer::InstallContext {
        game_path: game_path.to_path_buf(),
        mods_path: staging_root.join("mods"),
        savegames_path: staging_root.join("savegames"),
        backup_path: staging_root.join("backups"),
    };

    // `bundles_runtime` will be true for this archive, so install_downloaded_file's
    // own UE4SS check is a no-op here - no recursion.
    crate::commands::mods::install_downloaded_file(
        &archive_path,
        &context,
        app,
        client,
        temp_root,
        None,
        Some(content_hash),
    )
    .await?;

    let active_profile =
        crate::state::load_config_from_path(&crate::state::app_config_root()?.join("config.json"))
            .map(|config| config.active_profile)
            .unwrap_or(None);
    let _ = crate::commands::mods::add_mod_to_active_profile(
        active_profile.as_deref(),
        UE4SS_ARCHIVE_NAME,
    );

    Ok(())
}
