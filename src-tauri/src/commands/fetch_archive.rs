use crate::models::{progress::ProgressEvent, Result};
use crate::state::AppState;
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadArchiveResult {
    /// MD5 of the archive, so callers can pass it to install and skip re-hashing.
    pub content_hash: Option<String>,
    /// True when a matching file from the user's Downloads folder was reused
    /// instead of fetching from the server.
    pub reused_local: bool,
}

#[tauri::command]
pub async fn download_mod_archive(
    app: AppHandle,
    url: String,
    filename: String,
    expected_hash: Option<String>,
    state: State<'_, AppState>,
) -> Result<DownloadArchiveResult> {
    let client = &state.client;
    let archives_root = crate::state::app_data_root()?.join("staged/archives");
    let dest = archives_root.join(&filename);

    // Re-use a matching file the user has already downloaded, validated by hash.
    let hash_app = app.clone();
    let hash_file = filename.clone();
    let hash_started = Instant::now();
    if let Some(found) = crate::services::downloader::find_in_downloads(
        &filename,
        None,
        expected_hash.as_deref(),
        |processed, total| {
            let elapsed = hash_started.elapsed().as_secs_f64().max(0.001);
            let mib_per_sec = (processed as f64 / elapsed) / (1024.0 * 1024.0);
            let _ = hash_app.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "hash".to_string(),
                    file: hash_file.clone(),
                    percent: 50.0,
                    message: format!(
                        "Checking {} in Downloads... {:.1} MiB/s",
                        hash_file, mib_per_sec
                    ),
                    total_bytes: if total > 0 { Some(total) } else { None },
                    processed_bytes: Some(processed),
                },
            );
        },
    ) {
        std::fs::create_dir_all(&archives_root)?;
        std::fs::copy(&found, &dest)?;
        let _ = app.emit(
            "install_progress",
            &ProgressEvent {
                operation: "download".to_string(),
                file: filename.clone(),
                percent: 99.0,
                message: format!("Found {} in Downloads, using local copy", filename),
                total_bytes: None,
                processed_bytes: None,
            },
        );
        // The validated file's MD5 equals the expected hash, so pass it through.
        return Ok(DownloadArchiveResult {
            content_hash: expected_hash,
            reused_local: true,
        });
    }

    let app_for_progress = app.clone();
    let filename_for_progress = filename.clone();
    let download_started = Instant::now();

    let hash = crate::services::downloader::download_file_with_progress(
        client,
        &url,
        &dest,
        Some(Box::new(move |downloaded, total| {
            let elapsed = download_started.elapsed().as_secs_f64().max(0.001);
            let mib_per_sec = (downloaded as f64 / elapsed) / (1024.0 * 1024.0);
            let percent = if total > 0 {
                (downloaded as f32 / total as f32) * 100.0
            } else {
                50.0
            };
            let _ = app_for_progress.emit(
                "install_progress",
                &ProgressEvent {
                    operation: "download".to_string(),
                    file: filename_for_progress.clone(),
                    percent: percent.min(99.0),
                    message: format!(
                        "Downloading {}... {:.1} MiB/s",
                        filename_for_progress, mib_per_sec
                    ),
                    total_bytes: if total > 0 { Some(total) } else { None },
                    processed_bytes: Some(downloaded),
                },
            );
        })),
    )
    .await?;
    Ok(DownloadArchiveResult {
        content_hash: Some(hash),
        reused_local: false,
    })
}
