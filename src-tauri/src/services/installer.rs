use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    fs::create_dir_all(&context.mods_path)?;
    fs::create_dir_all(&context.savegames_path)?;
    fs::create_dir_all(&context.backup_path)?;

    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| AppError::Validation(format!("invalid zip archive: {error}")))?;

    let mut report = InstallReport::default();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            AppError::Validation(format!("invalid zip entry at {index}: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let entry_path = PathBuf::from(entry.name());
        match classify_archive_entry(&entry_path) {
            ModFileType::PakMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid pak path in archive: {}", entry.name()))
                })?;
                let destination = context.mods_path.join(file_name);
                if copy_entry_if_changed(&mut entry, &destination)? {
                    report.installed += 1;
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::WorldGenSave => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid save path in archive: {}", entry.name()))
                })?;
                let destination = context.savegames_path.join(file_name);
                if copy_entry_if_changed(&mut entry, &destination)? {
                    report.installed += 1;
                } else {
                    report.skipped += 1;
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

                if copy_entry_if_changed(&mut entry, &destination)? {
                    report.installed += 1;
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::Unknown => {
                report.skipped += 1;
            }
        }
    }

    Ok(report)
}

fn copy_entry_if_changed(entry: &mut zip::read::ZipFile<'_>, destination: &Path) -> Result<bool> {
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
    io::copy(entry, &mut output)?;
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
