use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use russh::client;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncWriteExt;

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

    let mut sorted_files: Vec<(String, u64)> =
        local_files.iter().map(|(k, v)| (k.clone(), *v)).collect();
    sorted_files.sort_by(|(a, _), (b, _)| {
        let a_json = a == "modpack.json";
        let b_json = b == "modpack.json";
        match (a_json, b_json) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_name = a.strip_prefix("mods/").unwrap_or(a.as_str());
                let b_name = b.strip_prefix("mods/").unwrap_or(b.as_str());
                a_name.to_lowercase().cmp(&b_name.to_lowercase())
            }
        }
    });

    let mut skipped: usize = 0;

    for (done, (rel_path, local_size)) in sorted_files.iter().enumerate() {
        let remote_size = remote_files.get(rel_path).copied();
        let needs_upload = remote_size != Some(*local_size);
        let pct = 10.0 + (done as f32 / total.max(1) as f32) * 80.0;
        let display = rel_path.strip_prefix("mods/").unwrap_or(rel_path.as_str());

        if needs_upload {
            let local_full = export_dir.join(rel_path.as_str());
            let remote_full = format!("{remote_path}/{rel_path}");

            let mut local_file = tokio::fs::File::open(&local_full)
                .await
                .map_err(|e| AppError::Validation(format!("Failed to open {local_full:?}: {e}")))?;

            let mut remote_file = sftp.create(&remote_full).await.map_err(|e| {
                AppError::Validation(format!("Failed to create remote file {remote_full}: {e}"))
            })?;

            let upload_start = std::time::Instant::now();
            let mut bytes_written: u64 = 0;
            let mut buf = vec![0u8; 256 * 1024];

            loop {
                use tokio::io::AsyncReadExt;
                let n = local_file.read(&mut buf).await.map_err(|e| {
                    AppError::Validation(format!("Failed to read {local_full:?}: {e}"))
                })?;
                if n == 0 {
                    break;
                }
                remote_file.write_all(&buf[..n]).await.map_err(|e| {
                    AppError::Validation(format!("Failed to write {remote_full}: {e}"))
                })?;
                bytes_written += n as u64;

                let elapsed = upload_start.elapsed().as_secs_f64();
                let speed_str = if elapsed > 0.001 {
                    format!(
                        " - {:.1} MiB/s",
                        (bytes_written as f64 / 1_048_576.0) / elapsed
                    )
                } else {
                    String::new()
                };
                let _ = app.emit(
                    "sync_progress",
                    ProgressEvent {
                        operation: "sync_uploading".to_string(),
                        file: rel_path.clone(),
                        percent: pct,
                        message: format!("Uploading {display}{speed_str}"),
                        total_bytes: Some(*local_size),
                        processed_bytes: Some(bytes_written),
                    },
                );
            }

            remote_file
                .flush()
                .await
                .map_err(|e| AppError::Validation(format!("Failed to flush {remote_full}: {e}")))?;

            let elapsed = upload_start.elapsed().as_secs_f64();
            let speed_str = if elapsed > 0.001 {
                format!(
                    " ({:.1} MiB/s)",
                    (*local_size as f64 / 1_048_576.0) / elapsed
                )
            } else {
                String::new()
            };
            let _ = app.emit(
                "sync_progress",
                ProgressEvent {
                    operation: "sync_upload".to_string(),
                    file: rel_path.clone(),
                    percent: pct,
                    message: format!("Uploaded {display}{speed_str}"),
                    total_bytes: Some(*local_size),
                    processed_bytes: Some(*local_size),
                },
            );
        } else if verbose {
            let _ = app.emit(
                "sync_progress",
                ProgressEvent {
                    operation: "sync_skip".to_string(),
                    file: rel_path.clone(),
                    percent: pct,
                    message: format!("  {display} - skipped"),
                    total_bytes: None,
                    processed_bytes: None,
                },
            );
        } else {
            skipped += 1;
        }
    }

    if !verbose && skipped > 0 {
        emit_progress(&format!("Modpack in sync - {skipped} files matched"), 90.0);
    }

    for rel_path in remote_files.keys() {
        if !local_files.contains_key(rel_path) {
            let remote_full = format!("{remote_path}/{rel_path}");
            let display = rel_path.strip_prefix("mods/").unwrap_or(rel_path.as_str());
            let _ = app.emit(
                "sync_progress",
                ProgressEvent {
                    operation: "sync_delete".to_string(),
                    file: rel_path.clone(),
                    percent: 90.0,
                    message: format!("✕ {display} - deleted from remote"),
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
