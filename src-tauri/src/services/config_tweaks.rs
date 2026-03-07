use std::fs;
use std::path::PathBuf;

use crate::models::{AppError, Result};

/// Get the platform-specific Game.ini path for Ready or Not
pub fn get_game_ini_path() -> Result<PathBuf> {
    let config_dir = if cfg!(windows) {
        // Windows: %LOCALAPPDATA%\ReadyOrNot\Saved\Config\Windows\Game.ini
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| AppError::Validation("LOCALAPPDATA not found".to_string()))?;
        PathBuf::from(local_app_data)
            .join("ReadyOrNot")
            .join("Saved")
            .join("Config")
            .join("Windows")
    } else {
        // Linux: ~/.local/share/ReadyOrNot/Saved/Config/Linux/Game.ini
        let home = dirs::home_dir()
            .ok_or_else(|| AppError::Validation("home directory not found".to_string()))?;
        home.join(".local")
            .join("share")
            .join("ReadyOrNot")
            .join("Saved")
            .join("Config")
            .join("Linux")
    };

    Ok(config_dir.join("Game.ini"))
}

/// Apply intro skip configuration to Game.ini
pub fn apply_intro_skip() -> Result<()> {
    let ini_path = get_game_ini_path()?;

    // Create parent directories if they don't exist
    if let Some(parent) = ini_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::Validation(format!("failed to create config directory: {e}")))?;
    }

    // Read existing content or start fresh
    let mut content = if ini_path.exists() {
        fs::read_to_string(&ini_path)
            .map_err(|e| AppError::Validation(format!("failed to read Game.ini: {e}")))?
    } else {
        String::from("; Configuration File\n")
    };

    // Ensure section exists
    let section = "[/Script/MoviePlayer.MoviePlayerSettings]";
    if !content.contains(section) {
        content.push('\n');
        content.push_str(section);
        content.push('\n');
    }

    // Add or update config keys
    let keys = vec![
        ("bWaitForMoviesToComplete", "False"),
        ("bMoviesAreSkippable", "True"),
        ("StartupMovies", ""),
    ];

    for (key, value) in keys {
        add_or_update_key(&mut content, section, key, value);
    }

    // Write the modified content back
    fs::write(&ini_path, &content)
        .map_err(|e| AppError::Validation(format!("failed to write Game.ini: {e}")))?;

    // Make read-only on Windows to prevent accidental modification
    #[cfg(windows)]
    {
        let metadata = fs::metadata(&ini_path)
            .map_err(|e| AppError::Validation(format!("failed to get metadata: {e}")))?;
        let mut perms = metadata.permissions();
        perms.set_readonly(true);
        fs::set_permissions(&ini_path, perms)
            .map_err(|e| AppError::Validation(format!("failed to set read-only: {e}")))?;
    }

    Ok(())
}

/// Check if intro skip is already applied
pub fn is_intro_skip_applied() -> Result<bool> {
    let ini_path = get_game_ini_path()?;

    if !ini_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&ini_path)
        .map_err(|e| AppError::Validation(format!("failed to read Game.ini: {e}")))?;

    let section = "[/Script/MoviePlayer.MoviePlayerSettings]";
    if !content.contains(section) {
        return Ok(false);
    }

    // Check if all required keys are present with correct values
    Ok(content.contains("bWaitForMoviesToComplete=False")
        && content.contains("bMoviesAreSkippable=True")
        && content.contains("StartupMovies="))
}

/// Add or update a key in the ini file
fn add_or_update_key(content: &mut String, section: &str, key: &str, value: &str) {
    let search_pattern = format!("{}=", key);

    // Check if key already exists
    if let Some(pos) = content.find(&search_pattern) {
        // Find the end of the line
        if let Some(end_pos) = content[pos..].find('\n') {
            let actual_end = pos + end_pos;
            content.replace_range(pos..actual_end, &format!("{}={}", key, value));
        } else {
            // Key is at end of file, no newline
            content.truncate(pos);
            content.push_str(&format!("{}={}", key, value));
        }
    } else {
        // Key doesn't exist, add it after section
        if let Some(section_pos) = content.find(section) {
            let insert_pos = section_pos + section.len();
            if let Some(newline_pos) = content[insert_pos..].find('\n') {
                let actual_insert = insert_pos + newline_pos + 1;
                content.insert_str(actual_insert, &format!("{}={}\n", key, value));
            } else {
                content.push('\n');
                content.push_str(&format!("{}={}\n", key, value));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_or_update_key_adds_new_key() {
        let mut content = "[/Script/MoviePlayer.MoviePlayerSettings]\n".to_string();
        add_or_update_key(
            &mut content,
            "[/Script/MoviePlayer.MoviePlayerSettings]",
            "bMoviesAreSkippable",
            "True",
        );

        assert!(content.contains("bMoviesAreSkippable=True"));
    }

    #[test]
    fn test_add_or_update_key_updates_existing_key() {
        let mut content =
            "[/Script/MoviePlayer.MoviePlayerSettings]\nbMoviesAreSkippable=False\n".to_string();
        add_or_update_key(
            &mut content,
            "[/Script/MoviePlayer.MoviePlayerSettings]",
            "bMoviesAreSkippable",
            "True",
        );

        assert!(content.contains("bMoviesAreSkippable=True"));
        assert!(!content.contains("bMoviesAreSkippable=False"));
    }

    #[test]
    fn test_is_intro_skip_applied_detects_correct_config() {
        let mut content = "[/Script/MoviePlayer.MoviePlayerSettings]\n".to_string();
        add_or_update_key(
            &mut content,
            "[/Script/MoviePlayer.MoviePlayerSettings]",
            "bWaitForMoviesToComplete",
            "False",
        );
        add_or_update_key(
            &mut content,
            "[/Script/MoviePlayer.MoviePlayerSettings]",
            "bMoviesAreSkippable",
            "True",
        );
        add_or_update_key(
            &mut content,
            "[/Script/MoviePlayer.MoviePlayerSettings]",
            "StartupMovies",
            "",
        );

        assert!(content.contains("bWaitForMoviesToComplete=False"));
        assert!(content.contains("bMoviesAreSkippable=True"));
        assert!(content.contains("StartupMovies="));
    }
}
