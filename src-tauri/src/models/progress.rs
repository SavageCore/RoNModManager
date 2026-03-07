use serde::{Deserialize, Serialize};

/// Progress event emitted during long-running operations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    /// Operation type: "download", "install", "extract", etc.
    pub operation: String,
    /// Current file being processed
    pub file: String,
    /// Current progress percentage (0-100)
    pub percent: f32,
    /// Human-readable status message
    pub message: String,
    /// Total bytes to process (for download/install)
    pub total_bytes: Option<u64>,
    /// Bytes processed so far
    pub processed_bytes: Option<u64>,
}

impl ProgressEvent {
    pub fn new_download(file: &str, percent: f32, processed: u64, total: u64) -> Self {
        Self {
            operation: "download".to_string(),
            file: file.to_string(),
            percent,
            message: format!("Downloading {}...", file),
            total_bytes: Some(total),
            processed_bytes: Some(processed),
        }
    }

    pub fn new_install(file: &str, percent: f32) -> Self {
        Self {
            operation: "install".to_string(),
            file: file.to_string(),
            percent,
            message: format!("Installing {}...", file),
            total_bytes: None,
            processed_bytes: None,
        }
    }

    pub fn new_extract(file: &str, percent: f32) -> Self {
        Self {
            operation: "extract".to_string(),
            file: file.to_string(),
            percent,
            message: format!("Extracting {}...", file),
            total_bytes: None,
            processed_bytes: None,
        }
    }

    pub fn new_complete() -> Self {
        Self {
            operation: "complete".to_string(),
            file: String::new(),
            percent: 100.0,
            message: "Installation complete!".to_string(),
            total_bytes: None,
            processed_bytes: None,
        }
    }

    pub fn new_error(message: String) -> Self {
        Self {
            operation: "error".to_string(),
            file: String::new(),
            percent: 0.0,
            message,
            total_bytes: None,
            processed_bytes: None,
        }
    }
}
