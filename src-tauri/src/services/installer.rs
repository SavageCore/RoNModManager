use std::collections::HashSet;
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
    BankMod,
    ConfigMod,
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
        .map(|component| {
            let c = component.as_os_str();
            c == "_overrides" || c == "ReadyOrNot"
        })
        .unwrap_or(false)
    {
        return ModFileType::Override;
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("pak") => ModFileType::PakMod,
        Some(ext) if ext.eq_ignore_ascii_case("sav") => ModFileType::WorldGenSave,
        Some(ext) if ext.eq_ignore_ascii_case("bank") => ModFileType::BankMod,
        Some(ext) if ext.eq_ignore_ascii_case("ini") => ModFileType::ConfigMod,
        _ => ModFileType::Unknown,
    }
}

/// A UE4SS mod archive (Lua/Blueprint script mod, or the UE4SS runtime itself)
/// needs its entries routed to `<game>/ReadyOrNot/Binaries/Win64/...` rather
/// than the usual `~mods` folder. Since that path starts with `ReadyOrNot`,
/// `classify_archive_entry` already treats it as an `Override`, so the rest of
/// the install/symlink/backup machinery needs no changes.
#[derive(Debug, Clone)]
pub struct Ue4ssLayout {
    /// Prepended to every entry path before classification.
    pub prefix: PathBuf,
    /// True if the archive already ships `UE4SS.dll` (the runtime itself, or a
    /// mod bundled together with it) - used as the recursion guard so
    /// installing UE4SS doesn't re-trigger installing UE4SS.
    pub bundles_runtime: bool,
}

/// Inspect an archive's entry names (no extraction needed) and decide whether
/// this is a UE4SS mod archive, and if so, what prefix routes its files into
/// `Binaries/Win64`.
pub fn detect_ue4ss_layout(entry_names: &[String]) -> Option<Ue4ssLayout> {
    let mut is_ue4ss = false;
    let mut bundles_runtime = false;
    let mut has_top_level_mods_dir = false;

    for name in entry_names {
        let path = Path::new(name);
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let is_root_level = path
            .parent()
            .map(|p| p.as_os_str().is_empty())
            .unwrap_or(true);
        let first_component = path
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str());
        let parent_dir_name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|f| f.to_str());

        if file_name.eq_ignore_ascii_case("enabled.txt") {
            is_ue4ss = true;
        }
        if file_name.eq_ignore_ascii_case("UE4SS.dll") {
            is_ue4ss = true;
            bundles_runtime = true;
            if is_root_level {
                has_top_level_mods_dir = true;
            }
        }
        if file_name.eq_ignore_ascii_case("dwmapi.dll") && is_root_level {
            is_ue4ss = true;
            has_top_level_mods_dir = true;
        }
        if file_name.to_ascii_lowercase().ends_with(".lua")
            && parent_dir_name
                .map(|p| p.eq_ignore_ascii_case("Scripts"))
                .unwrap_or(false)
        {
            is_ue4ss = true;
        }
        if first_component
            .map(|c| c.eq_ignore_ascii_case("Mods"))
            .unwrap_or(false)
        {
            has_top_level_mods_dir = true;
        }
    }

    if !is_ue4ss {
        return None;
    }

    let prefix = if has_top_level_mods_dir {
        PathBuf::from("ReadyOrNot/Binaries/Win64")
    } else {
        PathBuf::from("ReadyOrNot/Binaries/Win64/Mods")
    };

    Some(Ue4ssLayout {
        prefix,
        bundles_runtime,
    })
}

/// List every file entry's name in a zip/rar/7z archive without extracting.
/// Used to detect a UE4SS layout up front - both inside the extractors below
/// and by callers deciding whether to install the UE4SS runtime first.
pub fn list_archive_entry_names(archive_path: &Path) -> Result<Vec<String>> {
    let extension = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    if extension.eq_ignore_ascii_case("zip") {
        let file = fs::File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| AppError::Validation(format!("invalid zip archive: {e}")))?;
        let mut names = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            let entry = archive
                .by_index(i)
                .map_err(|e| AppError::Validation(format!("zip entry error: {e}")))?;
            if !entry.is_dir() {
                names.push(entry.name().to_string());
            }
        }
        return Ok(names);
    }

    if extension.eq_ignore_ascii_case("rar") {
        let archive = RarArchive::new(archive_path)
            .open_for_listing()
            .map_err(|e| AppError::Validation(format!("failed to open RAR: {e:?}")))?;
        let mut names = Vec::new();
        for entry_result in archive {
            let entry = entry_result
                .map_err(|e| AppError::Validation(format!("RAR entry error: {e:?}")))?;
            if !entry.is_directory() {
                names.push(entry.filename.to_string_lossy().to_string());
            }
        }
        return Ok(names);
    }

    if extension.eq_ignore_ascii_case("7z") {
        let archive = sevenz_rust2::Archive::open(archive_path)
            .map_err(|e| AppError::Validation(format!("failed to open 7z: {e}")))?;
        return Ok(archive
            .files
            .iter()
            .filter(|f| !f.is_directory)
            .map(|f| f.name.clone())
            .collect());
    }

    Ok(vec![])
}

pub fn install_archive(archive_path: &Path, context: &InstallContext) -> Result<InstallReport> {
    install_archive_with_progress(archive_path, context, |_| {}, None)
}

pub fn install_archive_with_progress<F>(
    archive_path: &Path,
    context: &InstallContext,
    mut on_progress: F,
    pak_filter: Option<&HashSet<String>>,
) -> Result<InstallReport>
where
    F: FnMut(ArchiveProgress),
{
    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| AppError::Validation(format!("invalid zip archive: {error}")))?;

    let mut report = InstallReport::default();
    let mut total_bytes = 0u64;

    let mut entry_names: Vec<String> = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            AppError::Validation(format!("invalid zip entry at {index}: {error}"))
        })?;
        if !entry.is_dir() {
            entry_names.push(entry.name().to_string());
        }
    }
    let ue4ss = detect_ue4ss_layout(&entry_names);
    let rewrite_entry_path = |path: PathBuf| -> PathBuf {
        match &ue4ss {
            Some(layout) => layout.prefix.join(path),
            None => path,
        }
    };

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            AppError::Validation(format!("invalid zip entry at {index}: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let entry_path = rewrite_entry_path(PathBuf::from(entry.name()));
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

        let entry_path = rewrite_entry_path(PathBuf::from(entry.name()));
        let entry_name = entry.name().to_string();
        match classify_archive_entry(&entry_path) {
            ModFileType::PakMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid pak path in archive: {}", entry.name()))
                })?;
                if pak_filter
                    .map(|f| !f.contains(file_name.to_string_lossy().as_ref()))
                    .unwrap_or(false)
                {
                    report.skipped += 1;
                    continue;
                }
                fs::create_dir_all(&context.mods_path)?;
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
                // Always install .sav files to savegames_path
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid save path in archive: {}", entry.name()))
                })?;
                fs::create_dir_all(&context.savegames_path)?;
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
            ModFileType::BankMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid bank path in archive: {}", entry.name()))
                })?;
                fs::create_dir_all(&context.mods_path)?;
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
            ModFileType::ConfigMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid ini path in archive: {}", entry.name()))
                })?;
                fs::create_dir_all(&context.mods_path)?;
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
            ModFileType::Override => {
                let override_relative = if entry_path.starts_with("_overrides") {
                    entry_path.strip_prefix("_overrides").map_err(|_| {
                        AppError::Validation(format!(
                            "invalid override path in archive: {}",
                            entry.name()
                        ))
                    })?
                } else {
                    &entry_path
                };

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

pub fn install_rar_archive(
    archive_path: &Path,
    context: &InstallContext,
    temp_root: &Path,
    pak_filter: Option<&HashSet<String>>,
) -> Result<InstallReport> {
    let mut report = InstallReport::default();

    let entry_names = list_archive_entry_names(archive_path)?;
    let ue4ss = detect_ue4ss_layout(&entry_names);

    // Create temporary directory for extraction
    let temp_dir = temp_root.join(format!(
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

        let raw_entry_path = PathBuf::from(&entry_name);
        let entry_path = match &ue4ss {
            Some(layout) => layout.prefix.join(&raw_entry_path),
            None => raw_entry_path.clone(),
        };
        let temp_file = temp_dir.join(&raw_entry_path);

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
                if pak_filter
                    .map(|f| !f.contains(file_name.to_string_lossy().as_ref()))
                    .unwrap_or(false)
                {
                    report.skipped += 1;
                    continue;
                }
                fs::create_dir_all(&context.mods_path)?;
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
                fs::create_dir_all(&context.savegames_path)?;
                let destination = context.savegames_path.join(file_name);
                if copy_file_if_changed(&temp_file, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::BankMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid bank path in archive: {}", entry_name))
                })?;
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_file_if_changed(&temp_file, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::ConfigMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid ini path in archive: {}", entry_name))
                })?;
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_file_if_changed(&temp_file, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::Override => {
                let override_relative = if entry_path.starts_with("_overrides") {
                    entry_path.strip_prefix("_overrides").map_err(|_| {
                        AppError::Validation(format!(
                            "invalid override path in archive: {}",
                            entry_name
                        ))
                    })?
                } else {
                    &entry_path
                };

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

pub fn install_7z_archive(
    archive_path: &Path,
    context: &InstallContext,
    temp_root: &Path,
    pak_filter: Option<&HashSet<String>>,
) -> Result<InstallReport> {
    let mut report = InstallReport::default();

    let temp_dir = temp_root.join(format!(
        "ronmod_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));
    fs::create_dir_all(&temp_dir)?;

    let entry_names = list_archive_entry_names(archive_path)?;
    let ue4ss = detect_ue4ss_layout(&entry_names);

    let src = archive_path.to_string_lossy().to_string();
    let dest = temp_dir.to_string_lossy().to_string();
    sevenz_rust2::decompress_file(&src, &dest)
        .map_err(|e| AppError::Validation(format!("Failed to open 7z archive: {e}")))?;

    for abs_path in walk_files(&temp_dir) {
        let raw_rel_path = abs_path
            .strip_prefix(&temp_dir)
            .unwrap_or(abs_path.as_path())
            .to_path_buf();
        let rel_path = match &ue4ss {
            Some(layout) => layout.prefix.join(&raw_rel_path),
            None => raw_rel_path,
        };

        match classify_archive_entry(&rel_path) {
            ModFileType::PakMod => {
                let file_name = rel_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!(
                        "invalid pak path in archive: {}",
                        rel_path.display()
                    ))
                })?;
                if pak_filter
                    .map(|f| !f.contains(file_name.to_string_lossy().as_ref()))
                    .unwrap_or(false)
                {
                    report.skipped += 1;
                    continue;
                }
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_file_if_changed(&abs_path, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::WorldGenSave => {
                let file_name = rel_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!(
                        "invalid save path in archive: {}",
                        rel_path.display()
                    ))
                })?;
                fs::create_dir_all(&context.savegames_path)?;
                let destination = context.savegames_path.join(file_name);
                if copy_file_if_changed(&abs_path, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::BankMod => {
                let file_name = rel_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!(
                        "invalid bank path in archive: {}",
                        rel_path.display()
                    ))
                })?;
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_file_if_changed(&abs_path, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::ConfigMod => {
                let file_name = rel_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!(
                        "invalid ini path in archive: {}",
                        rel_path.display()
                    ))
                })?;
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_file_if_changed(&abs_path, &destination)? {
                    report.installed += 1;
                    report.installed_files.push(destination);
                } else {
                    report.skipped += 1;
                }
            }
            ModFileType::Override => {
                let override_relative = if rel_path.starts_with("_overrides") {
                    rel_path.strip_prefix("_overrides").map_err(|_| {
                        AppError::Validation(format!(
                            "invalid override path in archive: {}",
                            rel_path.display()
                        ))
                    })?
                } else {
                    &rel_path
                };

                if override_relative.as_os_str().is_empty() {
                    report.skipped += 1;
                    continue;
                }

                let destination = context.game_path.join(override_relative);
                if destination.exists() {
                    backup_existing_file(&destination, &context.backup_path)?;
                    report.overrides_backed_up += 1;
                }

                if copy_file_if_changed(&abs_path, &destination)? {
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

    let _ = fs::remove_dir_all(&temp_dir);

    Ok(report)
}

fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk_files(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
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

pub fn backup_existing_file(source: &Path, backup_root: &Path) -> Result<()> {
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
            classify_archive_entry(Path::new("ReadyOrNot/Content/Movies/foo.mp4")),
            ModFileType::Override
        );
        assert_eq!(
            classify_archive_entry(Path::new("readme.txt")),
            ModFileType::Unknown
        );
    }

    #[test]
    fn detect_ue4ss_layout_for_standalone_lua_mod() {
        // e.g. "RoundReport-1.0.zip" - the mod folder sits at the archive root.
        let names = vec![
            "RoundReport/enabled.txt".to_string(),
            "RoundReport/Scripts/main.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64/Mods"));
        assert!(!layout.bundles_runtime);

        let rewritten = layout.prefix.join("RoundReport/Scripts/main.lua");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/Mods/RoundReport/Scripts/main.lua")
        );
        assert_eq!(classify_archive_entry(&rewritten), ModFileType::Override);
    }

    #[test]
    fn detect_ue4ss_layout_for_bundled_runtime() {
        // e.g. "RoundReport-1.0-with-UE4SS-3.0.1.zip" - a full UE4SS drop.
        let names = vec![
            "dwmapi.dll".to_string(),
            "UE4SS.dll".to_string(),
            "Mods/mods.txt".to_string(),
            "Mods/RoundReport/enabled.txt".to_string(),
            "Mods/RoundReport/Scripts/main.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64"));
        assert!(layout.bundles_runtime);

        let rewritten = layout.prefix.join("Mods/RoundReport/Scripts/main.lua");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/Mods/RoundReport/Scripts/main.lua")
        );

        // A nested .ini under a mod's Mods/ tree must stay an Override, not a ConfigMod
        // that would get mis-routed to Saved/Config/Windows.
        let nested_ini = layout.prefix.join("Mods/RoundReport/config.ini");
        assert_eq!(classify_archive_entry(&nested_ini), ModFileType::Override);
    }

    #[test]
    fn detect_ue4ss_layout_returns_none_for_ordinary_pak_mod() {
        let names = vec!["nested/a_mod.pak".to_string(), "readme.txt".to_string()];
        assert!(detect_ue4ss_layout(&names).is_none());
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
    fn install_readyornot_rooted_override_backs_up_and_replaces() {
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());

        let existing_target = context
            .game_path
            .join("ReadyOrNot")
            .join("Content")
            .join("Movies")
            .join("RoNLogo.mp4");
        fs::create_dir_all(existing_target.parent().unwrap()).unwrap();
        fs::write(&existing_target, b"original").unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![("ReadyOrNot/Content/Movies/RoNLogo.mp4", b"replacement")],
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

    #[test]
    fn install_routes_ue4ss_lua_mod_into_binaries_win64() {
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.game_path).unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![
                ("RoundReport/enabled.txt", b""),
                ("RoundReport/Scripts/main.lua", b"-- lua code"),
            ],
        );

        let report = install_archive(&archive, &context).unwrap();

        assert_eq!(report.installed, 2);
        let installed_lua = context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/Mods/RoundReport/Scripts/main.lua");
        assert!(installed_lua.exists());
        assert_eq!(fs::read(installed_lua).unwrap(), b"-- lua code");
    }
}
