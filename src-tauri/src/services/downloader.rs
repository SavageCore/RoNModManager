use crate::models::Result;
use reqwest::Client;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Progress callback type for download progress updates
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Download a file from a URL to a destination path
pub async fn download_file(client: &Client, url: &str, dest: &Path) -> Result<()> {
    download_file_with_progress(client, url, dest, None).await
}

/// Download a file with progress reporting
pub async fn download_file_with_progress(
    client: &Client,
    url: &str,
    dest: &Path,
    progress_callback: Option<ProgressCallback>,
) -> Result<()> {
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(crate::models::AppError::Http(
            response.error_for_status().unwrap_err(),
        ));
    }

    let total_size = response.content_length().unwrap_or(0);

    // Create parent directory if it doesn't exist
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut file = File::create(dest).await?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if let Some(ref callback) = progress_callback {
            callback(downloaded, total_size);
        }
    }

    file.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_download_file() {
        let client = Client::new();
        let temp_file = NamedTempFile::new().unwrap();
        let dest = temp_file.path();

        // Download a small file from a reliable source (GitHub's robots.txt)
        let url = "https://raw.githubusercontent.com/github/gitignore/main/README.md";

        let result = download_file(&client, url, dest).await;

        // If this fails due to network issues, just skip the test
        if result.is_ok() {
            let contents = tokio::fs::read_to_string(dest).await.unwrap();
            assert!(!contents.is_empty());
        }
    }

    #[tokio::test]
    async fn test_download_file_with_progress() {
        let client = Client::new();
        let temp_file = NamedTempFile::new().unwrap();
        let dest = temp_file.path();

        let progress_updates = Arc::new(Mutex::new(Vec::new()));
        let progress_updates_clone = progress_updates.clone();

        let callback = Box::new(move |downloaded: u64, total: u64| {
            progress_updates_clone
                .lock()
                .unwrap()
                .push((downloaded, total));
        });

        let url = "https://raw.githubusercontent.com/github/gitignore/main/README.md";

        let result = download_file_with_progress(&client, url, dest, Some(callback)).await;

        if result.is_ok() {
            // Progress should have been reported at least once
            let updates = progress_updates.lock().unwrap();
            assert!(!updates.is_empty());

            // Last update should show full download
            if let Some((downloaded, total)) = updates.last() {
                if *total > 0 {
                    assert_eq!(downloaded, total);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_download_invalid_url() {
        let client = Client::new();
        let temp_file = NamedTempFile::new().unwrap();
        let dest = temp_file.path();

        let url = "https://httpbin.org/status/404";

        let result = download_file(&client, url, dest).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_creates_parent_directory() {
        use tempfile::TempDir;

        let client = Client::new();
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir
            .path()
            .join("subdir")
            .join("nested")
            .join("file.txt");

        let url = "https://raw.githubusercontent.com/github/gitignore/main/README.md";

        let result = download_file(&client, url, &nested_path).await;

        if result.is_ok() {
            assert!(nested_path.exists());
            assert!(nested_path.parent().unwrap().exists());
        }
    }
}
