use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use unrar::Archive as RarArchive;
use zip::ZipArchive;

use crate::models::{AppError, Result};
use crate::services::hasher;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModFileType {
    PakMod,
    WorldGenSave,
    Override,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct InstallContext {
    pub game_path: PathBuf,
    pub mods_path: PathBuf,
    pub savegames_path: PathBuf,
    pub backup_path: PathBuf,
}

#[derive(Debug, Default, Clone)]
pub struct InstallReport {
    pub installed: usize,
    pub skipped: usize,
    pub overrides_backed_up: usize,
    pub installed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ArchiveProgress {
    pub file: String,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

pub fn classify_archive_entry(path: &Path) -> ModFileType {
    if path
        .components()
        .next()
        .map(|component| component.as_os_str() == "_overrides")
        .unwrap_or(false)
    {
        return ModFileType::Override;
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("pak") => ModFileType::PakMod,
        Some("sav") => ModFileType::WorldGenSave,
        _ => ModFileType::Unknown,
    }
}

pub fn install_archive(archive_path: &Path, context: &InstallContext) -> Result<InstallReport> {
    install_archive_with_progress(archive_path, context, |_| {})
}

pub fn install_archive_with_progress<F>(
    archive_path: &Path,
    context: &InstallContext,
    mut on_progress: F,
) -> Result<InstallReport>
where
    F: FnMut(ArchiveProgress),
{
    fs::create_dir_all(&context.mods_path)?;

    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| AppError::Validation(format!("invalid zip archive: {error}")))?;

    let mut report = InstallReport::default();
    let mut total_bytes = 0u64;

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            AppError::Validation(format!("invalid zip entry at {index}: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let entry_path = PathBuf::from(entry.name());
        if classify_archive_entry(&entry_path) != ModFileType::Unknown {
            total_bytes = total_bytes.saturating_add(entry.size());
        }
    }

    let mut processed_bytes = 0u64;

    let mut emit_progress = |file: &str, processed: u64| {
        let percent = if total_bytes == 0 {
            100.0
        } else {
            (processed as f32 / total_bytes as f32 * 100.0).min(100.0)
        };
        on_progress(ArchiveProgress {
            file: file.to_string(),
            processed_bytes: processed,
            total_bytes,
            percent,
        });
    };

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            AppError::Validation(format!("invalid zip entry at {index}: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let entry_path = PathBuf::from(entry.name());
        let entry_name = entry.name().to_string();
        match classify_archive_entry(&entry_path) {
            ModFileType::PakMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid pak path in archive: {}", entry.name()))
                })?;
                let destination = context.mods_path.join(file_name);
                let entry_size = entry.size();
                if copy_entry_if_changed_with_progress(&mut entry, &destination, |chunk| {
                    processed_bytes = processed_bytes.saturating_add(chunk);
                    emit_progress(&entry_name, processed_bytes);
                })? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                    processed_bytes = processed_bytes.saturating_add(entry_size);
                    emit_progress(&entry_name, processed_bytes);
                }
            }
            ModFileType::WorldGenSave => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid save path in archive: {}", entry.name()))
                })?;
                let destination = context.savegames_path.join(file_name);
                let entry_size = entry.size();
                if copy_entry_if_changed_with_progress(&mut entry, &destination, |chunk| {
                    processed_bytes = processed_bytes.saturating_add(chunk);
                    emit_progress(&entry_name, processed_bytes);
                })? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                    processed_bytes = processed_bytes.saturating_add(entry_size);
                    emit_progress(&entry_name, processed_bytes);
                }
            }
            ModFileType::Override => {
                let override_relative = entry_path.strip_prefix("_overrides").map_err(|_| {
                    AppError::Validation(format!(
                        "invalid override path in archive: {}",
                        entry.name()
                    ))
                })?;

                if override_relative.as_os_str().is_empty() {
                    report.skipped += 1;
                    continue;
                }

                let destination = context.game_path.join(override_relative);
                if destination.exists() {
                    backup_existing_file(&destination, &context.backup_path)?;
                    report.overrides_backed_up += 1;
                }

                let entry_size = entry.size();
                if copy_entry_if_changed_with_progress(&mut entry, &destination, |chunk| {
                    processed_bytes = processed_bytes.saturating_add(chunk);
                    emit_progress(&entry_name, processed_bytes);
                })? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                    processed_bytes = processed_bytes.saturating_add(entry_size);
                    emit_progress(&entry_name, processed_bytes);
                }
            }
            ModFileType::Unknown => {
                report.skipped += 1;
            }
        }
    }

    emit_progress("Archive complete", total_bytes);

    Ok(report)
}

pub fn install_rar_archive(archive_path: &Path, context: &InstallContext) -> Result<InstallReport> {
    fs::create_dir_all(&context.mods_path)?;

    let mut report = InstallReport::default();

    // Create temporary directory for extraction
    let temp_dir = std::env::temp_dir().join(format!(
        "ronmod_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));
    fs::create_dir_all(&temp_dir)?;

    // Extract RAR archive to temp directory
    let mut archive = RarArchive::new(archive_path)
        .open_for_processing()
        .map_err(|e| AppError::Validation(format!("Failed to open RAR archive: {:?}", e)))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| AppError::Validation(format!("Failed to read RAR header: {:?}", e)))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();

        if header.entry().is_directory() {
            archive = header
                .skip()
                .map_err(|e| AppError::Validation(format!("Failed to skip RAR entry: {:?}", e)))?;
            continue;
        }

        let entry_path = PathBuf::from(&entry_name);
        let temp_file = temp_dir.join(&entry_path);

        if let Some(parent) = temp_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // Extract file to temp location
        archive = header.extract_to(&temp_file).map_err(|e| {
            AppError::Validation(format!(
                "Failed to extract RAR entry '{}': {:?}",
                entry_name, e
            ))
        })?;

        // Process the extracted file based on type
        match classify_archive_entry(&entry_path) {
            ModFileType::PakMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid pak path in archive: {}", entry_name))
                })?;
                let destination = context.mods_path.join(file_name);
                if copy_file_if_changed(&temp_file, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::WorldGenSave => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid save path in archive: {}", entry_name))
                })?;
                let destination = context.savegames_path.join(file_name);
                if copy_file_if_changed(&temp_file, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::Override => {
                let override_relative = entry_path.strip_prefix("_overrides").map_err(|_| {
                    AppError::Validation(format!(
                        "invalid override path in archive: {}",
                        entry_name
                    ))
                })?;

                if override_relative.as_os_str().is_empty() {
                    report.skipped += 1;
                    continue;
                }

                let destination = context.game_path.join(override_relative);
                if destination.exists() {
                    backup_existing_file(&destination, &context.backup_path)?;
                    report.overrides_backed_up += 1;
                }

                if copy_file_if_changed(&temp_file, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::Unknown => {
                report.skipped += 1;
            }
        }
    }

    // Clean up temp directory
    let _ = fs::remove_dir_all(temp_dir);

    Ok(report)
}

fn copy_file_if_changed(source: &Path, destination: &Path) -> Result<bool> {
    let needs_copy = if destination.exists() {
        let source_crc = hasher::crc32_file(source)?;
        let dest_crc = hasher::crc32_file(destination)?;
        source_crc != dest_crc
    } else {
        true
    };

    if needs_copy {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn copy_entry_if_changed_with_progress<F>(
    entry: &mut zip::read::ZipFile<'_, std::fs::File>,
    destination: &Path,
    mut on_chunk: F,
) -> Result<bool>
where
    F: FnMut(u64),
{
    if destination.exists() {
        let current_crc = hasher::crc32_file(destination)?;
        if current_crc == entry.crc32() {
            return Ok(false);
        }
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut output = fs::File::create(destination)?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = entry.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        on_chunk(read as u64);
    }
    Ok(true)
}

fn backup_existing_file(source: &Path, backup_root: &Path) -> Result<()> {
    let relative_name = source
        .to_string_lossy()
        .replace(['/', '\\'], "__")
        .trim_start_matches("__")
        .to_string();

    let backup_file_name = format!("{}_backup", relative_name);
    let destination = backup_root.join(backup_file_name);

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;
    use zip::write::FileOptions;

    use super::*;

    fn create_context(root: &Path) -> InstallContext {
        InstallContext {
            game_path: root.join("game"),
            mods_path: root.join("mods"),
            savegames_path: root.join("savegames"),
            backup_path: root.join("backups"),
        }
    }

    fn create_test_archive(root: &Path, entries: Vec<(&str, &[u8])>) -> PathBuf {
        let archive_path = root.join("test.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: FileOptions<'_, ()> = FileOptions::default();

        for (name, bytes) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }

        zip.finish().unwrap();
        archive_path
    }

    #[test]
    fn classify_entries() {
        assert_eq!(
            classify_archive_entry(Path::new("maps/cool_mod.pak")),
            ModFileType::PakMod
        );
        assert_eq!(
            classify_archive_entry(Path::new("SaveGames/world.sav")),
            ModFileType::WorldGenSave
        );
        assert_eq!(
            classify_archive_entry(Path::new("_overrides/ReadyOrNot/Config.ini")),
            ModFileType::Override
        );
        assert_eq!(
            classify_archive_entry(Path::new("readme.txt")),
            ModFileType::Unknown
        );
    }

    #[test]
    fn install_extracts_pak_and_save_files() {
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.game_path).unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![
                ("nested/a_mod.pak", b"pak-content"),
                ("deep/world.sav", b"save-content"),
                ("readme.txt", b"ignored"),
            ],
        );

        let report = install_archive(&archive, &context).unwrap();

        assert_eq!(report.installed, 2);
        assert_eq!(report.skipped, 1);
        assert!(context.mods_path.join("a_mod.pak").exists());
        assert!(context.savegames_path.join("world.sav").exists());
    }

    #[test]
    fn install_overrides_and_creates_backup() {
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());

        let existing_target = context
            .game_path
            .join("ReadyOrNot")
            .join("Content")
            .join("file.txt");
        fs::create_dir_all(existing_target.parent().unwrap()).unwrap();
        fs::write(&existing_target, b"original").unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![("_overrides/ReadyOrNot/Content/file.txt", b"replacement")],
        );

        let report = install_archive(&archive, &context).unwrap();
        let replaced = fs::read(&existing_target).unwrap();

        assert_eq!(report.installed, 1);
        assert_eq!(report.overrides_backed_up, 1);
        assert_eq!(replaced, b"replacement");
        assert!(fs::read_dir(&context.backup_path).unwrap().next().is_some());
    }

    #[test]
    fn install_skips_identical_file_by_crc32() {
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.mods_path).unwrap();
        fs::write(context.mods_path.join("same.pak"), b"same-bytes").unwrap();

        let archive = create_test_archive(temp.path(), vec![("same.pak", b"same-bytes")]);

        let report = install_archive(&archive, &context).unwrap();
        assert_eq!(report.installed, 0);
        assert_eq!(report.skipped, 1);
    }
}
