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

/// Restore intro movie files from .bak backups
pub fn undo_intro_skip(game_path: &Path) -> Result<()> {
    let movies_dir = get_movies_path(game_path);

    for file_name in INTRO_MOVIE_FILES {
        let mp4 = movies_dir.join(file_name);
        let bak = movies_dir.join(format!("{file_name}.bak"));

        if bak.exists() {
            fs::rename(&bak, &mp4)
                .map_err(|e| AppError::Validation(format!("failed to restore {file_name}: {e}")))?;
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
        fs::create_dir_all(parent).map_err(|e| AppError::Validation(e.to_string()))?;
    }
    let backup = backup_path(&ini);
    if !backup.exists() && ini.exists() {
        fs::copy(&ini, &backup).map_err(|e| AppError::Validation(format!("backup failed: {e}")))?;
    }
    let content = get_profile_content(profile)?;
    let tmp = ini.with_extension("ini.tmp");
    fs::write(&tmp, &content).map_err(|e| AppError::Validation(e.to_string()))?;
    fs::rename(&tmp, &ini).map_err(|e| AppError::Validation(e.to_string()))?;
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

pub fn restore_optimization() -> Result<()> {
    let ini = get_engine_ini_path()?;
    let backup = backup_path(&ini);
    if backup.exists() {
        fs::rename(&backup, &ini).map_err(|e| AppError::Validation(e.to_string()))?;
    } else if ini.exists() {
        fs::remove_file(&ini).map_err(|e| AppError::Validation(e.to_string()))?;
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
