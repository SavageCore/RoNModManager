use tauri::State;

use crate::models::Result;
use crate::services::ue4ss::{self, Ue4ssConsoleMode, Ue4ssSettings};
use crate::state::AppState;

/// Current UE4SS runtime settings, read from the staged
/// `UE4SS-settings.ini`. Reports stock defaults when the runtime is not
/// installed.
#[tauri::command]
pub async fn get_ue4ss_settings(state: State<'_, AppState>) -> Result<Ue4ssSettings> {
    let config = state.get_config()?;
    let Some(game_path) = config.game_path else {
        let mut stock = Ue4ssSettings::guide_defaults();
        stock.settings_present = false;
        return Ok(stock);
    };
    Ok(ue4ss::read_settings(&game_path))
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssSettingsUpdate {
    pub use_object_array_cache: bool,
    pub engine_major_version: String,
    pub engine_minor_version: String,
    pub graphics_api: String,
    pub hook_begin_play: bool,
    pub console_mode: Ue4ssConsoleMode,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssSettingsApplyResult {
    pub changed: bool,
    pub stale_shims_removed: usize,
}

/// Write UE4SS settings to the staged ini (symlinked live, so it applies
/// immediately) and sweep stale UE4SS 2.x `xinput1_3.dll` shims.
#[tauri::command]
pub async fn set_ue4ss_settings(
    state: State<'_, AppState>,
    updates: Ue4ssSettingsUpdate,
) -> Result<Ue4ssSettingsApplyResult> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("Game path is not configured".to_string())
    })?;
    // Validate the graphics API against what UE4SS accepts; free-text engine
    // versions are fine (numeric, but UE4SS tolerates anything there).
    if !ue4ss::GRAPHICS_API_OPTIONS
        .iter()
        .any(|o| o.eq_ignore_ascii_case(&updates.graphics_api))
    {
        return Err(crate::models::AppError::Validation(format!(
            "Unknown GraphicsAPI '{}' - expected one of: {}",
            updates.graphics_api,
            ue4ss::GRAPHICS_API_OPTIONS.join(", ")
        )));
    }
    let changed = ue4ss::apply_settings(
        &game_path,
        &Ue4ssSettings {
            use_object_array_cache: updates.use_object_array_cache,
            engine_major_version: updates.engine_major_version.trim().to_string(),
            engine_minor_version: updates.engine_minor_version.trim().to_string(),
            graphics_api: updates.graphics_api.trim().to_lowercase(),
            hook_begin_play: updates.hook_begin_play,
            console_mode: updates.console_mode,
            settings_present: true,
        },
    )?;
    let stale_shims_removed = ue4ss::remove_stale_shims(&game_path);
    Ok(Ue4ssSettingsApplyResult {
        changed,
        stale_shims_removed,
    })
}
