use std::fs;

use tauri::State;

use crate::models::{AppError, ModPack, Result};
use crate::state::AppState;

#[tauri::command]
pub async fn share_modpack_via_code(
    _modpack: ModPack,
    _state: State<'_, AppState>,
) -> Result<String> {
    Err(AppError::Validation(
        "share_modpack_via_code requires sync server and is not implemented yet".to_string(),
    ))
}

#[tauri::command]
pub async fn import_from_code(_code: String, _state: State<'_, AppState>) -> Result<ModPack> {
    Err(AppError::Validation(
        "import_from_code requires sync server and is not implemented yet".to_string(),
    ))
}

#[tauri::command]
pub async fn push_modpack_update(
    _code: String,
    _modpack: ModPack,
    _state: State<'_, AppState>,
) -> Result<()> {
    Err(AppError::Validation(
        "push_modpack_update requires sync server and is not implemented yet".to_string(),
    ))
}

#[tauri::command]
pub async fn import_modpack_from_file(path: String) -> Result<ModPack> {
    let bytes = fs::read(path).map_err(|e| AppError::Validation(e.to_string()))?;
    serde_json::from_slice::<ModPack>(&bytes).map_err(|e| AppError::Validation(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppConfig;
    use crate::test_support::{mock_app_with, scratch_dir};
    use tauri::Manager;

    fn modpack() -> ModPack {
        serde_json::from_str(
            r#"{
                "schemaVersion": 1,
                "name": "Shared Pack",
                "version": "1.0.0",
                "description": "",
                "author": null,
                "mods": {},
                "collections": {}
            }"#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn code_based_sharing_is_not_implemented_yet() {
        let app = mock_app_with(AppConfig::default());
        let state = app.state::<AppState>();

        let share = share_modpack_via_code(modpack(), state.clone()).await;
        assert!(share
            .unwrap_err()
            .to_string()
            .contains("is not implemented yet"));

        let import = import_from_code("code".to_string(), state.clone()).await;
        assert!(import
            .unwrap_err()
            .to_string()
            .contains("is not implemented yet"));

        let push = push_modpack_update("code".to_string(), modpack(), state).await;
        assert!(push
            .unwrap_err()
            .to_string()
            .contains("is not implemented yet"));
    }

    #[tokio::test]
    async fn modpack_files_are_imported_from_disk() {
        let dir = scratch_dir("sharing");
        let path = dir.path().join("pack.json");
        fs::write(&path, serde_json::to_vec(&modpack()).unwrap()).unwrap();

        let imported = import_modpack_from_file(path.to_string_lossy().to_string())
            .await
            .unwrap();

        assert_eq!(imported.name, "Shared Pack");
        assert_eq!(imported.version, "1.0.0");
    }

    #[tokio::test]
    async fn importing_rejects_missing_files_and_bad_json() {
        let dir = scratch_dir("sharing-bad");

        let missing = import_modpack_from_file(
            dir.path()
                .join("missing.json")
                .to_string_lossy()
                .to_string(),
        )
        .await;
        assert!(missing.is_err());

        let broken = dir.path().join("broken.json");
        fs::write(&broken, b"{not json").unwrap();
        let invalid = import_modpack_from_file(broken.to_string_lossy().to_string()).await;
        assert!(invalid.is_err());
    }
}
