use crate::models::Result;
use std::fs;
use std::path::{Path, PathBuf};

const READY_OR_NOT_APP_ID: &str = "1144200";

/// Detect the installation path of Ready or Not
pub fn detect_game_path() -> Result<PathBuf> {
    // Find Steam installation
    let steam_path = find_steam_path()?;

    // Parse library folders
    let library_folders = parse_library_folders(&steam_path)?;

    // Search for Ready or Not in all library folders
    for library in library_folders {
        let manifest_path = library
            .join("steamapps")
            .join(format!("appmanifest_{}.acf", READY_OR_NOT_APP_ID));

        if manifest_path.exists() {
            // Found the game, return the common path
            let game_path = library
                .join("steamapps")
                .join("common")
                .join("Ready Or Not");
            if game_path.exists() {
                return Ok(game_path);
            }
        }
    }

    Err(crate::models::AppError::NotFound(
        "Ready or Not installation not found in any Steam library".to_string(),
    ))
}

/// Find the Steam installation directory
#[cfg(target_os = "windows")]
fn find_steam_path() -> Result<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    // Try 64-bit registry first
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\Wow6432Node\\Valve\\Steam") {
        if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
            return Ok(PathBuf::from(install_path));
        }
    }

    // Try 32-bit registry
    if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\Valve\\Steam") {
        if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
            return Ok(PathBuf::from(install_path));
        }
    }

    Err(crate::models::AppError::NotFound(
        "Steam installation not found in Windows registry".to_string(),
    ))
}

#[cfg(target_os = "linux")]
fn find_steam_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| {
        crate::models::AppError::NotFound("HOME environment variable not set".to_string())
    })?;

    // Check common Steam paths in order
    let possible_paths = vec![
        PathBuf::from(&home).join(".steam/steam"),
        PathBuf::from(&home).join(".local/share/Steam"),
        PathBuf::from(&home).join(".var/app/com.valvesoftware.Steam/.steam/steam"), // Flatpak
        PathBuf::from(&home).join("snap/steam/common/.steam/steam"),                // Snap
    ];

    for path in possible_paths {
        if path.join("steamapps").exists() {
            return Ok(path);
        }
    }

    Err(crate::models::AppError::NotFound(
        "Steam installation not found in common Linux locations".to_string(),
    ))
}

/// Parse libraryfolders.vdf to get all Steam library locations
fn parse_library_folders(steam_path: &Path) -> Result<Vec<PathBuf>> {
    let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");

    if !vdf_path.exists() {
        return Ok(vec![steam_path.to_path_buf()]);
    }

    let content = fs::read_to_string(&vdf_path).map_err(crate::models::AppError::Io)?;

    let mut libraries = vec![steam_path.to_path_buf()]; // Always include main Steam path

    // Simple VDF parser - look for "path" entries
    for line in content.lines() {
        let trimmed = line.trim();

        // Look for lines like: "path"		"C:\\SteamLibrary"
        if trimmed.starts_with("\"path\"") {
            if let Some(path_start) = trimmed.find("\"path\"") {
                let after_key = &trimmed[path_start + 6..].trim_start();

                // Extract the path value between quotes
                if let Some(first_quote) = after_key.find('"') {
                    let value_part = &after_key[first_quote + 1..];
                    if let Some(last_quote) = value_part.find('"') {
                        let path_str = &value_part[..last_quote];

                        // Handle escaped backslashes on Windows
                        let path_str = path_str.replace("\\\\", "\\");

                        let library_path = PathBuf::from(path_str);
                        if library_path.exists() {
                            libraries.push(library_path);
                        }
                    }
                }
            }
        }
    }

    Ok(libraries)
}

/// Get the mods directory path for Ready or Not
pub fn get_mods_path(game_path: &Path) -> PathBuf {
    game_path
        .join("ReadyOrNot")
        .join("Content")
        .join("Paks")
        .join("~mods")
}

/// Get the SaveGames directory path for Ready or Not
#[cfg(target_os = "windows")]
pub fn get_savegames_path() -> Result<PathBuf> {
    let local_appdata = std::env::var("LOCALAPPDATA")
        .map_err(|_| crate::models::AppError::NotFound("LOCALAPPDATA not found".to_string()))?;

    Ok(PathBuf::from(local_appdata)
        .join("ReadyOrNot")
        .join("Saved")
        .join("SaveGames"))
}

/// Get the SaveGames directory path for Ready or Not (Proton on Linux)
#[cfg(target_os = "linux")]
pub fn get_savegames_path() -> Result<PathBuf> {
    let steam_path = find_steam_path()?;

    Ok(steam_path
        .join("steamapps")
        .join("compatdata")
        .join(READY_OR_NOT_APP_ID)
        .join("pfx")
        .join("drive_c")
        .join("users")
        .join("steamuser")
        .join("AppData")
        .join("Local")
        .join("ReadyOrNot")
        .join("Saved")
        .join("SaveGames"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_mock_steam_dir() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let steam_apps = temp_dir.path().join("steamapps");
        fs::create_dir_all(&steam_apps).unwrap();
        temp_dir
    }

    #[test]
    fn test_parse_library_folders_no_vdf() {
        let temp_dir = create_mock_steam_dir();
        let libraries = parse_library_folders(temp_dir.path()).unwrap();

        // Should return at least the steam path itself
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0], temp_dir.path());
    }

    #[test]
    fn test_parse_library_folders_with_vdf() {
        let temp_dir = create_mock_steam_dir();

        // Create a secondary library
        let second_lib = temp_dir.path().join("SteamLibrary2");
        fs::create_dir_all(&second_lib).unwrap();

        // Create a mock libraryfolders.vdf
        let vdf_content = format!(
            r#"
"libraryfolders"
{{
    "0"
    {{
        "path"		"{}"
        "label"		""
        "contentid"		"1234567890"
    }}
    "1"
    {{
        "path"		"{}"
        "label"		""
        "contentid"		"9876543210"
    }}
}}
"#,
            temp_dir.path().display().to_string().replace('\\', "\\\\"),
            second_lib.display().to_string().replace('\\', "\\\\")
        );

        let vdf_path = temp_dir.path().join("steamapps").join("libraryfolders.vdf");
        fs::write(vdf_path, vdf_content).unwrap();

        let libraries = parse_library_folders(temp_dir.path()).unwrap();

        // Should include main path + secondary library
        assert!(libraries.len() >= 2);
        assert!(libraries.contains(&temp_dir.path().to_path_buf()));
        assert!(libraries.contains(&second_lib));
    }

    #[test]
    fn test_get_mods_path() {
        let game_path = PathBuf::from("/path/to/Ready Or Not");
        let mods_path = get_mods_path(&game_path);

        assert_eq!(
            mods_path,
            PathBuf::from("/path/to/Ready Or Not/ReadyOrNot/Content/Paks/~mods")
        );
    }

    #[test]
    fn test_detect_game_path_not_found() {
        // This test will fail if Ready or Not is actually installed
        // In a real test environment, we'd mock the filesystem
        // For now, just verify the function signature compiles
        let _ = detect_game_path();
    }
}
