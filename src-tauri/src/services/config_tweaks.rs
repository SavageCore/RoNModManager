use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{AppError, Result};

const INTRO_MOVIE_FILES: &[&str] = &["ReadyOrNot_StartupMovie.mp4", "RoNLogo.mp4"];

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
