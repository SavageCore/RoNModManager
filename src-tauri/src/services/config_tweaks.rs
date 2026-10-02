use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{AppError, Result};

const INTRO_MOVIE_FILES: &[&str] = &["ReadyOrNot_StartupMovie.mp4", "RoNLogo.mp4"];
const ENGINE_BACKUP_SUFFIX: &str = ".ronmm.bak";

fn get_movies_path(game_path: &Path) -> PathBuf {
    game_path.join("ReadyOrNot").join("Content").join("Movies")
}

/// Apply intro skip by renaming movie files to .bak
pub fn apply_intro_skip(game_path: &Path) -> Result<()> {
    let movies_dir = get_movies_path(game_path);

    for file_name in INTRO_MOVIE_FILES {
        let mp4 = movies_dir.join(file_name);
        let bak = movies_dir.join(format!("{file_name}.bak"));

        if mp4.exists() {
            if bak.exists() {
                // Game update restored the file; we already have the backup, discard the new copy
                fs::remove_file(&mp4).map_err(|e| {
                    AppError::Validation(format!("failed to remove {file_name}: {e}"))
                })?;
            } else {
                fs::rename(&mp4, &bak).map_err(|e| {
                    AppError::Validation(format!("failed to rename {file_name}: {e}"))
                })?;
            }
        }
    }

    Ok(())
}

/// Restore intro movie files from .bak backups. Clean at rest: after undo no
/// `.bak` files remain, and calling undo twice is a no-op. If both the movie
/// and its backup exist (game update restored the file while skip was active),
/// the live file is already stock so just drop the stale backup instead of
/// renaming over it (cross-platform deterministic).
pub fn undo_intro_skip(game_path: &Path) -> Result<()> {
    let movies_dir = get_movies_path(game_path);

    for file_name in INTRO_MOVIE_FILES {
        let mp4 = movies_dir.join(file_name);
        let bak = movies_dir.join(format!("{file_name}.bak"));

        if bak.exists() {
            if mp4.exists() {
                fs::remove_file(&bak).map_err(|e| {
                    AppError::Validation(format!("failed to remove {file_name}.bak: {e}"))
                })?;
            } else {
                fs::rename(&bak, &mp4).map_err(|e| {
                    AppError::Validation(format!("failed to restore {file_name}: {e}"))
                })?;
            }
        }
    }

    Ok(())
}

/// Returns true if any intro movie backup exists (intro skip was enabled by the user)
pub fn is_intro_skip_applied(game_path: &Path) -> Result<bool> {
    let movies_dir = get_movies_path(game_path);
    Ok(INTRO_MOVIE_FILES
        .iter()
        .any(|file_name| movies_dir.join(format!("{file_name}.bak")).exists()))
}

// --- Engine.ini optimization ---

fn get_engine_ini_path() -> Result<PathBuf> {
    Ok(crate::services::steam::get_config_path()?.join("Engine.ini"))
}

fn backup_path(ini: &Path) -> PathBuf {
    PathBuf::from(format!("{}{}", ini.display(), ENGINE_BACKUP_SUFFIX))
}

/// Whether the file carries the Windows read-only attribute. Users commonly
/// mark `Engine.ini` read-only (the UE5 optimization pack asks for it and the
/// game would otherwise rewrite the tune), so every write has to cope with it.
#[cfg(windows)]
fn is_readonly(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.permissions().readonly())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_readonly(_path: &Path) -> bool {
    false
}

/// Set or clear the read-only attribute. A missing file is not an error: the
/// callers only care about protecting the user's file, not about its existence.
#[cfg(windows)]
fn set_readonly(path: &Path, readonly: bool) -> std::io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    let mut perms = metadata.permissions();
    if perms.readonly() != readonly {
        perms.set_readonly(readonly);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_readonly(_path: &Path, _readonly: bool) -> std::io::Result<()> {
    Ok(())
}

/// Rename `src` over `dst`, clearing `dst`'s read-only bit first. On Windows
/// `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING` refuses a read-only
/// destination and fails with `ERROR_ACCESS_DENIED` ("Access is denied.
/// (os error 5)"). The moved file keeps its own attributes, so renaming a
/// read-only backup back into place restores the user's original protection
/// without tracking it anywhere.
fn rename_over(src: &Path, dst: &Path) -> std::io::Result<()> {
    set_readonly(dst, false)?;
    fs::rename(src, dst)
}

/// Available optimization profiles (display name -> file stem)
pub fn available_gpu_profiles() -> Vec<String> {
    vec![
        "GTX_1050_Ti__4_GB_",
        "GTX_1060_6GB",
        "GTX_1650",
        "GTX_1650_Super",
        "GTX_1660",
        "GTX_1660_Super",
        "GTX_970",
        "GTX_980",
        "RTX_2060-2060_Super",
        "RTX_2070-_2070_Super",
        "RTX_3050",
        "RTX_3050_Ti",
        "RTX_3060-3060_Ti",
        "RTX_3070-3070_Ti",
        "RTX_3080-3080_Ti",
        "RTX_3090-3090_Ti",
        "RTX_4060",
        "RTX_4060_Ti",
        "RTX_4070",
        "RTX_4070_Ti-4070_Super",
        "RTX_4080-4080_Super",
        "RTX_4090",
        "RX_5500_XT_8GB",
        "RX_550__2_GB_",
        "RX_5600_XT",
        "RX_560__4_GB_",
        "RX_5700-5700_XT",
        "RX_570__4_GB_",
        "RX_580_4_GB_version",
        "RX_580_8GB",
        "RX_590",
        "RX_6500_XT",
        "RX_6600-6600_XT",
        "RX_6600_M",
        "RX_6700_XT",
        "RX_6750_XT",
        "RX_6800",
        "RX_6800_XT",
        "RX_6900_XT",
        "RX_6950_XT",
        "RX_7900_XT",
        "RX_7900_XTX",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn profile_bytes(profile: &str) -> Option<&'static [u8]> {
    match profile {
        "GTX_1050_Ti__4_GB_" => Some(include_bytes!(
            "../../assets/optimization/GTX_1050_Ti__4_GB_.ini"
        )),
        "GTX_1060_6GB" => Some(include_bytes!("../../assets/optimization/GTX_1060_6GB.ini")),
        "GTX_1650" => Some(include_bytes!("../../assets/optimization/GTX_1650.ini")),
        "GTX_1650_Super" => Some(include_bytes!(
            "../../assets/optimization/GTX_1650_Super.ini"
        )),
        "GTX_1660" => Some(include_bytes!("../../assets/optimization/GTX_1660.ini")),
        "GTX_1660_Super" => Some(include_bytes!(
            "../../assets/optimization/GTX_1660_Super.ini"
        )),
        "GTX_970" => Some(include_bytes!("../../assets/optimization/GTX_970.ini")),
        "GTX_980" => Some(include_bytes!("../../assets/optimization/GTX_980.ini")),
        "RTX_2060-2060_Super" => Some(include_bytes!(
            "../../assets/optimization/RTX_2060-2060_Super.ini"
        )),
        "RTX_2070-_2070_Super" => Some(include_bytes!(
            "../../assets/optimization/RTX_2070-_2070_Super.ini"
        )),
        "RTX_3050" => Some(include_bytes!("../../assets/optimization/RTX_3050.ini")),
        "RTX_3050_Ti" => Some(include_bytes!("../../assets/optimization/RTX_3050_Ti.ini")),
        "RTX_3060-3060_Ti" => Some(include_bytes!(
            "../../assets/optimization/RTX_3060-3060_Ti.ini"
        )),
        "RTX_3070-3070_Ti" => Some(include_bytes!(
            "../../assets/optimization/RTX_3070-3070_Ti.ini"
        )),
        "RTX_3080-3080_Ti" => Some(include_bytes!(
            "../../assets/optimization/RTX_3080-3080_Ti.ini"
        )),
        "RTX_3090-3090_Ti" => Some(include_bytes!(
            "../../assets/optimization/RTX_3090-3090_Ti.ini"
        )),
        "RTX_4060" => Some(include_bytes!("../../assets/optimization/RTX_4060.ini")),
        "RTX_4060_Ti" => Some(include_bytes!("../../assets/optimization/RTX_4060_Ti.ini")),
        "RTX_4070" => Some(include_bytes!("../../assets/optimization/RTX_4070.ini")),
        "RTX_4070_Ti-4070_Super" => Some(include_bytes!(
            "../../assets/optimization/RTX_4070_Ti-4070_Super.ini"
        )),
        "RTX_4080-4080_Super" => Some(include_bytes!(
            "../../assets/optimization/RTX_4080-4080_Super.ini"
        )),
        "RTX_4090" => Some(include_bytes!("../../assets/optimization/RTX_4090.ini")),
        "RX_5500_XT_8GB" => Some(include_bytes!(
            "../../assets/optimization/RX_5500_XT_8GB.ini"
        )),
        "RX_550__2_GB_" => Some(include_bytes!(
            "../../assets/optimization/RX_550__2_GB_.ini"
        )),
        "RX_5600_XT" => Some(include_bytes!("../../assets/optimization/RX_5600_XT.ini")),
        "RX_560__4_GB_" => Some(include_bytes!(
            "../../assets/optimization/RX_560__4_GB_.ini"
        )),
        "RX_5700-5700_XT" => Some(include_bytes!(
            "../../assets/optimization/RX_5700-5700_XT.ini"
        )),
        "RX_570__4_GB_" => Some(include_bytes!(
            "../../assets/optimization/RX_570__4_GB_.ini"
        )),
        "RX_580_4_GB_version" => Some(include_bytes!(
            "../../assets/optimization/RX_580_4_GB_version.ini"
        )),
        "RX_580_8GB" => Some(include_bytes!("../../assets/optimization/RX_580_8GB.ini")),
        "RX_590" => Some(include_bytes!("../../assets/optimization/RX_590.ini")),
        "RX_6500_XT" => Some(include_bytes!("../../assets/optimization/RX_6500_XT.ini")),
        "RX_6600-6600_XT" => Some(include_bytes!(
            "../../assets/optimization/RX_6600-6600_XT.ini"
        )),
        "RX_6600_M" => Some(include_bytes!("../../assets/optimization/RX_6600_M.ini")),
        "RX_6700_XT" => Some(include_bytes!("../../assets/optimization/RX_6700_XT.ini")),
        "RX_6750_XT" => Some(include_bytes!("../../assets/optimization/RX_6750_XT.ini")),
        "RX_6800" => Some(include_bytes!("../../assets/optimization/RX_6800.ini")),
        "RX_6800_XT" => Some(include_bytes!("../../assets/optimization/RX_6800_XT.ini")),
        "RX_6900_XT" => Some(include_bytes!("../../assets/optimization/RX_6900_XT.ini")),
        "RX_6950_XT" => Some(include_bytes!("../../assets/optimization/RX_6950_XT.ini")),
        "RX_7900_XT" => Some(include_bytes!("../../assets/optimization/RX_7900_XT.ini")),
        "RX_7900_XTX" => Some(include_bytes!("../../assets/optimization/RX_7900_XTX.ini")),
        _ => None,
    }
}

pub fn get_profile_content(profile: &str) -> Result<Vec<u8>> {
    if let Some(b) = profile_bytes(profile) {
        return Ok(b.to_vec());
    }
    Err(AppError::Validation(format!(
        "unknown GPU profile: {profile}"
    )))
}

pub fn apply_optimization(profile: &str) -> Result<()> {
    let ini = get_engine_ini_path()?;
    if let Some(parent) = ini.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::Validation(format!("could not create {}: {e}", parent.display()))
        })?;
    }
    // Resolve the profile before touching disk so a bad name can't leave a
    // half-committed folder behind.
    let content = get_profile_content(profile)?;
    // Capture the user's read-only protection up front; we clear it to write and
    // put it back on the profile so the game can't overwrite the tune.
    let was_readonly = is_readonly(&ini);

    let backup = backup_path(&ini);
    let made_backup = !backup.exists() && ini.exists();
    if made_backup {
        fs::copy(&ini, &backup).map_err(|e| {
            AppError::Validation(format!("backup failed for {}: {e}", ini.display()))
        })?;
    }

    let tmp = ini.with_extension("ini.tmp");
    let write = fs::write(&tmp, &content).and_then(|()| rename_over(&tmp, &ini));
    if let Err(e) = write {
        // Never leave a stray .tmp, and never leave a backup we created with
        // nothing applied over it - a later Restore would then clobber the
        // user's ini with a folder that looks optimized but isn't.
        let _ = fs::remove_file(&tmp);
        let _ = set_readonly(&ini, was_readonly);
        if made_backup {
            let _ = fs::remove_file(&backup);
        }
        return Err(AppError::Validation(format!(
            "could not write {}: {e}",
            ini.display()
        )));
    }
    // The rename dropped the original's attributes along with it; re-assert the
    // read-only protection the user had on their own Engine.ini.
    if was_readonly {
        let _ = set_readonly(&ini, true);
    }
    Ok(())
}

fn normalize_bytes(b: &[u8]) -> Vec<u8> {
    // Normalize CRLF -> LF for comparison
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' && i + 1 < b.len() && b[i + 1] == b'\n' {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

pub fn detect_applied_profile() -> Option<String> {
    let ini = get_engine_ini_path().ok()?;
    let data = fs::read(&ini).ok()?;
    let normalized = normalize_bytes(&data);
    for p in available_gpu_profiles() {
        if let Some(b) = profile_bytes(&p) {
            if normalize_bytes(b) == normalized {
                return Some(p);
            }
        }
    }
    None
}

/// Restore the pre-optimization `Engine.ini`. Clean at rest: after restore no
/// `.ronmm.bak` remains and the folder is stock (original content or no file).
/// Idempotent: a second call is a no-op and never deletes the user's original.
/// When no backup exists, only a file matching one of our own bundled profiles
/// is removed (apply created it from scratch); anything else is left alone.
pub fn restore_optimization() -> Result<()> {
    let ini = get_engine_ini_path()?;
    let backup = backup_path(&ini);
    if backup.exists() {
        // The backup carries the original's attributes (fs::copy copies them on
        // Windows), so the restored ini comes back with the user's read-only
        // protection intact.
        rename_over(&backup, &ini).map_err(|e| {
            AppError::Validation(format!("could not restore {}: {e}", ini.display()))
        })?;
    } else if detect_applied_profile().is_some() {
        // `fs::remove_file` also refuses a read-only file on Windows, and the
        // user may have re-protected the applied ini after the fact.
        let _ = set_readonly(&ini, false);
        fs::remove_file(&ini).map_err(|e| {
            AppError::Validation(format!("could not remove {}: {e}", ini.display()))
        })?;
    }
    Ok(())
}

fn normalize_gpu_name(s: &str) -> String {
    s.to_lowercase().replace(['_', '-'], " ")
}

fn gpu_string() -> Option<String> {
    // Prefer nvidia-smi / glxinfo which give exact model
    for cmd in [
        (
            &["nvidia-smi", "--query-gpu=name", "--format=csv,noheader"][..],
            None,
        ),
        (&["glxinfo"][..], Some("renderer")),
    ] {
        if let Ok(out) = std::process::Command::new(cmd.0[0])
            .args(&cmd.0[1..])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            let relevant = if let Some(needle) = cmd.1 {
                text.lines()
                    .find(|l| l.contains(needle))
                    .unwrap_or(&text)
                    .to_string()
            } else {
                text.to_string()
            };
            if relevant.contains("rtx")
                || relevant.contains("gtx")
                || relevant.contains(" radeon")
                || relevant.contains(" rx ")
            {
                return Some(relevant);
            }
        }
    }
    std::process::Command::new("lspci").output().ok().map(|o| {
        String::from_utf8_lossy(&o.stdout)
            .to_lowercase()
            .to_string()
    })
}

pub fn detect_gpu() -> Option<String> {
    let s = gpu_string()?;
    // Check most specific profiles first (longest name) to avoid "super" false match
    let mut profiles = available_gpu_profiles();
    profiles.sort_by_key(|p| std::cmp::Reverse(p.len()));
    // explicit combined variant: the asset is RTX_4080-4080_Super covering both
    if s.contains("4080 super") || s.contains("4080") {
        return Some("RTX_4080-4080_Super".to_string());
    }
    if s.contains("4090") {
        return Some("RTX_4090".to_string());
    }
    for p in profiles {
        let needle = normalize_gpu_name(&p);
        // require at least "rtx 4070" etc, not just "super"
        if s.contains(&needle) {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn fake_game_path() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("ReadyOrNot/Content/Movies")).unwrap();
        dir
    }

    fn movies_dir(game_path: &Path) -> PathBuf {
        game_path.join("ReadyOrNot/Content/Movies")
    }

    #[test]
    fn test_intro_skip_round_trip() {
        let game = fake_game_path();
        let movies = movies_dir(game.path());
        fs::write(movies.join("ReadyOrNot_StartupMovie.mp4"), b"a").unwrap();
        fs::write(movies.join("RoNLogo.mp4"), b"b").unwrap();

        assert!(!is_intro_skip_applied(game.path()).unwrap());
        apply_intro_skip(game.path()).unwrap();
        assert!(is_intro_skip_applied(game.path()).unwrap());
        assert!(!movies.join("ReadyOrNot_StartupMovie.mp4").exists());
        assert!(movies.join("ReadyOrNot_StartupMovie.mp4.bak").exists());

        undo_intro_skip(game.path()).unwrap();
        assert!(!is_intro_skip_applied(game.path()).unwrap());
        assert!(movies.join("ReadyOrNot_StartupMovie.mp4").exists());
        assert!(!movies.join("ReadyOrNot_StartupMovie.mp4.bak").exists());
    }

    #[test]
    fn test_intro_skip_is_noop_without_movies() {
        let game = TempDir::new().unwrap();
        apply_intro_skip(game.path()).unwrap();
        undo_intro_skip(game.path()).unwrap();
        assert!(!is_intro_skip_applied(game.path()).unwrap());
    }

    #[test]
    fn test_apply_intro_skip_discards_restored_copy_when_backup_exists() {
        let game = fake_game_path();
        let movies = movies_dir(game.path());
        // Game update restored the mp4 while our backup is still around.
        fs::write(movies.join("ReadyOrNot_StartupMovie.mp4"), b"new").unwrap();
        fs::write(movies.join("ReadyOrNot_StartupMovie.mp4.bak"), b"old").unwrap();

        apply_intro_skip(game.path()).unwrap();
        assert!(!movies.join("ReadyOrNot_StartupMovie.mp4").exists());
        assert_eq!(
            fs::read(movies.join("ReadyOrNot_StartupMovie.mp4.bak")).unwrap(),
            b"old"
        );
    }

    #[test]
    fn test_undo_intro_skip_is_idempotent_and_clean_at_rest() {
        let game = fake_game_path();
        let movies = movies_dir(game.path());
        fs::write(movies.join("ReadyOrNot_StartupMovie.mp4"), b"a").unwrap();
        fs::write(movies.join("RoNLogo.mp4"), b"b").unwrap();

        apply_intro_skip(game.path()).unwrap();
        undo_intro_skip(game.path()).unwrap();
        // Second undo is a no-op: movies stay, no .bak residue.
        undo_intro_skip(game.path()).unwrap();
        assert!(movies.join("ReadyOrNot_StartupMovie.mp4").exists());
        assert!(movies.join("RoNLogo.mp4").exists());
        assert!(!movies.join("ReadyOrNot_StartupMovie.mp4.bak").exists());
        assert!(!movies.join("RoNLogo.mp4.bak").exists());
        assert!(!is_intro_skip_applied(game.path()).unwrap());
    }

    #[test]
    fn test_undo_intro_skip_drops_stale_backup_when_movie_present() {
        let game = fake_game_path();
        let movies = movies_dir(game.path());
        // Game update restored the mp4 while our backup is still around;
        // undo means "stock at rest", so keep the live file and drop the bak.
        fs::write(movies.join("ReadyOrNot_StartupMovie.mp4"), b"new").unwrap();
        fs::write(movies.join("ReadyOrNot_StartupMovie.mp4.bak"), b"old").unwrap();

        undo_intro_skip(game.path()).unwrap();
        assert_eq!(
            fs::read(movies.join("ReadyOrNot_StartupMovie.mp4")).unwrap(),
            b"new"
        );
        assert!(!movies.join("ReadyOrNot_StartupMovie.mp4.bak").exists());
    }

    #[test]
    fn test_backup_path_appends_suffix() {
        let ini = Path::new("/some/dir/Engine.ini");
        assert_eq!(
            backup_path(ini),
            PathBuf::from("/some/dir/Engine.ini.ronmm.bak")
        );
    }

    #[test]
    fn test_available_profiles_all_have_content() {
        let profiles = available_gpu_profiles();
        assert!(!profiles.is_empty());
        assert!(profiles.contains(&"RX_7900_XTX".to_string()));
        for profile in &profiles {
            assert!(
                !get_profile_content(profile).unwrap().is_empty(),
                "profile {profile} has no content"
            );
        }
        assert!(get_profile_content("NOT_A_GPU").is_err());
    }

    #[test]
    fn test_normalize_bytes_converts_crlf() {
        assert_eq!(normalize_bytes(b"a\r\nb\r\n"), b"a\nb\n");
        assert_eq!(normalize_bytes(b"a\nb"), b"a\nb");
        assert_eq!(normalize_bytes(b"a\rb"), b"a\rb");
    }

    #[test]
    fn test_normalize_gpu_name() {
        assert_eq!(normalize_gpu_name("RTX_4070-Ti"), "rtx 4070 ti");
        assert_eq!(normalize_gpu_name("GTX970"), "gtx970");
    }

    /// The shared config tree's `Engine.ini` paths, cleared on construction and
    /// on drop. Optimization writes to the platform config dir (`LOCALAPPDATA`
    /// on Windows, `HOME` on Linux), and `commands::config` asserts on the very
    /// same files, so every test here takes `shared_tree_guard()` and leaves
    /// nothing behind - including the read-only flag, which would make their
    /// `remove_file` + `write` fail.
    struct EngineIni {
        ini: PathBuf,
        backup: PathBuf,
        tmp: PathBuf,
    }

    impl EngineIni {
        fn reset() -> Self {
            use crate::test_support::isolated_root;
            // Must precede the path lookup: it is what pins the config dir.
            isolated_root();
            let ini = get_engine_ini_path().unwrap();
            let this = EngineIni {
                backup: backup_path(&ini),
                tmp: ini.with_extension("ini.tmp"),
                ini,
            };
            this.wipe();
            this
        }

        fn wipe(&self) {
            for path in [&self.ini, &self.backup, &self.tmp] {
                let _ = set_readonly(path, false);
                let _ = fs::remove_file(path);
                let _ = fs::remove_dir_all(path);
            }
        }

        fn ini(&self) -> &Path {
            &self.ini
        }

        fn backup(&self) -> &Path {
            &self.backup
        }

        fn tmp(&self) -> &Path {
            &self.tmp
        }
    }

    impl Drop for EngineIni {
        fn drop(&mut self) {
            self.wipe();
        }
    }

    fn first_profile() -> String {
        available_gpu_profiles().first().cloned().unwrap()
    }

    fn write_original(ini: &Path) {
        fs::create_dir_all(ini.parent().unwrap()).unwrap();
        fs::write(ini, b"[Original]\nkey=orig\n").unwrap();
    }

    #[test]
    fn apply_then_restore_round_trips_a_writable_ini() {
        use crate::test_support::shared_tree_guard;
        let _guard = shared_tree_guard();
        let files = EngineIni::reset();
        write_original(files.ini());
        let profile = first_profile();

        apply_optimization(&profile).unwrap();
        assert!(files.backup().exists());
        assert!(!files.tmp().exists());
        assert_eq!(detect_applied_profile().as_deref(), Some(profile.as_str()));

        restore_optimization().unwrap();
        assert_eq!(fs::read(files.ini()).unwrap(), b"[Original]\nkey=orig\n");
        assert!(!files.backup().exists());
        assert!(!files.tmp().exists());
    }

    /// The reported bug: `Engine.ini` marked read-only made the replace-rename
    /// fail with `Access is denied. (os error 5)`, leaving a stray `.tmp` and a
    /// backup behind. Apply and Restore must both work over a read-only file.
    #[test]
    fn apply_and_restore_work_over_a_readonly_ini() {
        use crate::test_support::shared_tree_guard;
        let _guard = shared_tree_guard();
        let files = EngineIni::reset();
        write_original(files.ini());
        set_readonly(files.ini(), true).unwrap();
        assert!(is_readonly(files.ini()));
        let profile = first_profile();

        apply_optimization(&profile).unwrap();
        assert_eq!(detect_applied_profile().as_deref(), Some(profile.as_str()));
        // The user's protection carries over to the applied profile, so the
        // game can't overwrite the tune on exit.
        assert!(is_readonly(files.ini()));
        assert!(files.backup().exists());
        assert!(!files.tmp().exists());

        restore_optimization().unwrap();
        assert_eq!(fs::read(files.ini()).unwrap(), b"[Original]\nkey=orig\n");
        // `fs::copy` carried the attribute onto the backup, so restoring brings
        // the read-only flag back with the original content.
        assert!(is_readonly(files.ini()));
        assert!(!files.backup().exists());
    }

    /// Re-applying over a read-only file left in place by a previous apply has
    /// to work too - the user switching GPU does not first press Restore.
    #[test]
    fn reapplying_a_second_profile_over_a_readonly_ini_succeeds() {
        use crate::test_support::shared_tree_guard;
        let _guard = shared_tree_guard();
        let files = EngineIni::reset();
        write_original(files.ini());
        set_readonly(files.ini(), true).unwrap();
        let profiles = available_gpu_profiles();
        let second = profiles[1].clone();

        apply_optimization(&profiles[0]).unwrap();
        apply_optimization(&second).unwrap();

        assert_eq!(detect_applied_profile().as_deref(), Some(second.as_str()));
        assert!(is_readonly(files.ini()));
        // Only the first apply backs up: the backup is the user's original.
        assert_eq!(fs::read(files.backup()).unwrap(), b"[Original]\nkey=orig\n");
        assert!(!files.tmp().exists());
    }

    /// A rename that fails after the temp file is written must not leave that
    /// temp file in the game's config folder, and must not disturb the backup a
    /// previous apply already owns. A directory parked on the ini path fails the
    /// replace-rename the same way a locked file would.
    #[test]
    fn a_failed_apply_removes_the_tmp_it_wrote() {
        use crate::test_support::shared_tree_guard;
        let _guard = shared_tree_guard();
        let files = EngineIni::reset();
        write_original(files.ini());
        // A backup already exists, so this apply does not own it.
        fs::write(files.backup(), b"[Original]\nkey=orig\n").unwrap();
        fs::remove_file(files.ini()).unwrap();
        fs::create_dir(files.ini()).unwrap();

        let err = apply_optimization(&first_profile()).unwrap_err();
        // The message names the file it could not write instead of surfacing a
        // bare "Access is denied. (os error 5)".
        assert!(
            err.to_string().contains(&files.ini().display().to_string()),
            "error should name the ini path, got: {err}"
        );
        assert!(!files.tmp().exists());
        assert_eq!(fs::read(files.backup()).unwrap(), b"[Original]\nkey=orig\n");
    }

    /// A backup created by an apply that then failed must be rolled back: left
    /// behind it would make a later Restore clobber the user's ini with a folder
    /// that only looks optimized.
    #[test]
    fn a_failed_apply_does_not_orphan_the_backup_it_created() {
        use crate::test_support::shared_tree_guard;
        let _guard = shared_tree_guard();
        let files = EngineIni::reset();
        write_original(files.ini());
        // A directory at the ini path makes the backup copy fail, so this apply
        // created the backup and then died.
        fs::remove_file(files.ini()).unwrap();
        fs::create_dir(files.ini()).unwrap();

        let err = apply_optimization(&first_profile()).unwrap_err();
        assert!(
            err.to_string().contains(&files.ini().display().to_string()),
            "error should name the ini path, got: {err}"
        );
        assert!(!files.backup().exists());
        assert!(!files.tmp().exists());
    }

    /// An unknown profile is rejected before anything is written, so a typo
    /// can't cost the user their original ini.
    #[test]
    fn an_unknown_profile_touches_nothing() {
        use crate::test_support::shared_tree_guard;
        let _guard = shared_tree_guard();
        let files = EngineIni::reset();
        write_original(files.ini());
        set_readonly(files.ini(), true).unwrap();

        assert!(apply_optimization("Voodoo_3dfx").is_err());
        assert_eq!(fs::read(files.ini()).unwrap(), b"[Original]\nkey=orig\n");
        assert!(!files.backup().exists());
        assert!(!files.tmp().exists());
    }

    #[test]
    fn rename_over_replaces_a_readonly_destination() {
        use crate::test_support::shared_tree_guard;
        let _guard = shared_tree_guard();
        let dir = crate::test_support::scratch_dir("rename-over");
        let dst = dir.path().join("dst.ini");
        let src = dir.path().join("src.ini");
        fs::write(&src, b"new").unwrap();
        fs::write(&dst, b"old").unwrap();
        set_readonly(&dst, true).unwrap();

        rename_over(&src, &dst).unwrap();

        assert_eq!(fs::read(&dst).unwrap(), b"new");
        assert!(!src.exists());
        // Renaming carries the source's own attributes, not the cleared ones.
        assert!(!is_readonly(&dst));
    }
}
