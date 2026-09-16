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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_event_carries_byte_counts() {
        let event = ProgressEvent::new_download("mod.zip", 42.5, 850, 2000);
        assert_eq!(event.operation, "download");
        assert_eq!(event.file, "mod.zip");
        assert_eq!(event.percent, 42.5);
        assert_eq!(event.message, "Downloading mod.zip...");
        assert_eq!(event.total_bytes, Some(2000));
        assert_eq!(event.processed_bytes, Some(850));
    }

    #[test]
    fn install_and_extract_events_omit_byte_counts() {
        let install = ProgressEvent::new_install("mod.zip", 10.0);
        assert_eq!(install.operation, "install");
        assert_eq!(install.message, "Installing mod.zip...");
        assert_eq!(install.percent, 10.0);
        assert_eq!(install.total_bytes, None);
        assert_eq!(install.processed_bytes, None);

        let extract = ProgressEvent::new_extract("mod.zip", 99.5);
        assert_eq!(extract.operation, "extract");
        assert_eq!(extract.message, "Extracting mod.zip...");
        assert_eq!(extract.percent, 99.5);
        assert_eq!(extract.total_bytes, None);
        assert_eq!(extract.processed_bytes, None);
    }

    #[test]
    fn complete_and_error_events_are_terminal() {
        let complete = ProgressEvent::new_complete();
        assert_eq!(complete.operation, "complete");
        assert_eq!(complete.percent, 100.0);
        assert_eq!(complete.message, "Installation complete!");
        assert!(complete.file.is_empty());
        assert_eq!(complete.total_bytes, None);

        let error = ProgressEvent::new_error("disk full".to_string());
        assert_eq!(error.operation, "error");
        assert_eq!(error.percent, 0.0);
        assert_eq!(error.message, "disk full");
        assert!(error.file.is_empty());
        assert_eq!(error.processed_bytes, None);
    }

    #[test]
    fn events_round_trip_through_json() {
        let event = ProgressEvent::new_download("mod.zip", 12.5, 1, 8);
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["operation"], "download");
        assert_eq!(json["file"], "mod.zip");
        assert_eq!(json["total_bytes"], 8);
        assert_eq!(json["processed_bytes"], 1);
        assert_eq!(json["percent"].as_f64(), Some(12.5));

        let parsed: ProgressEvent = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.message, "Downloading mod.zip...");
        assert_eq!(parsed.total_bytes, Some(8));
    }

    #[test]
    fn events_deserialize_null_byte_counts() {
        let parsed: ProgressEvent = serde_json::from_str(
            r#"{"operation":"install","file":"a.zip","percent":5.0,"message":"m","total_bytes":null,"processed_bytes":null}"#,
        )
        .unwrap();
        assert_eq!(parsed.operation, "install");
        assert_eq!(parsed.total_bytes, None);
        assert_eq!(parsed.processed_bytes, None);

        // The struct is Clone + Debug, which the installer threads rely on.
        let cloned = parsed.clone();
        assert_eq!(cloned.message, parsed.message);
        assert!(format!("{cloned:?}").contains("install"));
    }
}
