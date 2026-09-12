use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ModSource {
    ModIo { mod_id: String },
    ModPack,
    Manual,
    Collection { name: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ModStatus {
    NotInstalled,
    Downloading,
    Downloaded,
    Installed,
    UpdateAvailable,
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModInfo {
    pub name: String,
    pub source: ModSource,
    pub status: ModStatus,
    pub filename: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModFile {
    pub name: String,
    pub path: String,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub archive_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub size: Option<u64>,
    /// Path of the file inside the mod, e.g.
    /// `Mods/VoiceCommanderMod/Scripts/main.lua` (the redundant
    /// `ReadyOrNot/Binaries/Win64/` prefix is stripped). Lets the UI tell
    /// apart same-named files (a dozen `main.lua`s in one UE4SS group)
    /// without showing the full staged absolute path.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relative_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModGroup {
    pub name: String,
    pub display_name: Option<String>,
    pub source_url: Option<String>,
    pub managed_by_manifest: bool,
    pub installed_at: Option<u64>,
    pub files: Vec<InstalledModFile>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub addon_files: Vec<InstalledModFile>,
    pub has_override_files: bool,
    #[serde(default)]
    pub is_ue4ss_mod: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub installed_version: Option<String>,
    #[serde(default)]
    pub total_size: u64,
}
