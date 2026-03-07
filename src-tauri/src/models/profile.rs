use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: Option<String>,
    /// Archive names of mods installed to this profile
    #[serde(alias = "enabled_collections")]
    pub installed_mod_names: Vec<String>,
    #[serde(default)]
    pub created_at: String,
}

impl Profile {
    pub fn new(name: String, installed_mod_names: Vec<String>) -> Self {
        Self {
            name,
            description: None,
            installed_mod_names,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}
