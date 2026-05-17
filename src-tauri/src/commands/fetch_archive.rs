use tauri::State;
use crate::state::AppState;
use crate::models::{AppError, Result};
use std::path::PathBuf;

#[tauri::command]
pub async fn download_mod_archive(url: String, filename: String, state: State<'_, AppState>) -> Result<()> {
    let client = &state.client;
    let archives_root = crate::state::app_data_root()?.join("staged/archives");
    let dest = archives_root.join(&filename);
    log::info!("Downloading mod archive from {} to {:?}", url, dest);
    crate::services::downloader::download_file(client, &url, &dest).await?;
    Ok(())
}
