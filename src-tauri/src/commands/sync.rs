use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use russh::client;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::models::{AppError, ProgressEvent, Result};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncAuth {
    Auto,
    Password {
        password: String,
    },
    KeyFile {
        path: String,
        passphrase: Option<String>,
    },
}

struct SshClientHandler;

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }
}

#[tauri::command]
pub async fn sync_modpack_to_remote(
    app: AppHandle,
    state: State<'_, AppState>,
    auth: Option<SyncAuth>,
    verbose: Option<bool>,
) -> Result<()> {
    let verbose = verbose.unwrap_or(false);
    let config = state.get_config()?;

    let export_dir = config
        .last_export_dir
        .as_deref()
        .map(expand_tilde)
        .ok_or_else(|| {
            AppError::Validation(
                "No export directory recorded - export a modpack first.".to_string(),
            )
        })?;
    let export_dir = PathBuf::from(export_dir);

    let remote_host = config.sync_remote_host.as_deref().ok_or_else(|| {
        AppError::Validation("No remote host configured. Set it in Settings.".to_string())
    })?;
    let remote_path = config
        .sync_remote_path
        .clone()
        .ok_or_else(|| AppError::Validation("No remote path configured.".to_string()))?;

    let (username, hostname) = parse_user_host(remote_host)?;

    let emit_progress = |msg: &str, percent: f32| {
        let _ = app.emit(
            "sync_progress",
            ProgressEvent {
                operation: "sync".to_string(),
                file: String::new(),
                percent,
                message: msg.to_string(),
                total_bytes: None,
                processed_bytes: None,
            },
        );
    };

    emit_progress("Connecting...", 0.0);

    let config_arc = Arc::new(client::Config::default());
    let mut session = client::connect(config_arc, (hostname.as_str(), 22u16), SshClientHandler)
        .await
        .map_err(|e| AppError::Validation(format!("SSH connect failed: {e}")))?;

    let authenticated = authenticate(&mut session, &username, auth).await?;
    if !authenticated {
        return Err(AppError::Validation("AUTH_REQUIRED".to_string()));
    }

    emit_progress("Opening SFTP session...", 2.0);

    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| AppError::Validation(format!("Failed to open channel: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| AppError::Validation(format!("Failed to start SFTP: {e}")))?;
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| AppError::Validation(format!("Failed to create SFTP session: {e}")))?;

    emit_progress("Scanning remote...", 5.0);

    ensure_remote_dir(&sftp, &remote_path).await?;
    let mods_remote = format!("{remote_path}/mods");
    ensure_remote_dir(&sftp, &mods_remote).await?;

    let remote_files = list_remote_files(&sftp, &remote_path).await?;
    let local_files = list_local_files(&export_dir)?;

    let total = local_files.len();

    for (done, (rel_path, local_size)) in local_files.iter().enumerate() {
        let remote_size = remote_files.get(rel_path).copied();
        let needs_upload = remote_size != Some(*local_size);
        let pct = 10.0 + (done as f32 / total.max(1) as f32) * 80.0;

        if needs_upload {
            let local_full = export_dir.join(rel_path);
            let remote_full = format!("{remote_path}/{rel_path}");
            let msg = if verbose {
                format!("↑ {rel_path} ({} bytes)", local_size)
            } else {
                format!("↑ {rel_path}")
            };
            let _ = app.emit(
                "sync_progress",
                ProgressEvent {
                    operation: "sync_upload".to_string(),
                    file: rel_path.clone(),
                    percent: pct,
                    message: msg,
                    total_bytes: Some(*local_size),
                    processed_bytes: None,
                },
            );
            upload_file(&sftp, &local_full, &remote_full).await?;
        } else {
            let msg = if verbose {
                format!("  {rel_path} — unchanged ({} bytes)", local_size)
            } else {
                format!("  {rel_path} — skipped")
            };
            let _ = app.emit(
                "sync_progress",
                ProgressEvent {
                    operation: "sync_skip".to_string(),
                    file: rel_path.clone(),
                    percent: pct,
                    message: msg,
                    total_bytes: None,
                    processed_bytes: None,
                },
            );
        }
    }

    for rel_path in remote_files.keys() {
        if !local_files.contains_key(rel_path) {
            let remote_full = format!("{remote_path}/{rel_path}");
            let _ = app.emit(
                "sync_progress",
                ProgressEvent {
                    operation: "sync_delete".to_string(),
                    file: rel_path.clone(),
                    percent: 90.0,
                    message: format!("✕ {rel_path} — deleted from remote"),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );
            let _ = sftp.remove_file(&remote_full).await;
        }
    }

    let _ = app.emit(
        "sync_progress",
        ProgressEvent {
            operation: "complete".to_string(),
            file: String::new(),
            percent: 100.0,
            message: "Sync complete!".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    Ok(())
}

async fn authenticate(
    session: &mut client::Handle<SshClientHandler>,
    username: &str,
    auth: Option<SyncAuth>,
) -> Result<bool> {
    match auth.unwrap_or(SyncAuth::Auto) {
        SyncAuth::Auto => {
            let home = dirs::home_dir().unwrap_or_default();
            let key_names = ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"];
            for name in key_names {
                let path = home.join(".ssh").join(name);
                if !path.exists() {
                    continue;
                }
                if let Ok(key) = russh::keys::load_secret_key(&path, None) {
                    let key_with_alg = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None);
                    if let Ok(result) = session.authenticate_publickey(username, key_with_alg).await
                    {
                        if result.success() {
                            return Ok(true);
                        }
                    }
                }
            }
            Ok(false)
        }
        SyncAuth::Password { password } => session
            .authenticate_password(username, password)
            .await
            .map(|r| r.success())
            .map_err(|e| AppError::Validation(format!("Password auth failed: {e}"))),
        SyncAuth::KeyFile { path, passphrase } => {
            let key = russh::keys::load_secret_key(path, passphrase.as_deref())
                .map_err(|e| AppError::Validation(format!("Failed to load key file: {e}")))?;
            let key_with_alg = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None);
            session
                .authenticate_publickey(username, key_with_alg)
                .await
                .map(|r| r.success())
                .map_err(|e| AppError::Validation(format!("Key file auth failed: {e}")))
        }
    }
}

async fn ensure_remote_dir(sftp: &SftpSession, path: &str) -> Result<()> {
    let _ = sftp.create_dir(path).await;
    Ok(())
}

async fn list_remote_files(sftp: &SftpSession, base: &str) -> Result<HashMap<String, u64>> {
    let mut map = HashMap::new();

    let entries = sftp
        .read_dir(base)
        .await
        .map_err(|e| AppError::Validation(format!("Failed to list remote dir: {e}")))?;

    for entry in entries {
        let name = entry.file_name();
        if entry.metadata().is_dir() {
            let sub = format!("{base}/{name}");
            if let Ok(sub_entries) = sftp.read_dir(&sub).await {
                for sub_entry in sub_entries {
                    let sub_name = sub_entry.file_name();
                    let size = sub_entry.metadata().size.unwrap_or(0);
                    map.insert(format!("{name}/{sub_name}"), size);
                }
            }
        } else {
            let size = entry.metadata().size.unwrap_or(0);
            map.insert(name, size);
        }
    }

    Ok(map)
}

fn list_local_files(base: &Path) -> Result<HashMap<String, u64>> {
    let mut map = HashMap::new();

    let top = std::fs::read_dir(base)
        .map_err(|e| AppError::Validation(format!("Cannot read export dir: {e}")))?;

    for entry in top.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = entry
            .metadata()
            .map_err(|e| AppError::Validation(format!("Cannot stat {name}: {e}")))?;

        if meta.is_dir() {
            let sub_dir = base.join(&name);
            if let Ok(sub) = std::fs::read_dir(&sub_dir) {
                for sub_entry in sub.flatten() {
                    let sub_name = sub_entry.file_name().to_string_lossy().into_owned();
                    let sub_meta = sub_entry.metadata().unwrap_or(meta.clone());
                    if !sub_meta.is_dir() {
                        map.insert(format!("{name}/{sub_name}"), sub_meta.len());
                    }
                }
            }
        } else {
            map.insert(name, meta.len());
        }
    }

    Ok(map)
}

async fn upload_file(sftp: &SftpSession, local: &Path, remote: &str) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let data = tokio::fs::read(local)
        .await
        .map_err(|e| AppError::Validation(format!("Failed to read {local:?}: {e}")))?;

    let mut file = sftp
        .create(remote)
        .await
        .map_err(|e| AppError::Validation(format!("Failed to create remote file {remote}: {e}")))?;

    file.write_all(&data)
        .await
        .map_err(|e| AppError::Validation(format!("Failed to write {remote}: {e}")))?;

    file.flush()
        .await
        .map_err(|e| AppError::Validation(format!("Failed to flush {remote}: {e}")))?;

    Ok(())
}

fn parse_user_host(user_host: &str) -> Result<(String, String)> {
    let mut parts = user_host.splitn(2, '@');
    let user = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Validation("Remote host must be in user@host format".to_string()))?
        .to_string();
    let host = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Validation("Remote host must be in user@host format".to_string()))?
        .to_string();
    Ok((user, host))
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}
