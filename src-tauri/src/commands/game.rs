use std::path::PathBuf;
use std::process::Command;

use tauri::State;

use crate::models::AppError;
use crate::services::steam;
use crate::state::AppState;

#[tauri::command]
pub async fn detect_game_path(_state: State<'_, AppState>) -> Result<Option<String>, String> {
    match steam::detect_game_path() {
        Ok(path) => Ok(Some(path.to_string_lossy().to_string())),
        Err(AppError::NotFound(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[tauri::command]
pub async fn set_game_path(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Err("Game path does not exist".to_string());
    }

    state
        .update_config(|config| {
            config.game_path = Some(path_buf);
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[tauri::command]
pub async fn launch_game(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.get_config().map_err(|e: AppError| e.to_string())?;

    let game_path = config
        .game_path
        .ok_or_else(|| "Game path not configured".to_string())?;

    // Construct the executable path
    let exe_path = if cfg!(windows) {
        game_path.join("ReadyOrNot.exe")
    } else {
        game_path.join("ReadyOrNot")
    };

    if !exe_path.exists() {
        return Err(format!(
            "Game executable not found at {}",
            exe_path.display()
        ));
    }

    // Spawn the game process
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/C", &format!("start \"\" \"{}\"", exe_path.display())])
            .spawn()
            .map_err(|e| format!("Failed to launch game: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(&exe_path)
            .spawn()
            .map_err(|e| format!("Failed to launch game: {}", e))?;
    }

    Ok(())
}
