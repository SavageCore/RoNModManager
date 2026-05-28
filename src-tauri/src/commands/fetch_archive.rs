use crate::models::{progress::ProgressEvent, Result};
use crate::state::AppState;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn download_mod_archive(
    app: AppHandle,
    url: String,
    filename: String,
    state: State<'_, AppState>,
) -> Result<()> {
    let client = &state.client;
    let archives_root = crate::state::app_data_root()?.join("staged/archives");
    let dest = archives_root.join(&filename);

    let app_for_progress = app.clone();
    let filename_for_progress = filename.clone();
    let download_started = Instant::now();

    let _ = crate::services::downloader::download_file_with_progress(
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
    Ok(())
}
