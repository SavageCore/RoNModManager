use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: Option<String>,
    pub enabled_collections: Vec<String>,
    #[serde(default)]
    pub created_at: String,
}

impl Profile {
    pub fn new(name: String, enabled_collections: Vec<String>) -> Self {
        Self {
            name,
            description: None,
            enabled_collections,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}
