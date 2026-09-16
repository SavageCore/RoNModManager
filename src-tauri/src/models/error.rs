use serde::Serialize;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
    #[error("Windows-only native mod: {0:?}")]
    BlockedWindowsOnlyDll(Vec<String>),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<AppError> for String {
    fn from(value: AppError) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_display_with_a_category_prefix() {
        let io = AppError::from(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert_eq!(io.to_string(), "I/O error: gone");

        let json = AppError::from(serde_json::from_str::<u8>("nope").unwrap_err());
        assert!(json.to_string().starts_with("JSON error: "));

        assert_eq!(
            AppError::Validation("bad input".to_string()).to_string(),
            "Validation error: bad input"
        );
        assert_eq!(
            AppError::NotFound("mod.zip".to_string()).to_string(),
            "Not found: mod.zip"
        );
        assert_eq!(
            AppError::Unsupported("no sftp".to_string()).to_string(),
            "Unsupported operation: no sftp"
        );
        assert_eq!(
            AppError::BlockedWindowsOnlyDll(vec!["native.dll".to_string()]).to_string(),
            "Windows-only native mod: [\"native.dll\"]"
        );
    }

    #[tokio::test]
    async fn http_errors_display_with_the_http_prefix() {
        let err = reqwest::Client::new()
            .get("http://127.0.0.1:1/")
            .send()
            .await
            .expect_err("connecting to a closed port must fail");
        assert!(AppError::from(err).to_string().starts_with("HTTP error: "));
    }

    #[test]
    fn errors_serialize_as_their_display_string() {
        let err = AppError::NotFound("mod.zip".to_string());
        assert_eq!(
            serde_json::to_string(&err).unwrap(),
            "\"Not found: mod.zip\""
        );

        // Commands return `String` errors through this conversion.
        let message: String = err.into();
        assert_eq!(message, "Not found: mod.zip");
    }
}
