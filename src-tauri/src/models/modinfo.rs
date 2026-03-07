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
