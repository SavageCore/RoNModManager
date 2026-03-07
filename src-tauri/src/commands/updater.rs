use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("failed to build updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("failed to check for updates: {e}"))?;

    if let Some(update) = update {
        Ok(UpdateInfo {
            available: true,
            version: Some(update.version),
            notes: update.body,
        })
    } else {
        Ok(UpdateInfo {
            available: false,
            version: None,
            notes: None,
        })
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("failed to build updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("failed to check for updates: {e}"))?;

    if let Some(update) = update {
        let version = update.version.clone();
        let notes = update.body.clone();

        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| format!("failed to download and install update: {e}"))?;

        Ok(UpdateInfo {
            available: true,
            version: Some(version),
            notes,
        })
    } else {
        Ok(UpdateInfo {
            available: false,
            version: None,
            notes: None,
        })
    }
}
