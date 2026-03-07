use reqwest::Client;

use crate::models::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub client: Client,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: AppConfig::default(),
            client: Client::new(),
        }
    }
}
