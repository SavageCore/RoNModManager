use std::fs;
use std::path::PathBuf;

use crate::models::{AppError, Result};

/// Get the platform-specific Game.ini path for Ready or Not
pub fn get_game_ini_path() -> Result<PathBuf> {
    let config_dir = if cfg!(windows) {
        // Windows native path: %LOCALAPPDATA%\ReadyOrNot\Saved\Config\Windows\Game.ini
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| AppError::Validation("LOCALAPPDATA not found".to_string()))?;
        PathBuf::from(local_app_data)
            .join("ReadyOrNot")
            .join("Saved")
            .join("Config")
            .join("Windows")
    } else {
        // Linux library-specific Proton path derived from detected game install.
        if let Ok(game_path) = crate::services::steam::detect_game_path() {
            if let Some(common_dir) = game_path.parent() {
                if let Some(steamapps_dir) = common_dir.parent() {
                    if steamapps_dir.file_name().and_then(|n| n.to_str()) == Some("steamapps") {
                        if let Some(library_dir) = steamapps_dir.parent() {
                            return Ok(library_dir
                                .join("steamapps")
                                .join("compatdata")
                                .join("1144200")
                                .join("pfx")
                                .join("drive_c")
                                .join("users")
                                .join("steamuser")
                                .join("AppData")
                                .join("Local")
                                .join("ReadyOrNot")
                                .join("Saved")
                                .join("Config")
                                .join("Windows")
                                .join("Game.ini"));
                        }
                    }
                }
            }
        }

        // Linux Steam/Proton path (preferred):
        // ~/.steam/steam/steamapps/compatdata/1144200/pfx/drive_c/users/steamuser/AppData/Local/ReadyOrNot/Saved/Config/Windows/Game.ini
        if let Ok(savegames_path) = crate::services::steam::get_savegames_path() {
            if let Some(saved_dir) = savegames_path.parent() {
                return Ok(saved_dir.join("Config").join("Windows").join("Game.ini"));
            }
        }

        // Linux fallback path for non-Proton layouts
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

    rewrite_movie_player_settings_section(
        &mut content,
        section,
        &[
            "bWaitForMoviesToComplete=False",
            "bMoviesAreSkippable=True",
            "StartupMovies=",
        ],
    );

    // Write the modified content back
    fs::write(&ini_path, &content)
        .map_err(|e| AppError::Validation(format!("failed to write Game.ini: {e}")))?;

    Ok(())
}

/// Remove intro skip configuration from Game.ini
pub fn undo_intro_skip() -> Result<()> {
    let ini_path = get_game_ini_path()?;
    if !ini_path.exists() {
        return Ok(());
    }

    let mut content = fs::read_to_string(&ini_path)
        .map_err(|e| AppError::Validation(format!("failed to read Game.ini: {e}")))?;

    let section = "[/Script/MoviePlayer.MoviePlayerSettings]";
    if !content.contains(section) {
        return Ok(());
    }

    // Remove managed intro-skip keys instead of forcing default values.
    remove_section_keys(
        &mut content,
        section,
        &[
            "bWaitForMoviesToComplete",
            "bMoviesAreSkippable",
            "!StartupMovies",
            "StartupMovies",
            "+StartupMovies",
            "-StartupMovies",
        ],
    );
    remove_empty_section(&mut content, section);

    fs::write(&ini_path, &content)
        .map_err(|e| AppError::Validation(format!("failed to write Game.ini: {e}")))?;

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

    let normalized = content.replace("\r\n", "\n");
    let expected_block = "[/Script/MoviePlayer.MoviePlayerSettings]\n\
bWaitForMoviesToComplete=False\n\
bMoviesAreSkippable=True\n\
StartupMovies=";

    Ok(normalized.contains(expected_block))
}

/// Remove key lines within a section while preserving all other content.
fn remove_section_keys(content: &mut String, section: &str, keys: &[&str]) {
    let mut in_section = false;
    let mut output: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            in_section = trimmed == section;
        }

        if in_section {
            let should_remove = keys
                .iter()
                .any(|key| trimmed.starts_with(&format!("{}=", key)));
            if should_remove {
                continue;
            }
        }

        output.push(line.to_string());
    }

    *content = output.join("\n");
    if !content.ends_with('\n') {
        content.push('\n');
    }
}

/// Remove a section header when no key/value lines remain in that section.
fn remove_empty_section(content: &mut String, section: &str) {
    let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let mut section_start: Option<usize> = None;
    let mut section_end = lines.len();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if section_start.is_some() {
                section_end = idx;
                break;
            }
            if trimmed == section {
                section_start = Some(idx);
            }
        }
    }

    let Some(start) = section_start else {
        return;
    };

    let has_key_lines = lines[start + 1..section_end]
        .iter()
        .any(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with(';') && trimmed.contains('=')
        });

    if has_key_lines {
        return;
    }

    lines.remove(start);

    // Trim one adjacent blank line to avoid leaving visual gaps.
    if start < lines.len() && lines[start].trim().is_empty() {
        lines.remove(start);
    } else if start > 0 && lines[start - 1].trim().is_empty() {
        lines.remove(start - 1);
    }

    *content = lines.join("\n");
    if !content.ends_with('\n') {
        content.push('\n');
    }
}

/// Rewrite a section so managed movie-player keys are removed, then add desired lines in exact order.
fn rewrite_movie_player_settings_section(content: &mut String, section: &str, desired_lines: &[&str]) {
    let mut output: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut inserted = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            if in_section {
                in_section = false;
            }

            output.push(line.to_string());
            if trimmed == section {
                for desired in desired_lines {
                    output.push((*desired).to_string());
                }
                inserted = true;
                in_section = true;
            }
            continue;
        }

        if in_section {
            let managed_prefixes = [
                "bWaitForMoviesToComplete=",
                "bMoviesAreSkippable=",
                "StartupMovies=",
                "+StartupMovies=",
                "-StartupMovies=",
                "!StartupMovies=",
            ];
            if managed_prefixes.iter().any(|prefix| trimmed.starts_with(prefix)) {
                continue;
            }
        }

        output.push(line.to_string());
    }

    if !inserted {
        if !output.is_empty() {
            output.push(String::new());
        }
        output.push(section.to_string());
        for desired in desired_lines {
            output.push((*desired).to_string());
        }
    }

    *content = output.join("\n");
    if !content.ends_with('\n') {
        content.push('\n');
    }
}

/// Add or update a key in the ini file
#[cfg(test)]
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

    #[test]
    fn test_rewrite_movie_player_settings_section_enforces_exact_order() {
        let section = "[/Script/MoviePlayer.MoviePlayerSettings]";
        let mut content = format!(
            "{}\n+StartupMovies=Intro\nbMoviesAreSkippable=False\nbWaitForMoviesToComplete=True\n",
            section
        );

        rewrite_movie_player_settings_section(
            &mut content,
            section,
            &[
                "bWaitForMoviesToComplete=False",
                "bMoviesAreSkippable=True",
                "StartupMovies=",
            ],
        );

        let expected_block = format!(
            "{}\nbWaitForMoviesToComplete=False\nbMoviesAreSkippable=True\nStartupMovies=",
            section
        );

        assert!(content.contains(&expected_block));
        assert!(!content.contains("+StartupMovies=Intro"));
        assert!(!content.contains("bMoviesAreSkippable=False"));
        assert!(!content.contains("bWaitForMoviesToComplete=True"));
    }

    #[test]
    fn test_undo_cleanup_removes_empty_movie_player_section() {
        let section = "[/Script/MoviePlayer.MoviePlayerSettings]";
        let mut content = format!(
            "{}\nbWaitForMoviesToComplete=True\nbMoviesAreSkippable=False\n\n[Other]\nKey=Value\n",
            section
        );

        remove_section_keys(
            &mut content,
            section,
            &[
                "bWaitForMoviesToComplete",
                "bMoviesAreSkippable",
                "!StartupMovies",
                "StartupMovies",
                "+StartupMovies",
                "-StartupMovies",
            ],
        );
        remove_empty_section(&mut content, section);

        assert!(!content.contains(section));
        assert!(content.contains("[Other]"));
        assert!(content.contains("Key=Value"));
    }
}
