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

/// Extensions that mark an in-progress browser download.
const PARTIAL_EXTS: [&str; 3] = ["part", "crdownload", "tmp"];

/// Archive extensions the app can install. The Nexus CDN file-id alias path
/// only claims one of these - a same-id `.txt` readme is never the download.
const ARCHIVE_EXTS: [&str; 3] = ["zip", "rar", "7z"];

/// Maximum subdirectory depth below the Downloads folder walked by the
/// download finders. Depth 1 covers `~/Downloads/NexusMods`, depth 2 one more
/// level; anything deeper is not a browser download location.
const MAX_DOWNLOAD_WALK_DEPTH: u32 = 2;

/// Upper bound on directory entries visited per scan, so the 2s manual
/// download poll stays cheap even in a crowded Downloads folder.
const MAX_DOWNLOAD_WALK_ENTRIES: usize = 5000;

/// True when `ext` (already lowercased, with leading dot) is an installable
/// archive extension.
fn is_archive_extension(ext: &str) -> bool {
    ARCHIVE_EXTS.contains(&ext.trim_start_matches('.'))
}

/// True when a file in Downloads refers to the same download as `expected`.
/// Beyond normalised equality this tolerates Nexus CDN suffixes - the browser
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

/// True when `candidate` is the Nexus CDN name for `file_id`: the normalised
/// stem ends with `-{file_id}` and the file carries an installable archive
/// extension. Normalisation already strips browser dedup suffixes (` (1)`),
/// so those still hit. The dash boundary is exact - file 31053 does not match
/// `-3105` or `-310530`. Callers still validate the candidate by size, so a
/// suffix hit alone cannot pair the wrong file.
fn download_name_matches_file_id(candidate: &str, file_id: u64) -> bool {
    let normalised = normalise_download_name(candidate);
    let (stem, ext) = match normalised.rsplit_once(' ') {
        Some(parts) => parts,
        None => return false,
    };
    if !is_archive_extension(ext) {
        return false;
    }
    stem.ends_with(&format!("-{file_id}"))
}

/// Shared candidate walker for both download finders: the Downloads folder
/// plus subdirectories down to `MAX_DOWNLOAD_WALK_DEPTH`, skipping
/// dot-entries and symlinked directories (so there can be no cycle) and
/// stopping after `MAX_DOWNLOAD_WALK_ENTRIES` entries to bound poll cost.
/// Returns files only; callers filter by name.
fn walk_download_candidates(download_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack: Vec<(PathBuf, u32)> = vec![(download_dir.to_path_buf(), 0)];
    let mut visited: usize = 0;
    while let Some((dir, depth)) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_DOWNLOAD_WALK_ENTRIES {
                return files;
            }
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            // `is_dir` follows symlinks, so a symlinked directory reads as a
            // directory here and is skipped via its file type below. Symlinked
            // files still count through `is_file` further down.
            if path.is_dir() {
                if entry
                    .file_type()
                    .map(|kind| kind.is_symlink())
                    .unwrap_or(false)
                {
                    continue;
                }
                if depth < MAX_DOWNLOAD_WALK_DEPTH {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if path.is_file() {
                files.push(path);
            }
        }
    }
    files
}

/// Tiered candidate scan of `download_dir`: exact-name hits first, then Nexus
/// CDN file-id hits, then tolerant-name hits. Each tier is validated by the
/// caller (size/hash), so ordering only decides which passing file wins.
fn scan_download_candidates(
    download_dir: &Path,
    filename: &str,
    expected_file_id: Option<u64>,
) -> Vec<PathBuf> {
    fn is_partial(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| PARTIAL_EXTS.contains(&e.to_ascii_lowercase().as_str()))
            .unwrap_or(false)
    }

    let mut exact: Vec<PathBuf> = Vec::new();
    let mut by_id: Vec<PathBuf> = Vec::new();
    let mut tolerant: Vec<PathBuf> = Vec::new();
    // Fast path: the top-level exact join first (previous behaviour).
    let top_exact = download_dir.join(filename);
    if top_exact.is_file() {
        exact.push(top_exact);
    }
    for path in walk_download_candidates(download_dir) {
        if exact.contains(&path) || is_partial(&path) {
            continue;
        }
        let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if fname == filename {
            if !exact.contains(&path) {
                exact.push(path);
            }
        } else if expected_file_id.is_some_and(|id| download_name_matches_file_id(fname, id)) {
            by_id.push(path);
        } else if download_names_match(fname, filename) {
            tolerant.push(path);
        }
    }
    exact.extend(by_id);
    exact.extend(tolerant);
    exact
}

/// Look for a file the user has already downloaded in their Downloads folder that
/// matches what we were about to fetch. Scans the Downloads folder plus
/// subdirectories (see `walk_download_candidates`). Tries an exact name match
/// first, then Nexus CDN file-id aliases (`*-{file_id}.zip`), then falls back
/// to a tolerant scan (normalised whitespace/case, browser dedup
/// suffixes like ` (1)`, Nexus CDN suffixes like `-1778691223`) - skipping any
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
    expected_file_id: Option<u64>,
    on_progress: F,
) -> Option<PathBuf>
where
    F: FnMut(u64, u64),
{
    let download_dir = dirs::download_dir()?;
    find_in_downloads_in(
        &download_dir,
        filename,
        expected_size,
        expected_md5,
        expected_file_id,
        on_progress,
    )
}

/// `find_in_downloads` against an explicit root, so tests can point it at a
/// temp dir instead of the real Downloads folder.
fn find_in_downloads_in<F>(
    download_dir: &Path,
    filename: &str,
    expected_size: Option<u64>,
    expected_md5: Option<&str>,
    expected_file_id: Option<u64>,
    mut on_progress: F,
) -> Option<PathBuf>
where
    F: FnMut(u64, u64),
{
    use crate::services::hasher;

    // Exact name match first, then Nexus CDN file-id aliases, then every
    // tolerant-scan match, so a stale same-named file can't shadow a freshly
    // completed download: each candidate is validated below and the first
    // passing one wins.
    let candidates = scan_download_candidates(download_dir, filename, expected_file_id);

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
            // No MD5 or size to validate against - accept any non-empty
            // candidate (caller must run a stability check before trusting it).
            return Some(candidate);
        }
    }
    None
}

/// True when `file_name` looks like an in-progress download of `expected`:
/// carries a browser partial extension (`.part`, `.crdownload`, `.tmp`) and
/// the remainder of the name matches tolerantly (see `download_names_match`)
/// or is the Nexus CDN name for `expected_file_id`.
fn is_partial_for(file_name: &str, expected: &str, expected_file_id: Option<u64>) -> bool {
    let Some((inner, ext)) = file_name.rsplit_once('.') else {
        return false;
    };
    if !PARTIAL_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
        return false;
    }
    if download_names_match(inner, expected) {
        return true;
    }
    expected_file_id.is_some_and(|id| download_name_matches_file_id(inner, id))
}

/// Look for an in-progress browser download in the Downloads folder (plus
/// subdirectories, see `walk_download_candidates`) that refers to `filename`.
/// Returns the path and current byte size of the largest matching partial
/// (`.part`, `.crdownload`, `.tmp`), so callers polling for a finished
/// download can tell that the transfer is still growing.
pub fn find_partial_download(
    filename: &str,
    expected_file_id: Option<u64>,
) -> Option<(PathBuf, u64)> {
    let download_dir = dirs::download_dir()?;
    find_partial_download_in(&download_dir, filename, expected_file_id)
}

/// `find_partial_download` against an explicit root, so tests can point it at
/// a temp dir instead of the real Downloads folder.
fn find_partial_download_in(
    download_dir: &Path,
    filename: &str,
    expected_file_id: Option<u64>,
) -> Option<(PathBuf, u64)> {
    let mut best: Option<(PathBuf, u64)> = None;
    for path in walk_download_candidates(download_dir) {
        let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !is_partial_for(fname, filename, expected_file_id) {
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

/// Upper bound on a single archive download.
///
/// Mod archives are realistically a few hundred megabytes; the largest Ready or
/// Not mods sit well under a gigabyte. The cap stops a server (or a hostile
/// mirror) from streaming until the disk is full. `Content-Length` is checked
/// before the first byte and the running total is checked as chunks arrive,
/// because the header is a claim rather than a guarantee.
const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024; // 8 GiB

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
    if total_size > MAX_DOWNLOAD_BYTES {
        return Err(crate::models::AppError::Validation(format!(
            "archive is {total_size} bytes, over the {} byte download limit",
            MAX_DOWNLOAD_BYTES
        )));
    }

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
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > MAX_DOWNLOAD_BYTES {
            // Drop the partial file rather than leaving a truncated archive in
            // the staging dir for a later install to pick up.
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(crate::models::AppError::Validation(format!(
                "archive exceeded the {} byte download limit",
                MAX_DOWNLOAD_BYTES
            )));
        }
        file.write_all(&chunk).await?;
        hasher.update(&chunk);

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
        assert!(is_partial_for("HK416D-6603-6-2.zip.part", expected, None));
        assert!(is_partial_for(
            "HK416D-6603-6-2-1778691223.zip.crdownload",
            expected,
            None
        ));
        assert!(is_partial_for("hk416d-6603-6-2.zip.tmp", expected, None));
        // Completed file or unrelated name with a partial extension
        assert!(!is_partial_for("HK416D-6603-6-2.zip", expected, None));
        // Bare stem with partial extension but no archive extension: the
        // inner name "HK416D-6603-6-2" has no extension to match against
        assert!(!is_partial_for("HK416D-6603-6-2.part", expected, None));
        assert!(!is_partial_for("SomeOtherMod.zip.part", expected, None));
        // No extension at all
        assert!(!is_partial_for("HK416D-6603-6-2", expected, None));
    }

    #[test]
    fn is_partial_for_matches_cdn_alias_by_file_id() {
        let api_name = "RoNVoiceCommanderMod.V1.4.0 8353 1.4.0 2026-10-11T11-57Z n75BPt0Bo.zip";
        // The CDN name shares nothing with the API name except the file id.
        assert!(is_partial_for(
            "Ready Or Not Voice Commander Mod (UE4SS)-31053.zip.part",
            api_name,
            Some(31053)
        ));
        // Without the file id the same partial does not match.
        assert!(!is_partial_for(
            "Ready Or Not Voice Commander Mod (UE4SS)-31053.zip.part",
            api_name,
            None
        ));
        // A different mod's file id does not match either.
        assert!(!is_partial_for(
            "Rebel Special Forces-31008.zip.crdownload",
            api_name,
            Some(31053)
        ));
    }

    #[test]
    fn download_name_matches_file_id_hits_and_misses() {
        // Live shape from mod 8353: the API name and the CDN name share
        // nothing except the file id.
        assert!(download_name_matches_file_id(
            "Ready Or Not Voice Commander Mod (UE4SS)-31053.zip",
            31053
        ));
        // Case and whitespace insensitive, like download_names_match.
        assert!(download_name_matches_file_id(
            "ready  or not voice commander mod (ue4ss)-31053.ZIP",
            31053
        ));
        // Browser dedup suffix tolerated.
        assert!(download_name_matches_file_id(
            "Ready Or Not Voice Commander Mod (UE4SS)-31053 (1).zip",
            31053
        ));
        // The id boundary is exact: shorter and longer ids do not hit.
        assert!(!download_name_matches_file_id(
            "Ready Or Not Voice Commander Mod (UE4SS)-3105.zip",
            31053
        ));
        assert!(!download_name_matches_file_id(
            "Ready Or Not Voice Commander Mod (UE4SS)-310530.zip",
            31053
        ));
        // A different mod's file id does not hit.
        assert!(!download_name_matches_file_id(
            "Rebel Special Forces-31008.zip",
            31053
        ));
        // Wrong extension: no extension, an in-progress partial, and a
        // non-archive file never hit the completed-file matcher.
        assert!(!download_name_matches_file_id(
            "Ready Or Not Voice Commander Mod (UE4SS)-31053",
            31053
        ));
        assert!(!download_name_matches_file_id(
            "Ready Or Not Voice Commander Mod (UE4SS)-31053.zip.part",
            31053
        ));
        assert!(!download_name_matches_file_id("readme-31053.txt", 31053));
    }

    fn write_sized(path: &std::path::Path, size: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; size]).unwrap();
    }

    #[test]
    fn finders_locate_cdn_alias_in_a_subdirectory() {
        let root = tempfile::TempDir::new().unwrap();
        let api_name = "RoNVoiceCommanderMod.V1.4.0 8353 1.4.0.zip";
        let cdn_name = "Ready Or Not Voice Commander Mod (UE4SS)-31053.zip";
        let cdn_path = root.path().join("NexusMods").join(cdn_name);
        write_sized(&cdn_path, 64);

        // Completed-file finder matches the CDN alias by file id plus size.
        let found = find_in_downloads_in(
            root.path(),
            api_name,
            Some(64),
            None,
            Some(31053),
            |_, _| {},
        );
        assert_eq!(found, Some(cdn_path.clone()));
        // Without the file id the same file stays invisible.
        assert_eq!(
            find_in_downloads_in(root.path(), api_name, Some(64), None, None, |_, _| {}),
            None
        );

        // Partial finder sees an in-progress CDN alias in the same subfolder.
        let partial_path = root
            .path()
            .join("NexusMods")
            .join(format!("{cdn_name}.part"));
        write_sized(&partial_path, 32);
        let hit = find_partial_download_in(root.path(), api_name, Some(31053));
        assert!(hit.is_some());
        let (hit_path, hit_size) = hit.unwrap();
        assert_eq!(hit_path, partial_path);
        assert_eq!(hit_size, 32);
    }

    #[test]
    fn exact_name_wins_over_file_id_alias() {
        let root = tempfile::TempDir::new().unwrap();
        let api_name = "Some Mod-1234-1-0.zip";
        let exact_path = root.path().join(api_name);
        write_sized(&exact_path, 64);
        let alias_path = root
            .path()
            .join("NexusMods")
            .join("Some Mod Renamed-99999.zip");
        write_sized(&alias_path, 64);

        let found = find_in_downloads_in(
            root.path(),
            api_name,
            Some(64),
            None,
            Some(99999),
            |_, _| {},
        );
        assert_eq!(found, Some(exact_path));
    }

    #[test]
    fn download_walk_stops_at_depth_cap() {
        let api_name = "Some Mod-1234-1-0.zip";
        // Depth 1 (~/Downloads/NexusMods) and depth 2 are covered.
        for sub in ["NexusMods", "a/b"] {
            let root = tempfile::TempDir::new().unwrap();
            let path = root.path().join(sub).join("Some Mod Renamed-99999.zip");
            write_sized(&path, 64);
            assert_eq!(
                find_in_downloads_in(
                    root.path(),
                    api_name,
                    Some(64),
                    None,
                    Some(99999),
                    |_, _| {}
                ),
                Some(path),
                "expected a hit at subdir {sub}"
            );
        }
        // Depth 3 is beyond the cap, so the file stays invisible.
        let root = tempfile::TempDir::new().unwrap();
        write_sized(
            &root.path().join("a/b/c").join("Some Mod Renamed-99999.zip"),
            64,
        );
        assert_eq!(
            find_in_downloads_in(
                root.path(),
                api_name,
                Some(64),
                None,
                Some(99999),
                |_, _| {}
            ),
            None
        );
        assert_eq!(
            find_partial_download_in(root.path(), api_name, Some(99999)),
            None
        );
    }

    #[test]
    fn download_walk_skips_hidden_dirs() {
        let root = tempfile::TempDir::new().unwrap();
        let api_name = "Some Mod-1234-1-0.zip";
        write_sized(
            &root
                .path()
                .join(".hidden")
                .join("Some Mod Renamed-99999.zip"),
            64,
        );
        assert_eq!(
            find_in_downloads_in(
                root.path(),
                api_name,
                Some(64),
                None,
                Some(99999),
                |_, _| {}
            ),
            None
        );
        write_sized(
            &root
                .path()
                .join(".hidden")
                .join("Some Mod Renamed-99999.zip.part"),
            32,
        );
        assert_eq!(
            find_partial_download_in(root.path(), api_name, Some(99999)),
            None
        );
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

    /// A one-shot HTTP server that answers with a fixed `Content-Length` and no
    /// body. Keeps the size-cap tests self-contained and deterministic rather
    /// than depending on a remote host's behaviour.
    async fn serve_declared_length(length: u64) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt as _};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0u8; 1024];
                let _ = socket.read(&mut buffer).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            }
        });

        format!("http://{addr}/archive.zip")
    }

    #[tokio::test]
    async fn a_declared_size_over_the_cap_is_refused_without_writing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let dest = temp_dir.path().join("archive.zip");
        let url = serve_declared_length(MAX_DOWNLOAD_BYTES + 1).await;

        let result = download_file(&Client::new(), &url, &dest).await;

        let error = result.expect_err("an oversized archive should be refused");
        assert!(
            error.to_string().contains("download limit"),
            "unexpected error: {error}"
        );
        assert!(
            !dest.exists(),
            "a refused download must not leave a file behind"
        );
    }

    #[tokio::test]
    async fn a_size_within_the_cap_still_downloads() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let dest = temp_dir.path().join("archive.zip");
        // Small, but declared, so the check under test is the same one.
        let url = serve_declared_length(0).await;

        let result = download_file(&Client::new(), &url, &dest).await;

        assert!(result.is_ok(), "a small archive should download");
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
