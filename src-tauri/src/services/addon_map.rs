use crate::models::AppError;
use crate::state;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

type AddonMap = HashMap<String, Vec<String>>;

const ADDON_MAP_FILENAME: &str = "addon_map.json";

pub fn get_addon_map_path() -> Result<PathBuf, AppError> {
    let staging_root = state::app_data_root()?.join("staged");
    Ok(staging_root.join(ADDON_MAP_FILENAME))
}

pub fn read_addon_map() -> Result<AddonMap, AppError> {
    let path = get_addon_map_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let data = fs::read_to_string(&path).map_err(AppError::Io)?;
    let map: AddonMap = serde_json::from_str(&data)
        .map_err(|e| AppError::Validation(format!("Failed to parse addon map: {e}")))?;
    Ok(map)
}

pub fn write_addon_map(map: &AddonMap) -> Result<(), AppError> {
    let path = get_addon_map_path()?;
    // Always create the file, even if the map is empty
    let data = if map.is_empty() {
        "{}".to_string()
    } else {
        serde_json::to_string_pretty(map)
            .map_err(|e| AppError::Validation(format!("Failed to serialize addon map: {e}")))?
    };
    fs::write(&path, data).map_err(AppError::Io)?;
    Ok(())
}
