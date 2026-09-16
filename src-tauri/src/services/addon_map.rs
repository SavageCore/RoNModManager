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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::isolated_root;

    #[test]
    fn addon_map_path_lives_under_the_staging_root() {
        isolated_root();
        assert_eq!(
            get_addon_map_path().unwrap(),
            crate::state::app_data_root()
                .unwrap()
                .join("staged")
                .join("addon_map.json")
        );
    }

    #[test]
    fn write_then_read_round_trips_and_reports_malformed_json() {
        // The addon map file is shared with the command-level tests.
        let _tree = crate::test_support::shared_tree_guard();
        isolated_root();
        let path = get_addon_map_path().unwrap();
        // write_addon_map does not create the staging root; callers stage first.
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        // An empty map is stored as an empty JSON object, not left absent.
        write_addon_map(&HashMap::new()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
        assert!(read_addon_map().unwrap().is_empty());

        let map = HashMap::from([
            (
                "cov-parent.zip".to_string(),
                vec!["cov-addon-a.zip".to_string(), "cov-addon-b.zip".to_string()],
            ),
            ("cov-solo.zip".to_string(), Vec::new()),
        ]);
        write_addon_map(&map).unwrap();
        assert_eq!(read_addon_map().unwrap(), map);

        // Malformed JSON is reported, not silently treated as an empty map.
        fs::write(&path, "{ not a map").unwrap();
        let err = read_addon_map().unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
        assert!(err.to_string().contains("Failed to parse addon map"));

        // A missing file reads as an empty map.
        fs::remove_file(&path).unwrap();
        assert!(read_addon_map().unwrap().is_empty());
    }
}
