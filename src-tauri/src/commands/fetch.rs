use crate::models::{AppError, Result};
use crate::state::AppState;
use log;
use tauri::State;

#[tauri::command]
pub async fn fetch_modpack_json(
    url: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value> {
    let client = &state.client;
    let response = client.get(&url).send().await.map_err(AppError::from)?;
    let response = response.error_for_status().map_err(AppError::from)?;
    let bytes = response.bytes().await.map_err(AppError::from)?;
    match std::str::from_utf8(&bytes) {
        Ok(_) => {}
        Err(_) => log::error!("Response body: <non-UTF8 bytes, length={}>", bytes.len()),
    }
    let json = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(AppError::from)?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppConfig;
    use crate::test_support::mock_app_with;
    use tauri::Manager;

    #[tokio::test]
    async fn modpack_json_is_fetched_and_parsed() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/modpack.json")
            .with_status(200)
            .with_body(r#"{"name":"Pack","mods":{"a.zip":{"enabled":true}}}"#)
            .create_async()
            .await;
        let app = mock_app_with(AppConfig::default());

        let json = fetch_modpack_json(
            format!("{}/modpack.json", server.url()),
            app.state::<AppState>(),
        )
        .await
        .unwrap();

        mock.assert_async().await;
        assert_eq!(json["name"], "Pack");
        assert_eq!(json["mods"]["a.zip"]["enabled"], true);
    }

    #[tokio::test]
    async fn error_responses_surface_as_errors() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/missing.json")
            .with_status(404)
            .create_async()
            .await;
        let app = mock_app_with(AppConfig::default());

        let result = fetch_modpack_json(
            format!("{}/missing.json", server.url()),
            app.state::<AppState>(),
        )
        .await;

        mock.assert_async().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn non_json_bodies_are_rejected() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/html")
            .with_status(200)
            .with_body("<html>not json</html>")
            .create_async()
            .await;
        let app = mock_app_with(AppConfig::default());

        let result =
            fetch_modpack_json(format!("{}/html", server.url()), app.state::<AppState>()).await;

        mock.assert_async().await;
        assert!(result.is_err());
    }
}
