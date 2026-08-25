use crate::models::Result;
use hex;
use md5::{Digest, Md5};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Progress callback type for download progress updates
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Lowercase, collapse whitespace runs to one space, strip a trailing browser
/// dedup suffix like ` (1)` immediately before the extension, then trim.
fn normalise_download_name(name: &str) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(pos) => (&name[..pos], &name[pos..]),
        None => (name, ""),
    };

    // Strip trailing ` (N)` dedup suffix from stem
    let stem = {
        let trimmed = stem.trim_end();
        if let Some(without_close) = trimmed.strip_suffix(')') {
            if let Some(open_pos) = without_close.rfind('(') {
                let inner = &without_close[open_pos + 1..];
                if inner.chars().all(|c| c.is_ascii_digit())
                    && open_pos > 0
                    && without_close[..open_pos].ends_with(' ')
                {
                    without_close[..open_pos].trim_end()
                } else {
                    trimmed
                }
            } else {
                trimmed
            }
        } else {
            trimmed
        }
    };

    format!(
        "{} {}",
        stem.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase(),
        ext.to_lowercase()
    )
    .trim()
    .to_string()
}

/// True when a file in Downloads refers to the same download as `expected`.
/// Beyond normalised equality this tolerates Nexus CDN suffixes — the browser
/// often saves `Mod-6603-6-2.zip` as `Mod-6603-6-2-1778691223.zip`. Callers
/// still validate the candidate by size/hash, so a suffix match alone can't
/// pair the wrong file.
fn download_names_match(candidate: &str, expected: &str) -> bool {
    let normalised_candidate = normalise_download_name(candidate);
    let normalised_expected = normalise_download_name(expected);
    if normalised_candidate == normalised_expected {
        return true;
    }

    // Suffix tolerance: "<expected-stem>-<digits> <ext>". Both normalised
    // names are "stem ext", so compare stems and require the extra segment
    // to be dash-separated digits only.
    let (cand_stem, cand_ext) = match normalised_candidate.rsplit_once(' ') {
        Some(parts) => parts,
        None => return false,
    };
    let (exp_stem, exp_ext) = match normalised_expected.rsplit_once(' ') {
        Some(parts) => parts,
        None => return false,
    };
    if cand_ext != exp_ext || !cand_stem.starts_with(exp_stem) {
        return false;
    }
    let extra = &cand_stem[exp_stem.len()..];
    let Some(digits_part) = extra.strip_prefix('-') else {
        return false;
    };
    !digits_part.is_empty()
        && digits_part
            .split('-')
            .all(|segment| !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()))
}

/// Look for a file the user has already downloaded in their Downloads folder that
/// matches what we were about to fetch. Tries an exact name match first, then
/// falls back to a tolerant scan (normalised whitespace/case, browser dedup
/// suffixes like ` (1)`, Nexus CDN suffixes like `-1778691223`) — skipping any
/// in-progress partials.
///
/// When an MD5 is provided the candidate is validated by hash (reporting progress
/// via `on_progress`, since hashing a large archive is slow); when `expected_size`
/// is provided, validates by size; when neither is provided, accepts any non-empty
/// candidate (caller is responsible for a stability check).
pub fn find_in_downloads<F>(
    filename: &str,
    expected_size: Option<u64>,
    expected_md5: Option<&str>,
    mut on_progress: F,
) -> Option<PathBuf>
where
    F: FnMut(u64, u64),
{
    use crate::services::hasher;

    let download_dir = dirs::download_dir()?;
    let partial_exts = ["part", "crdownload", "tmp"];

    // Exact name match first, then every tolerant-scan match, so a stale
    // same-named file can't shadow a freshly completed download: each
    // candidate is validated below and the first passing one wins.
    let mut candidates: Vec<PathBuf> = Vec::new();
    let exact = download_dir.join(filename);
    if exact.is_file() {
        candidates.push(exact);
    }
    for entry in std::fs::read_dir(&download_dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| partial_exts.contains(&e))
            .unwrap_or(false)
        {
            continue;
        }
        if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
            if download_names_match(fname, filename) && !candidates.contains(&path) {
                candidates.push(path);
            }
        }
    }

    for candidate in candidates {
        let size_on_disk = std::fs::metadata(&candidate).map(|m| m.len()).ok();
        if let Some(md5) = expected_md5 {
            // Cheap reject before hashing when we already know the expected size differs.
            if let Some(size) = expected_size {
                if size > 0 && size_on_disk != Some(size) {
                    continue;
                }
            }
            if let Ok(actual) = hasher::md5_file_with_progress(&candidate, &mut on_progress) {
                if actual.eq_ignore_ascii_case(md5) {
                    return Some(candidate);
                }
            }
        } else if let Some(size) = expected_size {
            if size > 0 && size_on_disk == Some(size) {
                return Some(candidate);
            }
        } else if size_on_disk.unwrap_or(0) > 0 {
            // No MD5 or size to validate against — accept any non-empty
            // candidate (caller must run a stability check before trusting it).
            return Some(candidate);
        }
    }
    None
}

/// True when `file_name` looks like an in-progress download of `expected`:
/// carries a browser partial extension (`.part`, `.crdownload`, `.tmp`) and
/// the remainder of the name matches tolerantly (see `download_names_match`).
fn is_partial_for(file_name: &str, expected: &str) -> bool {
    const PARTIAL_EXTS: [&str; 3] = ["part", "crdownload", "tmp"];
    let Some((inner, ext)) = file_name.rsplit_once('.') else {
        return false;
    };
    if !PARTIAL_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
        return false;
    }
    download_names_match(inner, expected)
}

/// Look for an in-progress browser download in the Downloads folder that refers
/// to `filename`. Returns the path and current byte size of the largest
/// matching partial (`.part`, `.crdownload`, `.tmp`), so callers polling for a
/// finished download can tell that the transfer is still growing.
pub fn find_partial_download(filename: &str) -> Option<(PathBuf, u64)> {
    let download_dir = dirs::download_dir()?;
    let mut best: Option<(PathBuf, u64)> = None;
    for entry in std::fs::read_dir(&download_dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !is_partial_for(fname, filename) {
            continue;
        }
        let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let take = match &best {
            None => true,
            Some((_, best_len)) => len > *best_len,
        };
        if take {
            best = Some((path, len));
        }
    }
    best
}

/// Download a file from a URL to a destination path, returning its MD5 hash.
pub async fn download_file(client: &Client, url: &str, dest: &Path) -> Result<String> {
    download_file_with_progress(client, url, dest, None).await
}

/// Download a file with progress reporting, returning its MD5 hash.
pub async fn download_file_with_progress(
    client: &Client,
    url: &str,
    dest: &Path,
    progress_callback: Option<ProgressCallback>,
) -> Result<String> {
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
    let mut hasher = Md5::new();
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;

        if let Some(ref callback) = progress_callback {
            callback(downloaded, total_size);
        }
    }

    file.flush().await?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tempfile::NamedTempFile;

    #[test]
    fn normalise_handles_common_browser_variations() {
        let base = normalise_download_name("Update 1.4 Suppressed Muffled Full Overhaul.zip");
        // Double space collapses
        assert_eq!(
            normalise_download_name("Update 1.4  Suppressed Muffled Full Overhaul.zip"),
            base
        );
        // Case difference
        assert_eq!(
            normalise_download_name("update 1.4 suppressed muffled full overhaul.ZIP"),
            base
        );
        // Browser dedup suffix
        assert_eq!(
            normalise_download_name("Update 1.4 Suppressed Muffled Full Overhaul (1).zip"),
            base
        );
        // Clearly different name does NOT match
        assert_ne!(
            normalise_download_name("Something Completely Different.zip"),
            base
        );
    }

    #[test]
    fn download_names_match_tolerates_cdn_suffixes() {
        let expected = "HK416D-6603-6-2.zip";
        // Exact
        assert!(download_names_match(expected, expected));
        // Nexus CDN timestamp suffix (the reported real-world case)
        assert!(download_names_match(
            "HK416D-6603-6-2-1778691223.zip",
            expected
        ));
        // Case-insensitive
        assert!(download_names_match(
            "hk416d-6603-6-2-1778691223.ZIP",
            expected
        ));
        // Different extension does not match
        assert!(!download_names_match(
            "HK416D-6603-6-2-1778691223.rar",
            expected
        ));
        // Different name with digits appended does not match
        assert!(!download_names_match(
            "SomeOtherMod-6603-6-2-1778691223.zip",
            expected
        ));
        // Prefix must end on a dash-digit boundary - a longer mod name that
        // merely starts with the expected stem is not a suffix match
        assert!(!download_names_match("HK416D-6603-6-2-X.zip", expected));
    }

    #[test]
    fn is_partial_for_matches_browser_partials() {
        let expected = "HK416D-6603-6-2.zip";
        // Firefox and Chrome partial names for this download
        assert!(is_partial_for("HK416D-6603-6-2.zip.part", expected));
        assert!(is_partial_for(
            "HK416D-6603-6-2-1778691223.zip.crdownload",
            expected
        ));
        assert!(is_partial_for("hk416d-6603-6-2.zip.tmp", expected));
        // Completed file or unrelated name with a partial extension
        assert!(!is_partial_for("HK416D-6603-6-2.zip", expected));
        // Bare stem with partial extension but no archive extension: the
        // inner name "HK416D-6603-6-2" has no extension to match against
        assert!(!is_partial_for("HK416D-6603-6-2.part", expected));
        assert!(!is_partial_for("SomeOtherMod.zip.part", expected));
        // No extension at all
        assert!(!is_partial_for("HK416D-6603-6-2", expected));
    }

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
