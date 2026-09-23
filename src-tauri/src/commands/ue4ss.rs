use tauri::State;

use crate::models::Result;
use crate::services::ue4ss::{
    self, Ue4ssConsoleMode, Ue4ssLuaConfig, Ue4ssModConfig, Ue4ssSettings,
};
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

/// Read a UE4SS script mod's `config.json` (e.g. SRankAlert's settings).
/// Reports `exists: false` before the mod's first launch creates the file -
/// the editor can still create one, and the mod keeps any pre-existing file
/// (missing fields use its defaults).
#[tauri::command]
pub async fn get_ue4ss_mod_config(
    state: State<'_, AppState>,
    mod_name: String,
) -> Result<Ue4ssModConfig> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("Game path is not configured".to_string())
    })?;
    ue4ss::read_mod_config(&game_path, &mod_name)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssModConfigWriteResult {
    pub path: String,
}

/// Write (or create) a UE4SS script mod's `config.json`. The content must be
/// a JSON object - syntax errors are rejected so a broken file can't disable
/// the mod's safe features for a session. Takes effect on next game launch.
#[tauri::command]
pub async fn set_ue4ss_mod_config(
    state: State<'_, AppState>,
    mod_name: String,
    content: String,
) -> Result<Ue4ssModConfigWriteResult> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("Game path is not configured".to_string())
    })?;
    let path = ue4ss::write_mod_config(&game_path, &mod_name, &content)?;
    Ok(Ue4ssModConfigWriteResult {
        path: path.to_string_lossy().to_string(),
    })
}

/// Read a UE4SS script mod's Lua config file (`Scripts/config.lua`,
/// seeded from `Scripts/config.default.lua` when the user file doesn't
/// exist yet). Returns the raw text for the file editor.
#[tauri::command]
pub async fn get_ue4ss_lua_config(
    state: State<'_, AppState>,
    mod_name: String,
) -> Result<Ue4ssLuaConfig> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("Game path is not configured".to_string())
    })?;
    ue4ss::read_lua_config(&game_path, &mod_name)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssLuaConfigWriteResult {
    pub path: String,
}

/// Write a UE4SS script mod's Lua config file as raw text. Anything with
/// Lua syntax errors is rejected before it can reach the game. The pre-edit
/// file is backed up once (see `revert_ue4ss_lua_config`). Takes effect on
/// next game launch.
#[tauri::command]
pub async fn set_ue4ss_lua_config(
    state: State<'_, AppState>,
    mod_name: String,
    content: String,
) -> Result<Ue4ssLuaConfigWriteResult> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("Game path is not configured".to_string())
    })?;
    let path = ue4ss::write_lua_config(&game_path, &mod_name, &content)?;
    Ok(Ue4ssLuaConfigWriteResult {
        path: path.to_string_lossy().to_string(),
    })
}

/// Restore a UE4SS script mod's Lua config from its pre-edit backup.
#[tauri::command]
pub async fn revert_ue4ss_lua_config(
    state: State<'_, AppState>,
    mod_name: String,
) -> Result<Ue4ssLuaConfigWriteResult> {
    let config = state.get_config()?;
    let game_path = config.game_path.ok_or_else(|| {
        crate::models::AppError::Validation("Game path is not configured".to_string())
    })?;
    let path = ue4ss::revert_lua_config(&game_path, &mod_name)?;
    Ok(Ue4ssLuaConfigWriteResult {
        path: path.to_string_lossy().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppConfig;
    use crate::test_support::{mock_app_with, scratch_dir};
    use tauri::Manager;

    fn update() -> Ue4ssSettingsUpdate {
        Ue4ssSettingsUpdate {
            use_object_array_cache: false,
            engine_major_version: "5.3".to_string(),
            engine_minor_version: "2".to_string(),
            graphics_api: "dx11".to_string(),
            hook_begin_play: false,
            console_mode: Ue4ssConsoleMode::None,
        }
    }

    /// A game tree with a hand-installed ini the commands can patch in place.
    fn game_dir_with_ini(contents: &str) -> tempfile::TempDir {
        let dir = scratch_dir("ue4ss-game");
        let ini = dir
            .path()
            .join("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS-settings.ini");
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(ini, contents).unwrap();
        dir
    }

    #[tokio::test]
    async fn settings_default_to_the_modding_guide_without_a_game_path() {
        let app = mock_app_with(AppConfig::default());

        let settings = get_ue4ss_settings(app.state::<AppState>()).await.unwrap();

        assert!(!settings.settings_present);
        assert_eq!(settings.graphics_api, ue4ss::GUIDE_GRAPHICS_API);
        assert_eq!(
            settings.engine_major_version,
            ue4ss::GUIDE_MAJOR_VERSION.to_string()
        );
        assert_eq!(settings.console_mode, Ue4ssConsoleMode::Gui);
    }

    #[tokio::test]
    async fn missing_runtime_reports_empty_stock_settings() {
        let game = scratch_dir("ue4ss-empty");
        let app = mock_app_with(AppConfig {
            game_path: Some(game.path().to_path_buf()),
            ..AppConfig::default()
        });

        let settings = get_ue4ss_settings(app.state::<AppState>()).await.unwrap();

        assert!(!settings.settings_present);
        assert_eq!(settings.graphics_api, "opengl");
        assert!(settings.use_object_array_cache);
    }

    #[tokio::test]
    async fn writing_settings_requires_a_game_path() {
        let app = mock_app_with(AppConfig::default());

        assert!(set_ue4ss_settings(app.state::<AppState>(), update())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn writing_settings_rejects_an_unknown_graphics_api() {
        let game = scratch_dir("ue4ss-bad-api");
        let app = mock_app_with(AppConfig {
            game_path: Some(game.path().to_path_buf()),
            ..AppConfig::default()
        });

        let result = set_ue4ss_settings(
            app.state::<AppState>(),
            Ue4ssSettingsUpdate {
                graphics_api: "directx".to_string(),
                ..update()
            },
        )
        .await;

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown GraphicsAPI"));
    }

    #[tokio::test]
    async fn writing_settings_without_a_runtime_is_rejected() {
        let game = scratch_dir("ue4ss-no-ini");
        let app = mock_app_with(AppConfig {
            game_path: Some(game.path().to_path_buf()),
            ..AppConfig::default()
        });

        assert!(set_ue4ss_settings(app.state::<AppState>(), update())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn settings_round_trip_through_the_ini() {
        let game = game_dir_with_ini("");
        let app = mock_app_with(AppConfig {
            game_path: Some(game.path().to_path_buf()),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();

        let first = set_ue4ss_settings(state.clone(), update()).await.unwrap();
        assert!(first.changed);
        assert_eq!(first.stale_shims_removed, 0);

        let settings = get_ue4ss_settings(state.clone()).await.unwrap();
        assert!(settings.settings_present);
        assert_eq!(settings.graphics_api, "dx11");
        assert_eq!(settings.engine_major_version, "5.3");
        assert_eq!(settings.engine_minor_version, "2");
        assert_eq!(settings.console_mode, Ue4ssConsoleMode::None);
        assert!(!settings.use_object_array_cache);
        assert!(!settings.hook_begin_play);

        // Re-applying the same values writes nothing.
        let second = set_ue4ss_settings(state, update()).await.unwrap();
        assert!(!second.changed);
    }

    #[tokio::test]
    async fn engine_versions_are_trimmed_and_the_api_lower_cased() {
        let game = game_dir_with_ini("");
        let app = mock_app_with(AppConfig {
            game_path: Some(game.path().to_path_buf()),
            ..AppConfig::default()
        });
        let state = app.state::<AppState>();

        set_ue4ss_settings(
            state.clone(),
            Ue4ssSettingsUpdate {
                engine_major_version: "  5.4 ".to_string(),
                engine_minor_version: " 1 ".to_string(),
                graphics_api: "Vulkan".to_string(),
                ..update()
            },
        )
        .await
        .unwrap();

        let settings = get_ue4ss_settings(state).await.unwrap();
        assert_eq!(settings.engine_major_version, "5.4");
        assert_eq!(settings.engine_minor_version, "1");
        assert_eq!(settings.graphics_api, "vulkan");
    }

    #[tokio::test]
    async fn stale_shims_are_swept_when_settings_are_written() {
        let game = game_dir_with_ini("");
        let shim = game.path().join("ReadyOrNot/Binaries/Win64/xinput1_3.dll");
        std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
        std::fs::write(&shim, b"stale").unwrap();
        let app = mock_app_with(AppConfig {
            game_path: Some(game.path().to_path_buf()),
            ..AppConfig::default()
        });

        let result = set_ue4ss_settings(app.state::<AppState>(), update())
            .await
            .unwrap();

        assert_eq!(result.stale_shims_removed, 1);
        assert!(!shim.exists());
    }
}
