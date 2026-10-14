use crate::models::{progress::ProgressEvent, Result};
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

/// Resolve where a download for `filename` is staged.
///
/// `filename` is a modpack key or a mod API field, both of which an untrusted
/// party controls, and it decides where the download is written. `Path::join`
/// alone is not containment: an absolute name discards the staging root and
/// `..` walks out of it, so the destination goes through the shared guard.
fn staged_destination(archives_root: &Path, filename: &str) -> Result<PathBuf> {
    crate::services::path_safety::safe_join(archives_root, filename)
}

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
    let dest = staged_destination(&archives_root, &filename)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/home/user/.config/app/staged/archives")
    }

    /// The archive name decides where the download lands, and it arrives from a
    /// modpack key or a mod API response - both attacker-controlled, since a
    /// shared modpack can come from a URL or a `ronmm://modpack/...` link.
    #[test]
    fn an_archive_name_cannot_write_outside_the_staging_root() {
        for name in [
            "../../../../evil.zip",
            "../../home/user/.bashrc",
            "/tmp/evil.zip",
            r"..\..\..\evil.zip",
            r"C:\Windows\evil.zip",
            "",
            ".",
            "..",
        ] {
            assert!(
                staged_destination(&root(), name).is_err(),
                "expected {name:?} to be refused as a download destination"
            );
        }
    }

    #[test]
    fn an_ordinary_archive_name_stages_where_it_should() {
        assert_eq!(
            staged_destination(&root(), "Cool Mod-1.2.zip").unwrap(),
            root().join("Cool Mod-1.2.zip")
        );
        // A relative path stays a relative path under the root rather than
        // escaping it.
        assert_eq!(
            staged_destination(&root(), "sub/Cool Mod.zip").unwrap(),
            root().join("sub").join("Cool Mod.zip")
        );
    }
}
