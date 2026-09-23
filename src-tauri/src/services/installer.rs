use std::collections::HashSet;
use std::fs;
use std::io::{Chain, Cursor, Read, Seek, SeekFrom, Take, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use unrar::Archive as RarArchive;
use zip::read::ZipFile;
use zip::{CompressionMethod, ZipArchive};

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

/// Stock UE4SS helper mods shipped in the official RE-UE4SS release zip
/// (e.g. `Mods/BPModLoaderMod`, `Mods/ConsoleEnablerMod`). Bundled mod archives
/// that re-package the runtime copy these verbatim alongside the actual mod;
/// we treat them as part of the runtime and skip them so only the user's mod is
/// installed into `Binaries/Win64/ue4ss/Mods/<mod>/`.
const UE4SS_STOCK_MODS: &[&str] = &[
    "ActorDumperMod",
    "BPML_GenericFunctions",
    "BPModLoaderMod",
    "CheatManagerEnablerMod",
    "ConsoleCommandsMod",
    "ConsoleEnablerMod",
    "Keybinds",
    "LineTraceMod",
    "SplitScreenMod",
    "jsbLuaProfilerMod",
];

/// Basenames of UE4SS runtime DLLs that are safe to install on Linux (they run
/// under Proton). Any other `.dll` / `.exe` in an archive is a native Windows
/// C++ mod binary that won't load under Proton and must be blocked.
const ALLOWED_RUNTIME_BINARIES: &[&str] = &["ue4ss.dll", "dwmapi.dll", "xinput1_3.dll"];

/// Scan archive entry names for native Windows binaries (`.dll`, `.exe`) that
/// are NOT part of the allowlisted UE4SS runtime. Returns the list of blocked
/// file paths (as-is from the archive) that are Windows-only.
///
/// Callers should skip this check when the archive is a plain UE4SS runtime
/// archive (`bundles_runtime == true`): those DLLs are allowlisted here, and
/// skipping avoids gating the recursive `ensure_installed` install.
pub fn detect_blocked_native_binaries(entry_names: &[String]) -> Vec<String> {
    let mut blocked = Vec::new();
    for name in entry_names {
        let path = Path::new(name);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let is_blocked_ext = matches!(ext.as_deref(), Some("dll") | Some("exe") | Some("asi"));
        if !is_blocked_ext {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if ALLOWED_RUNTIME_BINARIES.contains(&file_name.as_str()) {
            continue;
        }
        blocked.push(name.clone());
    }
    blocked
}

/// A UE4SS mod archive (Lua/Blueprint script mod, or the UE4SS runtime itself)
/// needs its entries routed to `<game>/ReadyOrNot/Binaries/Win64/...` rather
/// than the usual `~mods` folder. Since that path starts with `ReadyOrNot`,
/// `classify_archive_entry` already treats it as an `Override`, so the rest of
/// the install/symlink/backup machinery needs no changes.
///
/// Mod authors ship these archives in two recurring shapes, both of which are
/// "the official UE4SS zip with the mod dropped into `Mods/` and re-zipped",
/// plus author extras (docs, voice-tool profiles) sitting at the archive root:
///  - `ue4ss/Mods/<mod>/...` next to `dwmapi.dll` (the stock wrapper name), or
///  - `<anything>/Mods/<mod>/...` (e.g. a `package/` folder) with the
///    `UE4SS.dll` runtime file sitting beside `Mods/` inside the wrapper.
///
/// `Ue4ssLayout` carries the info the extractors need to:
///  - strip the wrapper folder before classifying paths
///  - install only the user's mod folder(s) from a bundled archive into the
///    runtime's `ue4ss/Mods/` subtree, dropping author extras at the archive
///    root, the copied UE4SS runtime and the stock helper mods (the pinned
///    runtime comes from `ensure_installed`)
///  - detect whether a user mod is present at all (vs. a plain runtime zip)
#[derive(Debug, Clone)]
pub struct Ue4ssLayout {
    /// Prepended to every entry path before classification.
    pub prefix: PathBuf,
    /// True only for a plain UE4SS runtime archive (official zip or equivalent)
    /// that installs the runtime itself. Used as the recursion guard so
    /// `ensure_installed` doesn't re-trigger itself. False for a mod archive
    /// that merely bundles the runtime - those still need `ensure_installed`
    /// to install the pinned runtime.
    pub bundles_runtime: bool,
    /// True for a mod archive that bundles the UE4SS runtime. Extractors only
    /// install the user's mod folder(s) from such archives, relying on the
    /// pinned UE4SS managed by `ensure_installed`.
    pub bundles_mod: bool,
    /// Path components to strip before classification (the wrapper folder,
    /// e.g. `ue4ss/`, `package/`, or a multi-level path like
    /// `<download_folder>/SwatPathAround_Bundle/Win64/ue4ss` for double-nested
    /// archives). Stored as a `PathBuf` so multi-component wrappers strip
    /// cleanly.
    pub wrapper: Option<PathBuf>,
}

/// Strip `prefix` from `path` case-insensitively, component by component.
/// Returns the remaining path when every prefix component matches (by
/// `eq_ignore_ascii_case`); `None` when the path doesn't start with `prefix`.
fn strip_prefix_ci(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let mut path_comps = path.components();
    let mut prefix_comps = prefix.components();
    loop {
        match prefix_comps.next() {
            Some(p) => {
                let c = path_comps.next()?;
                if !p.as_os_str().eq_ignore_ascii_case(c.as_os_str()) {
                    return None;
                }
            }
            None => return Some(path_comps.as_path().to_path_buf()),
        }
    }
}

/// Inspect an archive's entry names (no extraction needed) and decide whether
/// this is a UE4SS mod archive, and if so, what prefix routes its files into
/// `Binaries/Win64`.
pub fn detect_ue4ss_layout(entry_names: &[String]) -> Option<Ue4ssLayout> {
    let mut is_ue4ss = false;
    let mut has_ue4ss_dll = false;
    let mut has_top_level_mods_dir = false;
    let mut wrapper: Option<PathBuf> = None;

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
            has_ue4ss_dll = true;
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

        // Detect a wrapper folder: a top-level directory whose children are
        // UE4SS runtime files or a `Mods/` tree. The wrapper is usually named
        // `ue4ss` (the stock release layout), but authors re-zip under any
        // name (e.g. `package/`), so match structurally: runtime DLL, runtime
        // settings, or a `Mods/`/`UE4SS_SDK_Backends` subtree at depth 2.
        // A bare mod folder (`<mod>/Scripts/main.lua`, no `Mods/` parent) is
        // NOT a wrapper - it is the mod itself (standalone shape).
        // Runtime files must sit directly under the wrapper (depth 2) so this
        // doesn't mis-fire on deeply-nested archives (e.g. a download-folder
        // name plus `_Bundle/Win64/ue4ss/...`); those are handled by the
        // multi-level detection below.
        if !is_root_level {
            if let Some(first) = first_component {
                let second = path
                    .components()
                    .nth(1)
                    .and_then(|c| c.as_os_str().to_str());
                let depth = path.components().count();
                let is_wrapper = (file_name.eq_ignore_ascii_case("UE4SS.dll")
                    || file_name.eq_ignore_ascii_case("dwmapi.dll")
                    || file_name.eq_ignore_ascii_case("UE4SS-settings.ini"))
                    && depth == 2
                    || second
                        .map(|s| s.eq_ignore_ascii_case("Mods"))
                        .unwrap_or(false)
                    || second
                        .map(|s| s.eq_ignore_ascii_case("UE4SS_SDK_Backends"))
                        .unwrap_or(false);
                if is_wrapper
                    && !(second
                        .map(|s| s.eq_ignore_ascii_case("Scripts"))
                        .unwrap_or(false)
                        && depth == 3)
                {
                    is_ue4ss = true;
                    wrapper = Some(PathBuf::from(first));
                    has_top_level_mods_dir = true;
                }
            }
        }

        // Detect a multi-level wrapper: some authors re-zip the UE4SS runtime
        // under a nested folder structure (e.g. a download-folder name plus a
        // `_Bundle/Win64/ue4ss` subtree). Find the first `Mods/` component at
        // any depth and treat everything before it as the wrapper so the
        // remaining tree reads as the standard experimental layout. Only runs
        // when the single-level detection above didn't already find one.
        if wrapper.is_none() {
            let comps: Vec<_> = path.components().collect();
            for (i, comp) in comps.iter().enumerate().skip(1) {
                if comp.as_os_str().eq_ignore_ascii_case("Mods") {
                    let wrapper_path: PathBuf = comps[..i].iter().collect();
                    wrapper = Some(wrapper_path);
                    has_top_level_mods_dir = true;
                    is_ue4ss = true;
                    break;
                }
            }
        }
    }

    if !is_ue4ss {
        return None;
    }

    // Collect the mods present under `Mods/` (after stripping any wrapper)
    // to decide whether this is a plain runtime zip or a mod archive
    // bundling the runtime, plus bare mod folders at the archive root
    // (`<mod>/Scripts/main.lua`, no `Mods/` parent, no runtime files).
    // Stock helpers and the runtime's own metadata files don't count.
    let mut bare_mod_folder = false;
    let user_mods: Vec<String> = entry_names
        .iter()
        .filter_map(|name| {
            let path = Path::new(name);
            let stripped = match &wrapper {
                Some(w) => strip_prefix_ci(path, w).unwrap_or_else(|| path.to_path_buf()),
                None => path.to_path_buf(),
            };
            let mut comps = stripped.components();
            let first = comps.next()?;
            if first.as_os_str().eq_ignore_ascii_case("Mods") {
                let mod_name = comps.next()?.as_os_str().to_str()?.to_string();
                if UE4SS_STOCK_MODS.contains(&mod_name.as_str())
                    || mod_name == "shared"
                    || mod_name == "mods.txt"
                    || mod_name == "mods.json"
                {
                    None
                } else {
                    Some(mod_name)
                }
            } else {
                // Bare mod folder: `<mod>/Scripts/<...>.lua` or
                // `<mod>/enabled.txt` at depth 2 with no wrapper.
                if wrapper.is_none() && path.components().count() == 3 {
                    let second = stripped.components().nth(1)?.as_os_str().to_str()?;
                    let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
                    if second.eq_ignore_ascii_case("Scripts")
                        && file_name.to_ascii_lowercase().ends_with(".lua")
                        || file_name.eq_ignore_ascii_case("enabled.txt")
                    {
                        bare_mod_folder = true;
                    }
                }
                None
            }
        })
        .collect();

    let bundles_mod = !user_mods.is_empty() || bare_mod_folder;
    // Only a plain runtime archive (no user mod inside) bundles the runtime for
    // install. A mod archive that bundles the runtime still needs
    // `ensure_installed` to supply the pinned runtime.
    let bundles_runtime = has_ue4ss_dll && !bundles_mod;

    // The experimental runtime loads Lua mods from `Win64/ue4ss/Mods/`, so a
    // bare mod folder at the archive root (no `Mods/` parent, no wrapper)
    // must not fall through to the legacy `Win64/Mods` prefix - rewrite() has
    // no `Mods/` segment to re-root under `ue4ss/`. Route it there directly.
    // `bundles_mod` alone is not the signal: a stable-era bundle whose mod
    // sits beside a root-level `UE4SS.dll`/`dwmapi.dll` sets
    // `has_top_level_mods_dir`, and rewrite() re-roots its `Mods/` segment
    // the same way. Only the bare-folder shape needs the direct prefix.
    let bare_mod_folder = bundles_mod && !has_top_level_mods_dir && wrapper.is_none();
    if bare_mod_folder {
        return Some(Ue4ssLayout {
            prefix: PathBuf::from("ReadyOrNot/Binaries/Win64/ue4ss/Mods"),
            bundles_runtime,
            bundles_mod,
            wrapper,
        });
    }

    let prefix = if has_top_level_mods_dir {
        PathBuf::from("ReadyOrNot/Binaries/Win64")
    } else {
        PathBuf::from("ReadyOrNot/Binaries/Win64/Mods")
    };

    Some(Ue4ssLayout {
        prefix,
        bundles_runtime,
        bundles_mod,
        wrapper,
    })
}

/// True if `stripped` (wrapper already removed) belongs to a UE4SS mod that
/// should install: either a user's mod folder (`Mods/<non-stock-name>/...`)
/// or the shared Lua library (`Mods/shared/...`), which every Lua mod needs
/// at runtime ("keep whichever is newer" - handled by newest-wins copy).
/// Stock helper mods and the runtime's own metadata files don't count.
/// The returned path is relative to the `ue4ss/` subtree: the experimental
/// runtime loads Lua mods from `Win64/ue4ss/Mods/`, and `dwmapi.dll` at the
/// Win64 root loads `ue4ss/UE4SS.dll` from beside itself.
fn user_ue4ss_mod_subpath(stripped: &Path) -> Option<PathBuf> {
    let mut comps = stripped.components();
    let first = comps.next().and_then(|c| c.as_os_str().to_str());
    if !first
        .map(|f| f.eq_ignore_ascii_case("Mods"))
        .unwrap_or(false)
    {
        return None;
    }
    let name = comps.next().and_then(|c| c.as_os_str().to_str())?;
    if name.eq_ignore_ascii_case("shared") {
        return Some(PathBuf::from("ue4ss").join(stripped));
    }
    if UE4SS_STOCK_MODS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(name))
        || name.eq_ignore_ascii_case("mods.txt")
        || name.eq_ignore_ascii_case("mods.json")
    {
        return None;
    }
    Some(PathBuf::from("ue4ss").join(stripped))
}

/// Rewrite a UE4SS entry path for extraction, then prepend the layout prefix.
/// Returns `None` when the entry must be skipped (only for `bundles_mod`
/// archives): bundled runtime files, stock helpers, and author extras (docs,
/// voice profiles, images) that sit outside the user's mod folder. The pinned
/// runtime comes from `ensure_installed`.
///
/// Wrapper handling differs by archive kind: for a mod bundle the wrapper
/// (e.g. `ue4ss/`, `package/`) is author packaging and is stripped, and the
/// mod folder is re-rooted under the runtime's `ue4ss/` subtree so it lands
/// at `Win64/ue4ss/Mods/<mod>/` where the experimental loader reads it. A
/// bare mod folder at the archive root carries a `ue4ss/Mods` prefix already
/// (see `detect_ue4ss_layout`) and installs verbatim. For a plain runtime
/// archive the tree IS the install layout (`dwmapi.dll` at the root,
/// everything else under `ue4ss/`), so it is preserved verbatim.
pub fn rewrite_ue4ss_path(raw: &Path, layout: &Ue4ssLayout) -> Option<PathBuf> {
    if layout.bundles_mod {
        // Bare mod folder at the archive root: the prefix already points at
        // `ue4ss/Mods`, so there is no `Mods/` segment to re-root.
        if layout.wrapper.is_none() && layout.prefix.ends_with(Path::new("Win64/ue4ss/Mods")) {
            return Some(layout.prefix.join(raw));
        }
        let stripped: Option<PathBuf> = match &layout.wrapper {
            Some(w) => strip_prefix_ci(raw, w),
            None => None,
        };
        // Mixed bundles (e.g. a UE4SS mod plus a `ReadyOrNot/Content` pak in
        // one archive) carry entries outside the wrapper. Game-rooted ones
        // pass through verbatim so they classify as Overrides/paks normally;
        // anything else falls through to the mod-subpath check below and is
        // skipped (author extras like READMEs).
        if stripped.is_none() {
            let first = raw.components().next().map(|c| c.as_os_str());
            if first
                .map(|f| f == "ReadyOrNot" || f == "_overrides")
                .unwrap_or(false)
            {
                return Some(raw.to_path_buf());
            }
        }
        let stripped = stripped.unwrap_or_else(|| raw.to_path_buf());
        return user_ue4ss_mod_subpath(&stripped).map(|rel| layout.prefix.join(rel));
    }

    Some(layout.prefix.join(raw))
}

/// One selectable UE4SS unit inside an archive (a `Mods/<name>` folder or a
/// `Mods/shared/<sub>` sub-library), with the file count and total
/// uncompressed bytes behind it for the picker UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssModFolder {
    pub name: String,
    pub path: String,
    pub file_count: usize,
    pub size: u64,
}

/// Strip the layout wrapper (if any) from a raw archive entry path.
/// Shared by `rewrite_ue4ss_path` and the mod-folder enumeration so both see
/// the same tree.
fn strip_wrapper(raw: &Path, layout: &Ue4ssLayout) -> PathBuf {
    match &layout.wrapper {
        Some(w) => strip_prefix_ci(raw, w).unwrap_or_else(|| raw.to_path_buf()),
        None => raw.to_path_buf(),
    }
}

/// The selectable install unit for a wrapper-stripped path: a top-level mod
/// folder (`Mods/<name>`, keyed as `<name>`) or a shared-library subfolder
/// (`Mods/shared/<sub>`, keyed as `shared/<sub>`). `Mods/shared/UEHelpers`
/// always installs (every Lua mod needs it) and files directly under
/// `Mods/shared/` ride along with it, so neither is selectable. Stock helpers
/// and non-mod paths return `None`. The bare-mod-folder shape (no `Mods/`
/// segment) keys on its first component.
fn ue4ss_selectable_unit(stripped: &Path, layout: &Ue4ssLayout) -> Option<String> {
    let mut comps = stripped.components();
    let first = comps.next()?.as_os_str().to_str()?;
    if first.eq_ignore_ascii_case("Mods") {
        let name = comps.next()?.as_os_str().to_str()?;
        if UE4SS_STOCK_MODS
            .iter()
            .any(|s| s.eq_ignore_ascii_case(name))
            || name.eq_ignore_ascii_case("mods.txt")
            || name.eq_ignore_ascii_case("mods.json")
        {
            return None;
        }
        if name.eq_ignore_ascii_case("shared") {
            // Shared sub-libraries (e.g. `shared/jsbProfiler`) are
            // individually deselectable; `UEHelpers`, and loose files directly
            // under `shared/`, always install.
            let sub = comps.next()?.as_os_str().to_str()?;
            if sub.eq_ignore_ascii_case("UEHelpers") {
                return None;
            }
            // A loose file directly under `shared/` (no deeper components)
            // is not a sub-library - it rides along with the shared install.
            comps.next()?;
            return Some(format!("shared/{sub}"));
        }
        return Some(name.to_string());
    }
    // Bare mod folder at the archive root: the first component is the mod.
    if layout.wrapper.is_none() && layout.prefix.ends_with(Path::new("Win64/ue4ss/Mods")) {
        return Some(first.to_string());
    }
    None
}

/// Filtered variant of `rewrite_ue4ss_path`: additionally drops entries that
/// belong to a deselected UE4SS unit (mod folder or shared sub-library).
/// `filter` holds the selected unit names (`None` = install all).
/// `shared/UEHelpers`, stock helpers and runtime files are unaffected - they
/// are never selectable.
pub fn rewrite_ue4ss_path_filtered(
    raw: &Path,
    layout: &Ue4ssLayout,
    filter: Option<&HashSet<String>>,
) -> Option<PathBuf> {
    if let Some(selected) = filter {
        if layout.bundles_mod {
            let stripped = strip_wrapper(raw, layout);
            if let Some(unit) = ue4ss_selectable_unit(&stripped, layout) {
                if !selected.contains(&unit) {
                    return None;
                }
            }
        }
    }
    rewrite_ue4ss_path(raw, layout)
}

/// Group an archive's `(entry name, uncompressed size)` pairs into selectable
/// UE4SS units (mod folders plus shared sub-libraries like
/// `shared/jsbProfiler`). Returns an empty vec when the archive is not a mod
/// bundle. `shared/UEHelpers`, stock helpers and runtime metadata are
/// excluded - they always install silently.
pub fn list_ue4ss_mod_folders(entries: &[(String, u64)]) -> Vec<Ue4ssModFolder> {
    let names: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
    let Some(layout) = detect_ue4ss_layout(&names) else {
        return vec![];
    };
    if !layout.bundles_mod {
        return vec![];
    }
    // Unit name -> (unit, file count, bytes), keyed case-insensitively so
    // `SwatPathAround` vs `swatpatharound` can't split rows.
    let mut grouped: std::collections::HashMap<String, (String, usize, u64)> =
        std::collections::HashMap::new();
    for (name, size) in entries {
        let raw = Path::new(name);
        let stripped = strip_wrapper(raw, &layout);
        let Some(unit) = ue4ss_selectable_unit(&stripped, &layout) else {
            continue;
        };
        let key = unit.to_ascii_lowercase();
        // Display path mirrors where the unit lands in-game.
        let display = format!("Mods/{unit}");
        grouped
            .entry(key)
            .and_modify(|(_, count, bytes)| {
                *count += 1;
                *bytes = bytes.saturating_add(*size);
            })
            .or_insert((display, 1, *size));
    }
    let mut folders: Vec<Ue4ssModFolder> = grouped
        .into_values()
        .map(|(path, file_count, size)| {
            let name = path.strip_prefix("Mods/").unwrap_or(&path).to_string();
            Ue4ssModFolder {
                name,
                path,
                file_count,
                size,
            }
        })
        .collect();
    folders.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    folders
}

/// List every file entry's `(name, uncompressed size)` in a zip/rar/7z
/// archive without extracting. Backs both the name-only listing and the
/// UE4SS mod-folder picker (which needs per-folder byte totals).
pub fn list_archive_entries(archive_path: &Path) -> Result<Vec<(String, u64)>> {
    let extension = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    if extension.eq_ignore_ascii_case("zip") {
        let file = fs::File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| AppError::Validation(format!("invalid zip archive: {e}")))?;
        let mut entries = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            let entry = archive
                .by_index(i)
                .map_err(|e| AppError::Validation(format!("zip entry error: {e}")))?;
            if !entry.is_dir() {
                entries.push((entry.name().to_string(), entry.size()));
            }
        }
        return Ok(entries);
    }

    if extension.eq_ignore_ascii_case("rar") {
        let archive = RarArchive::new(archive_path)
            .open_for_listing()
            .map_err(|e| AppError::Validation(format!("failed to open RAR: {e:?}")))?;
        let mut entries = Vec::new();
        for entry_result in archive {
            let entry = entry_result
                .map_err(|e| AppError::Validation(format!("RAR entry error: {e:?}")))?;
            if !entry.is_directory() {
                entries.push((
                    entry.filename.to_string_lossy().to_string(),
                    entry.unpacked_size,
                ));
            }
        }
        return Ok(entries);
    }

    if extension.eq_ignore_ascii_case("7z") {
        let archive = sevenz_rust2::Archive::open(archive_path)
            .map_err(|e| AppError::Validation(format!("failed to open 7z: {e}")))?;
        return Ok(archive
            .files
            .iter()
            .filter(|f| !f.is_directory)
            .map(|f| (f.name.clone(), f.size))
            .collect());
    }

    Ok(vec![])
}

/// List every file entry's name in a zip/rar/7z archive without extracting.
/// Used to detect a UE4SS layout up front - both inside the extractors below
/// and by callers deciding whether to install the UE4SS runtime first.
pub fn list_archive_entry_names(archive_path: &Path) -> Result<Vec<String>> {
    Ok(list_archive_entries(archive_path)?
        .into_iter()
        .map(|(name, _)| name)
        .collect())
}

pub fn install_archive(archive_path: &Path, context: &InstallContext) -> Result<InstallReport> {
    install_archive_with_progress(archive_path, context, |_| {}, None, None)
}

type LzmaEntryDecoder = xz2::read::XzDecoder<Chain<Cursor<Vec<u8>>, Take<fs::File>>>;

enum ZipEntrySource<'a> {
    Zip(ZipFile<'a, fs::File>),
    Lzma(LzmaEntryDecoder),
}

impl Read for ZipEntrySource<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ZipEntrySource::Zip(entry) => entry.read(buf),
            ZipEntrySource::Lzma(decoder) => decoder.read(buf),
        }
    }
}

fn open_lzma_zip_entry(
    archive_path: &Path,
    entry: &ZipFile<'_, fs::File>,
) -> Result<LzmaEntryDecoder> {
    if entry.encrypted() {
        return Err(AppError::Validation(format!(
            "encrypted LZMA zip entry '{}' is not supported",
            entry.name()
        )));
    }

    let data_start = entry.data_start().ok_or_else(|| {
        AppError::Validation(format!(
            "missing data offset for zip entry '{}'",
            entry.name()
        ))
    })?;
    let compressed_size = entry.compressed_size();
    if compressed_size < 9 {
        return Err(AppError::Validation(format!(
            "truncated LZMA zip entry '{}'",
            entry.name()
        )));
    }

    let mut raw = fs::File::open(archive_path)?;
    raw.seek(SeekFrom::Start(data_start))?;

    let mut header = [0u8; 4];
    raw.read_exact(&mut header)?;
    let props_size = u16::from_le_bytes([header[2], header[3]]);
    if props_size != 5 {
        return Err(AppError::Validation(format!(
            "unexpected LZMA properties size of {props_size} in zip entry '{}'",
            entry.name()
        )));
    }
    let mut properties = [0u8; 5];
    raw.read_exact(&mut properties)?;
    let dict_size = u32::from_le_bytes(properties[1..5].try_into().expect("slice is 4 bytes"));

    let uncompressed_size = entry.size();
    let mut lzma_header = Vec::with_capacity(13);
    lzma_header.push(properties[0]);
    lzma_header.extend_from_slice(&dict_size.to_le_bytes());
    lzma_header.extend_from_slice(&uncompressed_size.to_le_bytes());

    let stream = xz2::stream::Stream::new_lzma_decoder(u64::MAX)
        .map_err(|e| AppError::Validation(format!("failed to init LZMA decoder: {e}")))?;

    Ok(xz2::read::XzDecoder::new_stream(
        Cursor::new(lzma_header).chain(raw.take(compressed_size - 9)),
        stream,
    ))
}

pub fn install_archive_with_progress<F>(
    archive_path: &Path,
    context: &InstallContext,
    mut on_progress: F,
    pak_filter: Option<&HashSet<String>>,
    ue4ss_mod_filter: Option<&HashSet<String>>,
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

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            AppError::Validation(format!("invalid zip entry at {index}: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let raw_path = Path::new(entry.name());
        let entry_path = match ue4ss
            .as_ref()
            .and_then(|layout| rewrite_ue4ss_path_filtered(raw_path, layout, ue4ss_mod_filter))
        {
            Some(p) => p,
            None => {
                // Not a UE4SS layout, or a bundled runtime file to skip.
                if ue4ss.is_some() {
                    continue;
                }
                raw_path.to_path_buf()
            }
        };
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
        let entry = archive.by_index(index).map_err(|error| {
            AppError::Validation(format!("invalid zip entry at {index}: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        let raw_path = Path::new(entry.name());
        let entry_name = entry.name().to_string();
        let entry_path = match ue4ss
            .as_ref()
            .and_then(|layout| rewrite_ue4ss_path_filtered(raw_path, layout, ue4ss_mod_filter))
        {
            Some(p) => p,
            None => {
                if ue4ss.is_some() {
                    report.skipped += 1;
                    continue;
                }
                raw_path.to_path_buf()
            }
        };
        let entry_crc32 = entry.crc32();
        let entry_size = entry.size();
        let mut source = match entry.compression() {
            CompressionMethod::Lzma => {
                ZipEntrySource::Lzma(open_lzma_zip_entry(archive_path, &entry)?)
            }
            _ => ZipEntrySource::Zip(entry),
        };

        match classify_archive_entry(&entry_path) {
            ModFileType::PakMod => {
                let file_name = entry_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!("invalid pak path in archive: {}", entry_name))
                })?;
                let filter_key = normalize_archive_path(&entry_path);
                if pak_filter
                    .map(|f| !f.contains(&filter_key))
                    .unwrap_or(false)
                {
                    report.skipped += 1;
                    continue;
                }
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_entry_if_changed_with_progress(
                    &mut source,
                    entry_crc32,
                    &entry_name,
                    &destination,
                    |chunk| {
                        processed_bytes = processed_bytes.saturating_add(chunk);
                        emit_progress(&entry_name, processed_bytes);
                    },
                )? {
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
                    AppError::Validation(format!("invalid save path in archive: {}", entry_name))
                })?;
                fs::create_dir_all(&context.savegames_path)?;
                let destination = context.savegames_path.join(file_name);
                if copy_entry_if_changed_with_progress(
                    &mut source,
                    entry_crc32,
                    &entry_name,
                    &destination,
                    |chunk| {
                        processed_bytes = processed_bytes.saturating_add(chunk);
                        emit_progress(&entry_name, processed_bytes);
                    },
                )? {
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
                    AppError::Validation(format!("invalid bank path in archive: {}", entry_name))
                })?;
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_entry_if_changed_with_progress(
                    &mut source,
                    entry_crc32,
                    &entry_name,
                    &destination,
                    |chunk| {
                        processed_bytes = processed_bytes.saturating_add(chunk);
                        emit_progress(&entry_name, processed_bytes);
                    },
                )? {
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
                    AppError::Validation(format!("invalid ini path in archive: {}", entry_name))
                })?;
                fs::create_dir_all(&context.mods_path)?;
                let destination = context.mods_path.join(file_name);
                if copy_entry_if_changed_with_progress(
                    &mut source,
                    entry_crc32,
                    &entry_name,
                    &destination,
                    |chunk| {
                        processed_bytes = processed_bytes.saturating_add(chunk);
                        emit_progress(&entry_name, processed_bytes);
                    },
                )? {
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

                if copy_entry_if_changed_with_progress(
                    &mut source,
                    entry_crc32,
                    &entry_name,
                    &destination,
                    |chunk| {
                        processed_bytes = processed_bytes.saturating_add(chunk);
                        emit_progress(&entry_name, processed_bytes);
                    },
                )? {
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

    // Some UE4SS mods don't ship the blank `enabled.txt` the loader needs -
    // create it so the mod actually loads.
    ensure_ue4ss_enabled_files(&mut report)?;

    emit_progress("Archive complete", total_bytes);

    Ok(report)
}

pub fn install_rar_archive(
    archive_path: &Path,
    context: &InstallContext,
    temp_root: &Path,
    pak_filter: Option<&HashSet<String>>,
    ue4ss_mod_filter: Option<&HashSet<String>>,
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

        let raw_entry_path = Path::new(&entry_name);
        let rewritten = ue4ss.as_ref().and_then(|layout| {
            rewrite_ue4ss_path_filtered(raw_entry_path, layout, ue4ss_mod_filter)
        });
        let skip_for_bundled_runtime = rewritten.is_none() && ue4ss.is_some();
        if skip_for_bundled_runtime {
            archive = header
                .skip()
                .map_err(|e| AppError::Validation(format!("Failed to skip RAR entry: {:?}", e)))?;
            report.skipped += 1;
            continue;
        }
        let entry_path = match rewritten {
            Some(p) => p,
            None => raw_entry_path.to_path_buf(),
        };
        let temp_file = temp_dir.join(raw_entry_path);

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
                let filter_key = normalize_archive_path(&entry_path);
                if pak_filter
                    .map(|f| !f.contains(&filter_key))
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

    // Some UE4SS mods don't ship the blank `enabled.txt` the loader needs -
    // create it so the mod actually loads.
    ensure_ue4ss_enabled_files(&mut report)?;

    // Clean up temp directory
    let _ = fs::remove_dir_all(temp_dir);

    Ok(report)
}

pub fn install_7z_archive(
    archive_path: &Path,
    context: &InstallContext,
    temp_root: &Path,
    pak_filter: Option<&HashSet<String>>,
    ue4ss_mod_filter: Option<&HashSet<String>>,
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
        let rel_path = match ue4ss
            .as_ref()
            .and_then(|layout| rewrite_ue4ss_path_filtered(&raw_rel_path, layout, ue4ss_mod_filter))
        {
            Some(p) => p,
            None => {
                if ue4ss.is_some() {
                    report.skipped += 1;
                    continue;
                }
                raw_rel_path
            }
        };

        match classify_archive_entry(&rel_path) {
            ModFileType::PakMod => {
                let file_name = rel_path.file_name().ok_or_else(|| {
                    AppError::Validation(format!(
                        "invalid pak path in archive: {}",
                        rel_path.display()
                    ))
                })?;
                let filter_key = normalize_archive_path(&rel_path);
                if pak_filter
                    .map(|f| !f.contains(&filter_key))
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

    // Some UE4SS mods don't ship the blank `enabled.txt` the loader needs -
    // create it so the mod actually loads.
    ensure_ue4ss_enabled_files(&mut report)?;

    let _ = fs::remove_dir_all(&temp_dir);

    Ok(report)
}

fn normalize_archive_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
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
    source: &mut ZipEntrySource<'_>,
    expected_crc32: u32,
    entry_label: &str,
    destination: &Path,
    mut on_chunk: F,
) -> Result<bool>
where
    F: FnMut(u64),
{
    if destination.exists() {
        let current_crc = hasher::crc32_file(destination)?;
        if current_crc == expected_crc32 {
            return Ok(false);
        }
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("failed to create directory {}: {}", parent.display(), e),
            ))
        })?;
    }

    let verify_crc32 = matches!(source, ZipEntrySource::Lzma(_));
    let mut crc32 = crc32fast::Hasher::new();
    let mut output = fs::File::create(destination).map_err(|e| {
        if e.raw_os_error() == Some(17) || e.kind() == std::io::ErrorKind::AlreadyExists {
            AppError::Validation(format!(
                "Failed to write '{}' - file is in use (is the game still running? Close the game and try again): {}",
                destination.display(),
                e
            ))
        } else {
            AppError::Io(e)
        }
    })?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer).map_err(|error| {
            AppError::Validation(format!("failed to extract '{entry_label}': {error}"))
        })?;
        if read == 0 {
            break;
        }
        if verify_crc32 {
            crc32.update(&buffer[..read]);
        }
        output.write_all(&buffer[..read])?;
        on_chunk(read as u64);
    }
    if verify_crc32 && crc32.finalize() != expected_crc32 {
        return Err(AppError::Validation(format!(
            "CRC mismatch while extracting '{entry_label}'"
        )));
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

/// The install dir (`.../ue4ss/Mods/<mod>`) for an installed UE4SS file, or
/// `None` when the file isn't inside a user mod folder. Stock helper mods,
/// the shared Lua library and runtime metadata are not user mods and never
/// need an `enabled.txt`.
fn ue4ss_mod_dir_for_installed_file(path: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = path.components().collect();
    for i in 0..comps.len() {
        let is_ue4ss = comps
            .get(i)
            .and_then(|c| c.as_os_str().to_str())
            .map(|s| s.eq_ignore_ascii_case("ue4ss"))
            .unwrap_or(false);
        let is_mods = comps
            .get(i + 1)
            .and_then(|c| c.as_os_str().to_str())
            .map(|s| s.eq_ignore_ascii_case("Mods"))
            .unwrap_or(false);
        if !(is_ue4ss && is_mods) {
            continue;
        }
        let mod_name = comps.get(i + 2)?.as_os_str().to_str()?;
        if mod_name.eq_ignore_ascii_case("shared")
            || mod_name.eq_ignore_ascii_case("mods.txt")
            || mod_name.eq_ignore_ascii_case("mods.json")
            || UE4SS_STOCK_MODS
                .iter()
                .any(|s| s.eq_ignore_ascii_case(mod_name))
        {
            return None;
        }
        // Installed files are files inside the mod dir, so there must be at
        // least one component beyond `<mod>`.
        if comps.len() <= i + 3 {
            return None;
        }
        return Some(comps[..=i + 2].iter().collect());
    }
    None
}

fn mod_dir_has_enabled_txt(mod_dir: &Path) -> bool {
    if mod_dir.join("enabled.txt").exists() {
        return true;
    }
    // Case-insensitive check so `ENABLED.TXT` from an odd archive doesn't
    // gain a duplicate.
    if let Ok(entries) = fs::read_dir(mod_dir) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_str()
                .map(|n| n.eq_ignore_ascii_case("enabled.txt"))
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

/// Ensure every freshly installed UE4SS user mod has its blank `enabled.txt`.
///
/// Some mods (e.g. MissionObjectiveCounter, SRankAlert) don't ship the file,
/// and without it the UE4SS loader ignores the mod. The file is blank (0
/// bytes) - its presence is the enable signal. Created files are appended to
/// `report.installed_files` so the install manifest tracks them for
/// symlinking.
pub fn ensure_ue4ss_enabled_files(report: &mut InstallReport) -> Result<()> {
    let mut mod_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for file in &report.installed_files {
        if let Some(mod_dir) = ue4ss_mod_dir_for_installed_file(file) {
            mod_dirs.insert(mod_dir);
        }
    }
    // Files installed but skipped as identical (already on disk) aren't in
    // the report, yet their mod dir still counts - rescan is handled by the
    // on-disk check below via `mod_dir_has_enabled_txt`.
    for mod_dir in mod_dirs {
        if mod_dir_has_enabled_txt(&mod_dir) {
            continue;
        }
        fs::create_dir_all(&mod_dir)?;
        let enabled_path = mod_dir.join("enabled.txt");
        fs::write(&enabled_path, b"")?;
        report.installed += 1;
        report.installed_files.push(enabled_path);
    }
    Ok(())
}

/// Repair previously staged installs whose UE4SS mods are missing the blank
/// `enabled.txt` (installed before `ensure_ue4ss_enabled_files` existed).
/// Scans `<staging>/mods/*/ReadyOrNot/Binaries/Win64/ue4ss/Mods/*`, creates
/// any missing file, and returns the created paths so callers can backfill
/// install manifests.
pub fn repair_staged_ue4ss_enabled_files(staging_mods_root: &Path) -> Result<Vec<PathBuf>> {
    let mut created = Vec::new();
    let Ok(keys) = fs::read_dir(staging_mods_root) else {
        return Ok(created);
    };
    for key in keys.flatten() {
        let key_path = key.path();
        if !key_path.is_dir() {
            continue;
        }
        let mods_root = key_path.join("ReadyOrNot/Binaries/Win64/ue4ss/Mods");
        if !mods_root.is_dir() {
            continue;
        }
        let Ok(mods) = fs::read_dir(&mods_root) else {
            continue;
        };
        for entry in mods.flatten() {
            let mod_dir = entry.path();
            if !mod_dir.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.eq_ignore_ascii_case("shared")
                || name_str.eq_ignore_ascii_case("mods.txt")
                || name_str.eq_ignore_ascii_case("mods.json")
                || UE4SS_STOCK_MODS
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(name_str.as_ref()))
            {
                continue;
            }
            if mod_dir_has_enabled_txt(&mod_dir) {
                continue;
            }
            fs::create_dir_all(&mod_dir)?;
            let enabled_path = mod_dir.join("enabled.txt");
            fs::write(&enabled_path, b"")?;
            created.push(enabled_path);
        }
    }
    Ok(created)
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
    fn installs_lzma_compressed_zip_entries() {
        let temp = TempDir::new().unwrap();
        let archive_path = Path::new("tests/fixtures/lzma_pak.zip");
        let context = create_context(temp.path());
        fs::create_dir_all(&context.mods_path).unwrap();
        let report = install_archive(archive_path, &context).unwrap();

        assert_eq!(report.installed, 1);
        let installed = &report.installed_files[0];
        assert_eq!(
            installed.file_name().unwrap(),
            "pakchunk99-Mods_CleanHouse_P.pak"
        );
        let expected = b"pak-bytes-for-lzma-entry".repeat(8);
        assert_eq!(fs::read(installed).unwrap(), expected);
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
        // The experimental loader reads `Win64/ue4ss/Mods/`, so the prefix
        // points there directly (rewrite() installs verbatim).
        let names = vec![
            "RoundReport/enabled.txt".to_string(),
            "RoundReport/Scripts/main.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(
            layout.prefix,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods")
        );
        assert!(!layout.bundles_runtime);

        let rewritten = rewrite_ue4ss_path(Path::new("RoundReport/Scripts/main.lua"), &layout)
            .expect("user mod file should not be skipped");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/RoundReport/Scripts/main.lua")
        );
        assert_eq!(classify_archive_entry(&rewritten), ModFileType::Override);
    }

    #[test]
    fn detect_ue4ss_layout_for_plain_runtime_archive() {
        // A plain stable UE4SS runtime zip - no user mod, just the runtime and
        // stock helpers. `bundles_runtime` is true so the recursion guard
        // kicks in.
        let names = vec![
            "dwmapi.dll".to_string(),
            "UE4SS.dll".to_string(),
            "UE4SS-settings.ini".to_string(),
            "Mods/mods.txt".to_string(),
            "Mods/BPModLoaderMod/Scripts/main.lua".to_string(),
            "Mods/shared/UEHelpers/UEHelpers.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64"));
        assert!(layout.bundles_runtime);
        assert!(!layout.bundles_mod);
        assert!(layout.wrapper.is_none());
    }

    #[test]
    fn detect_ue4ss_layout_for_experimental_runtime_archive() {
        // The experimental rolling release: `dwmapi.dll` at the root,
        // everything else under `ue4ss/`. No user mod, so `bundles_runtime`
        // is true and the full tree installs.
        let names = vec![
            "dwmapi.dll".to_string(),
            "ue4ss/UE4SS.dll".to_string(),
            "ue4ss/UE4SS-settings.ini".to_string(),
            "ue4ss/UE4SS_SDK_Backends/UE4SS.json".to_string(),
            "ue4ss/Mods/mods.txt".to_string(),
            "ue4ss/Mods/mods.json".to_string(),
            "ue4ss/Mods/BPModLoaderMod/Scripts/main.lua".to_string(),
            "ue4ss/Mods/shared/UEHelpers/UEHelpers.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64"));
        assert!(layout.bundles_runtime);
        assert!(!layout.bundles_mod);
        assert_eq!(layout.wrapper.as_deref(), Some(Path::new("ue4ss")));

        // Runtime files route into the ue4ss/ subtree, not the Win64 root.
        let dll = rewrite_ue4ss_path(Path::new("ue4ss/UE4SS.dll"), &layout)
            .expect("runtime DLL should install");
        assert_eq!(dll, Path::new("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS.dll"));
        let ini = rewrite_ue4ss_path(Path::new("ue4ss/UE4SS-settings.ini"), &layout)
            .expect("runtime ini should install");
        assert_eq!(
            ini,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS-settings.ini")
        );
        let shared = rewrite_ue4ss_path(
            Path::new("ue4ss/Mods/shared/UEHelpers/UEHelpers.lua"),
            &layout,
        )
        .expect("shared Lua library should install");
        assert_eq!(
            shared,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
        );
    }

    #[test]
    fn detect_ue4ss_layout_for_runtime_with_user_mod() {
        // A UE4SS runtime zip with a user mod dropped into Mods/ - the runtime
        // is present but `bundles_runtime` is false so `ensure_installed` runs.
        let names = vec![
            "dwmapi.dll".to_string(),
            "UE4SS.dll".to_string(),
            "Mods/mods.txt".to_string(),
            "Mods/BPModLoaderMod/Scripts/main.lua".to_string(),
            "Mods/RoundReport/enabled.txt".to_string(),
            "Mods/RoundReport/Scripts/main.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64"));
        assert!(!layout.bundles_runtime);
        assert!(layout.bundles_mod);

        let rewritten = rewrite_ue4ss_path(Path::new("Mods/RoundReport/Scripts/main.lua"), &layout)
            .expect("user mod file should not be skipped");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/RoundReport/Scripts/main.lua")
        );

        // A nested .ini under a mod's Mods/ tree must stay an Override, not a
        // ConfigMod that would get mis-routed to Saved/Config/Windows.
        let nested_ini = rewrite_ue4ss_path(Path::new("Mods/RoundReport/config.ini"), &layout)
            .expect("user mod file should not be skipped");
        assert_eq!(classify_archive_entry(&nested_ini), ModFileType::Override);

        // Stock helper mods are skipped, but the shared Lua library installs
        // (Lua mods need it at runtime; newest copy wins at link time).
        assert!(
            rewrite_ue4ss_path(Path::new("Mods/BPModLoaderMod/Scripts/main.lua"), &layout)
                .is_none()
        );
        assert!(rewrite_ue4ss_path(Path::new("dwmapi.dll"), &layout).is_none());
        assert!(rewrite_ue4ss_path(Path::new("UE4SS.dll"), &layout).is_none());
        assert!(rewrite_ue4ss_path(Path::new("Mods/mods.txt"), &layout).is_none());
        let shared = rewrite_ue4ss_path(Path::new("Mods/shared/UEHelpers/UEHelpers.lua"), &layout)
            .expect("shared Lua library should install");
        assert_eq!(
            shared,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
        );
    }

    #[test]
    fn detect_ue4ss_layout_for_nested_wrapper_bundle() {
        // RemoteCommander 8721 style: `ue4ss/` wrapper around the runtime +
        // stock helpers + the user mod.
        let names = vec![
            "dwmapi.dll".to_string(),
            "ue4ss/UE4SS.dll".to_string(),
            "ue4ss/UE4SS-settings.ini".to_string(),
            "ue4ss/UE4SS_SDK_Backends/UE4SS.json".to_string(),
            "ue4ss/Mods/mods.json".to_string(),
            "ue4ss/Mods/mods.txt".to_string(),
            "ue4ss/Mods/shared/UEHelpers/UEHelpers.lua".to_string(),
            "ue4ss/Mods/BPModLoaderMod/Scripts/main.lua".to_string(),
            "ue4ss/Mods/RemoteCommander/enabled.txt".to_string(),
            "ue4ss/Mods/RemoteCommander/Scripts/main.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64"));
        assert!(!layout.bundles_runtime);
        assert!(layout.bundles_mod);
        assert_eq!(layout.wrapper.as_deref(), Some(Path::new("ue4ss")));

        // Wrapper is stripped and the user mod routes into the runtime's
        // ue4ss/Mods subtree where the experimental loader reads it.
        let rewritten = rewrite_ue4ss_path(
            Path::new("ue4ss/Mods/RemoteCommander/Scripts/main.lua"),
            &layout,
        )
        .expect("user mod file should not be skipped");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/RemoteCommander/Scripts/main.lua")
        );

        // Bundled runtime files and stock helpers are skipped.
        assert!(rewrite_ue4ss_path(Path::new("ue4ss/UE4SS.dll"), &layout).is_none());
        assert!(rewrite_ue4ss_path(Path::new("dwmapi.dll"), &layout).is_none());
        assert!(
            rewrite_ue4ss_path(Path::new("ue4ss/UE4SS_SDK_Backends/UE4SS.json"), &layout).is_none()
        );
        assert!(rewrite_ue4ss_path(Path::new("ue4ss/Mods/mods.json"), &layout).is_none());
        assert!(rewrite_ue4ss_path(Path::new("ue4ss/Mods/mods.txt"), &layout).is_none());
        assert!(rewrite_ue4ss_path(
            Path::new("ue4ss/Mods/BPModLoaderMod/Scripts/main.lua"),
            &layout
        )
        .is_none());
        // Shared Lua library installs into the runtime subtree (needed at runtime).
        let shared = rewrite_ue4ss_path(
            Path::new("ue4ss/Mods/shared/UEHelpers/UEHelpers.lua"),
            &layout,
        )
        .expect("shared Lua library should install");
        assert_eq!(
            shared,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
        );
    }

    #[test]
    fn detect_ue4ss_layout_nested_wrapper_case_insensitive() {
        // Wrapper name casing varies; detection and strip should be agnostic.
        let names = vec![
            "UE4SS/UE4SS.dll".to_string(),
            "UE4SS/Mods/CustomMod/Scripts/main.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert!(layout.bundles_mod);
        assert_eq!(layout.wrapper.as_deref(), Some(Path::new("UE4SS")));

        let rewritten =
            rewrite_ue4ss_path(Path::new("UE4SS/Mods/CustomMod/Scripts/main.lua"), &layout)
                .expect("user mod file should not be skipped");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/CustomMod/Scripts/main.lua")
        );
    }

    #[test]
    fn detect_ue4ss_layout_for_double_nested_wrapper_bundle() {
        // SwatPathAround 8250 "Bundle" layout: the UE4SS runtime + mod are
        // nested two levels deep inside a download-folder-named wrapper:
        // `<root>/SwatPathAround_Bundle/Win64/ue4ss/Mods/SwatPathAround/...`
        let names = vec![
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/dwmapi.dll"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/UE4SS.dll"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/UE4SS-settings.ini"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Docs/installation-guide.md"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/MemberVarLayoutTemplates/MemberVariableLayout.ini"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/mods.txt"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/mods.json"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/BPModLoaderMod/Scripts/main.lua"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/SwatPathAround/enabled.txt"
                .to_string(),
            "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/SwatPathAround/Scripts/main.lua"
                .to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64"));
        assert!(!layout.bundles_runtime);
        assert!(layout.bundles_mod);
        assert_eq!(
            layout.wrapper.as_deref(),
            Some(Path::new(
                "SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss"
            ))
        );

        // User mod files route into the runtime's ue4ss/Mods subtree.
        let rewritten = rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/SwatPathAround/Scripts/main.lua"),
            &layout,
        )
        .expect("user mod file should not be skipped");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/SwatPathAround/Scripts/main.lua")
        );

        // Shared Lua library installs.
        let shared = rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua"),
            &layout,
        )
        .expect("shared Lua library should install");
        assert_eq!(
            shared,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
        );

        // Bundled runtime files, stock helpers, docs, and template directories
        // are all skipped (not under Mods/, so user_ue4ss_mod_subpath is None).
        assert!(rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/UE4SS.dll"),
            &layout
        )
        .is_none());
        assert!(rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/dwmapi.dll"),
            &layout
        )
        .is_none());
        assert!(rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Docs/installation-guide.md"),
            &layout
        )
        .is_none());
        assert!(rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/MemberVarLayoutTemplates/MemberVariableLayout.ini"),
            &layout
        )
        .is_none());
        assert!(rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/BPModLoaderMod/Scripts/main.lua"),
            &layout
        )
        .is_none());
        assert!(rewrite_ue4ss_path(
            Path::new("SwatPathAround Bundle 8250 1 2026 uwrWHLEPO/SwatPathAround_Bundle/Win64/ue4ss/Mods/mods.txt"),
            &layout
        )
        .is_none());
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
    fn pak_filter_matches_full_path_not_basename() {
        // Simulates a FOMOD "choose one" archive (e.g. Holo Clarity): three variants
        // share the identical basename but live in different nested folders.
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.mods_path).unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![
                ("Mod/Variants/Green/shared.pak", b"green"),
                ("Mod/Variants/Red/shared.pak", b"red"),
                ("Mod/Variants/Blue/shared.pak", b"blue"),
            ],
        );

        let filter =
            std::collections::HashSet::from([String::from("Mod/Variants/Blue/shared.pak")]);
        let report =
            install_archive_with_progress(&archive, &context, |_| {}, Some(&filter), None).unwrap();

        assert_eq!(report.installed, 1);
        assert_eq!(report.skipped, 2);
        assert_eq!(
            fs::read(context.mods_path.join("shared.pak")).unwrap(),
            b"blue",
            "installed variant must be the user's choice, not walk-order"
        );
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
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/RoundReport/Scripts/main.lua");
        assert!(installed_lua.exists());
        assert_eq!(fs::read(installed_lua).unwrap(), b"-- lua code");
    }

    #[test]
    fn install_skips_bundled_runtime_in_nested_wrapper_archive() {
        // RemoteCommander 8721 layout: `ue4ss/` wrapper around the runtime,
        // stock helpers, shared Lua lib and the user mod. The user mod and
        // shared lib install; the bundled runtime and stock helpers are skipped.
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.game_path).unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![
                // Bundled runtime - skipped.
                ("dwmapi.dll", b"runtime-dwmapi"),
                ("ue4ss/UE4SS.dll", b"runtime-dll"),
                ("ue4ss/UE4SS-settings.ini", b"runtime-settings"),
                ("ue4ss/UE4SS_SDK_Backends/UE4SS.json", b"runtime-sdk"),
                // Stock helpers under Mods/ - skipped.
                ("ue4ss/Mods/mods.json", b"{}"),
                ("ue4ss/Mods/mods.txt", b"mods"),
                ("ue4ss/Mods/shared/UEHelpers/UEHelpers.lua", b"-- shared"),
                ("ue4ss/Mods/BPModLoaderMod/Scripts/main.lua", b"-- stock"),
                (
                    "ue4ss/Mods/ConsoleEnablerMod/Scripts/main.lua",
                    b"-- stock2",
                ),
                // User mod - installed.
                ("ue4ss/Mods/RemoteCommander/enabled.txt", b""),
                (
                    "ue4ss/Mods/RemoteCommander/Scripts/main.lua",
                    b"-- remote commander",
                ),
            ],
        );

        let report = install_archive(&archive, &context).unwrap();

        assert_eq!(
            report.installed, 3,
            "user mod files + shared Lua lib should install"
        );
        assert_eq!(report.skipped, 8, "runtime + stock helpers skipped");

        let installed_lua = context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/RemoteCommander/Scripts/main.lua");
        assert!(installed_lua.exists());
        assert_eq!(fs::read(installed_lua).unwrap(), b"-- remote commander");

        let installed_enabled = context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/RemoteCommander/enabled.txt");
        assert!(installed_enabled.exists());

        // Shared Lua library installs into the runtime subtree; stock helpers
        // and runtime do not.
        assert!(context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
            .exists());
        assert!(!context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/BPModLoaderMod/Scripts/main.lua")
            .exists());
        assert!(!context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/UE4SS.dll")
            .exists());
    }

    #[test]
    fn detect_ue4ss_layout_for_package_wrapper_with_root_extras() {
        // VoiceCommander 8353 layout: a `package/` wrapper holding the runtime
        // and the user mod, plus author extras (docs, voice profiles) at the
        // archive root. Wrapper name is structural, not hard-coded.
        let names = vec![
            "LICENSE".to_string(),
            "LICENSE-UE4SS.txt".to_string(),
            "Ready or Not Voice Commander v1.1-Profile.vap".to_string(),
            "Ready or Not Voice Commander v1.1.xml".to_string(),
            "RoNVoiceCommander.png".to_string(),
            "install.html".to_string(),
            "release_notes.html".to_string(),
            "voice_commands.html".to_string(),
            "package/UE4SS.dll".to_string(),
            "package/UE4SS-settings.ini".to_string(),
            "package/dwmapi.dll".to_string(),
            "package/Mods/shared/UEHelpers/UEHelpers.lua".to_string(),
            "package/Mods/VoiceCommanderMod/enabled.txt".to_string(),
            "package/Mods/VoiceCommanderMod/Scripts/main.lua".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert_eq!(layout.prefix, Path::new("ReadyOrNot/Binaries/Win64"));
        assert!(!layout.bundles_runtime);
        assert!(layout.bundles_mod);
        assert_eq!(layout.wrapper.as_deref(), Some(Path::new("package")));

        // The user's mod routes into the runtime's ue4ss/Mods subtree;
        // everything else is skipped.
        let rewritten = rewrite_ue4ss_path(
            Path::new("package/Mods/VoiceCommanderMod/Scripts/main.lua"),
            &layout,
        )
        .expect("user mod file should not be skipped");
        assert_eq!(
            rewritten,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/VoiceCommanderMod/Scripts/main.lua")
        );
        assert!(rewrite_ue4ss_path(Path::new("package/UE4SS.dll"), &layout).is_none());
        assert!(rewrite_ue4ss_path(Path::new("package/dwmapi.dll"), &layout).is_none());
        assert!(rewrite_ue4ss_path(Path::new("LICENSE"), &layout).is_none());
        // Shared Lua library installs into the runtime subtree (needed at runtime).
        let shared = rewrite_ue4ss_path(
            Path::new("package/Mods/shared/UEHelpers/UEHelpers.lua"),
            &layout,
        )
        .expect("shared Lua library should install");
        assert_eq!(
            shared,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
        );
    }

    #[test]
    fn install_installs_only_mod_folder_from_package_wrapper_archive() {
        // VoiceCommander 8353 layout end to end: the user mod plus the shared
        // Lua library install; everything else is skipped.
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.game_path).unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![
                ("LICENSE", b"license"),
                ("LICENSE-UE4SS.txt", b"license ue4ss"),
                (
                    "Ready or Not Voice Commander v1.1-Profile.vap",
                    b"voice profile",
                ),
                ("Ready or Not Voice Commander v1.1.xml", b"voice commands"),
                ("RoNVoiceCommander.png", b"png"),
                ("install.html", b"docs"),
                ("release_notes.html", b"docs"),
                ("voice_commands.html", b"docs"),
                ("package/UE4SS.dll", b"runtime-dll"),
                ("package/UE4SS-settings.ini", b"runtime-settings"),
                ("package/dwmapi.dll", b"runtime-dwmapi"),
                ("package/Mods/shared/UEHelpers/UEHelpers.lua", b"-- shared"),
                ("package/Mods/VoiceCommanderMod/enabled.txt", b""),
                (
                    "package/Mods/VoiceCommanderMod/Scripts/main.lua",
                    b"-- voice commander",
                ),
            ],
        );

        let report = install_archive(&archive, &context).unwrap();

        assert_eq!(
            report.installed, 3,
            "user mod files + shared Lua lib should install"
        );
        assert_eq!(report.skipped, 11, "runtime + extras skipped");

        let installed_lua = context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/VoiceCommanderMod/Scripts/main.lua");
        assert!(installed_lua.exists());
        assert_eq!(fs::read(installed_lua).unwrap(), b"-- voice commander");

        let installed_enabled = context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/VoiceCommanderMod/enabled.txt");
        assert!(installed_enabled.exists());

        assert!(context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
            .exists());
    }

    #[test]
    fn detect_blocked_native_binaries_flags_mod_dll() {
        // SWAT Unleashed "ModOnly" layout: Lua mod + native C++ DLL under
        // `Mods/<mod>/dlls/`. The DLL is Windows-only and must be blocked.
        let names = vec![
            "ReadyOrNot/Binaries/Win64/Mods/RonCommandNative/enabled.txt".to_string(),
            "ReadyOrNot/Binaries/Win64/Mods/RonCommandNative/dlls/main.dll".to_string(),
            "ReadyOrNot/Binaries/Win64/Mods/RonCommandNative/dlls/SWATEnhanced.ini".to_string(),
            "ReadyOrNot/Binaries/Win64/Mods/RonAIHooks/Scripts/main.lua".to_string(),
            "ReadyOrNot/Binaries/Win64/Mods/RonAIHooks/enabled.txt".to_string(),
            "ReadyOrNot/Content/Paks/zzz_RonAI_TacticalFix_V1_P.pak".to_string(),
        ];
        let blocked = detect_blocked_native_binaries(&names);
        assert_eq!(
            blocked,
            vec!["ReadyOrNot/Binaries/Win64/Mods/RonCommandNative/dlls/main.dll"]
        );
    }

    #[test]
    fn detect_blocked_native_binaries_allows_runtime_dlls() {
        // Plain UE4SS runtime archive: `UE4SS.dll`, `dwmapi.dll`,
        // `xinput1_3.dll` are allowlisted and must NOT be blocked.
        let names = vec![
            "dwmapi.dll".to_string(),
            "UE4SS.dll".to_string(),
            "UE4SS-settings.ini".to_string(),
            "xinput1_3.dll".to_string(),
        ];
        assert!(detect_blocked_native_binaries(&names).is_empty());
    }

    #[test]
    fn detect_blocked_native_binaries_handles_case_variants() {
        // Basename comparison is case-insensitive.
        let names = vec![
            "Mods/SomeMod/dlls/MAIN.DLL".to_string(),
            "Mods/SomeMod/dlls/Main.Dll".to_string(),
            "package/UE4SS.DLL".to_string(),
        ];
        let blocked = detect_blocked_native_binaries(&names);
        assert_eq!(
            blocked,
            vec!["Mods/SomeMod/dlls/MAIN.DLL", "Mods/SomeMod/dlls/Main.Dll"]
        );
    }

    #[test]
    fn detect_blocked_native_binaries_flags_exe_and_asi() {
        // `.exe` installers and `.asi` script loader DLLs are also blocked.
        let names = vec![
            "installer.exe".to_string(),
            "re-shade/asi/loader.asi".to_string(),
            "Mods/ModName/Scripts/main.lua".to_string(),
        ];
        let blocked = detect_blocked_native_binaries(&names);
        assert_eq!(blocked, vec!["installer.exe", "re-shade/asi/loader.asi"]);
    }

    #[test]
    fn detect_blocked_native_binaries_empty_for_pure_pak_mod() {
        let names = vec!["maps/cool_mod.pak".to_string(), "readme.txt".to_string()];
        assert!(detect_blocked_native_binaries(&names).is_empty());
    }

    fn swat_path_around_bundle_entries() -> Vec<(String, u64)> {
        let prefix = "DL/SwatPathAround_Bundle/Win64/ue4ss";
        vec![
            (format!("{prefix}/UE4SS.dll"), 100),
            (format!("{prefix}/Docs/guide.md"), 10),
            (format!("{prefix}/Mods/mods.txt"), 5),
            (format!("{prefix}/Mods/BPModLoaderMod/Scripts/main.lua"), 20),
            (format!("{prefix}/Mods/shared/UEHelpers/UEHelpers.lua"), 30),
            (format!("{prefix}/Mods/shared/Types.lua"), 40),
            (format!("{prefix}/Mods/shared/jsbProfiler/jsbProfi.lua"), 50),
            (format!("{prefix}/Mods/SwatPathAround/enabled.txt"), 0),
            (format!("{prefix}/Mods/SwatPathAround/Scripts/main.lua"), 60),
        ]
    }

    #[test]
    fn list_ue4ss_mod_folders_groups_mods_and_shared_subs() {
        let entries = swat_path_around_bundle_entries();
        let folders = list_ue4ss_mod_folders(&entries);
        // Only the user mod and the deselectable shared sub-library are
        // listed: UEHelpers, loose shared files, stock helpers, runtime
        // files, docs and metadata stay silent.
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].name, "shared/jsbProfiler");
        assert_eq!(folders[0].path, "Mods/shared/jsbProfiler");
        assert_eq!(folders[0].file_count, 1);
        assert_eq!(folders[0].size, 50);
        assert_eq!(folders[1].name, "SwatPathAround");
        assert_eq!(folders[1].path, "Mods/SwatPathAround");
        assert_eq!(folders[1].file_count, 2);
        assert_eq!(folders[1].size, 60);
    }

    #[test]
    fn list_ue4ss_mod_folders_empty_for_non_bundle() {
        let entries = vec![
            ("maps/cool_mod.pak".to_string(), 100),
            ("readme.txt".to_string(), 10),
        ];
        assert!(list_ue4ss_mod_folders(&entries).is_empty());
    }

    #[test]
    fn list_ue4ss_mod_folders_empty_for_plain_runtime() {
        let entries = vec![
            ("dwmapi.dll".to_string(), 100),
            ("ue4ss/UE4SS.dll".to_string(), 200),
            ("ue4ss/Mods/shared/UEHelpers/UEHelpers.lua".to_string(), 30),
        ];
        assert!(list_ue4ss_mod_folders(&entries).is_empty());
    }

    #[test]
    fn rewrite_filtered_drops_deselected_units_but_keeps_shared_lib() {
        let entries = swat_path_around_bundle_entries();
        let names: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert!(layout.bundles_mod);
        let selected: HashSet<String> = HashSet::from(["SwatPathAround".to_string()]);

        // Deselected shared sub-library is skipped.
        assert!(rewrite_ue4ss_path_filtered(
            Path::new("DL/SwatPathAround_Bundle/Win64/ue4ss/Mods/shared/jsbProfiler/jsbProfi.lua"),
            &layout,
            Some(&selected),
        )
        .is_none());
        // Selected user mod installs.
        let installed = rewrite_ue4ss_path_filtered(
            Path::new("DL/SwatPathAround_Bundle/Win64/ue4ss/Mods/SwatPathAround/Scripts/main.lua"),
            &layout,
            Some(&selected),
        )
        .expect("selected mod should install");
        assert_eq!(
            installed,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/SwatPathAround/Scripts/main.lua")
        );
        // UEHelpers is never selectable, so the filter can't drop it.
        let shared = rewrite_ue4ss_path_filtered(
            Path::new("DL/SwatPathAround_Bundle/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua"),
            &layout,
            Some(&selected),
        )
        .expect("shared lib should always install");
        assert_eq!(
            shared,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
        );
        // No filter installs everything, as before.
        assert!(rewrite_ue4ss_path_filtered(
            Path::new("DL/SwatPathAround_Bundle/Win64/ue4ss/Mods/shared/jsbProfiler/jsbProfi.lua"),
            &layout,
            None,
        )
        .is_some());
    }

    #[test]
    fn rewrite_passes_through_non_wrapper_overrides_in_mixed_bundle() {
        // SWAT Unleashed 8753 shape: UE4SS mods under a nested Win64 tree plus
        // a ReadyOrNot/Content pak outside the wrapper.
        let names = vec![
            "ReadyOrNot/Binaries/Win64/Mods/RonCommandNative/enabled.txt".to_string(),
            "ReadyOrNot/Binaries/Win64/Mods/RonAIHooks/Scripts/main.lua".to_string(),
            "ReadyOrNot/Content/Paks/zzz_RonAI_TacticalFix_V1_P.pak".to_string(),
            "README.txt".to_string(),
        ];
        let layout = detect_ue4ss_layout(&names).expect("should detect UE4SS layout");
        assert!(layout.bundles_mod);

        // The pak passes through verbatim so it classifies as an Override.
        let pak = rewrite_ue4ss_path(
            Path::new("ReadyOrNot/Content/Paks/zzz_RonAI_TacticalFix_V1_P.pak"),
            &layout,
        )
        .expect("pak outside wrapper should pass through");
        assert_eq!(
            pak,
            Path::new("ReadyOrNot/Content/Paks/zzz_RonAI_TacticalFix_V1_P.pak")
        );
        assert_eq!(classify_archive_entry(&pak), ModFileType::Override);

        // Mod folders still re-root into the runtime subtree.
        let lua = rewrite_ue4ss_path(
            Path::new("ReadyOrNot/Binaries/Win64/Mods/RonAIHooks/Scripts/main.lua"),
            &layout,
        )
        .expect("user mod should install");
        assert_eq!(
            lua,
            Path::new("ReadyOrNot/Binaries/Win64/ue4ss/Mods/RonAIHooks/Scripts/main.lua")
        );

        // Author extras at the root are still skipped.
        assert!(rewrite_ue4ss_path(Path::new("README.txt"), &layout).is_none());
    }

    #[test]
    fn install_archive_respects_ue4ss_mod_filter() {
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.game_path).unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![
                ("Mods/AlphaMod/enabled.txt", b""),
                ("Mods/AlphaMod/Scripts/main.lua", b"-- alpha"),
                ("Mods/BetaMod/enabled.txt", b""),
                ("Mods/BetaMod/Scripts/main.lua", b"-- beta"),
                ("Mods/shared/UEHelpers/UEHelpers.lua", b"-- shared"),
            ],
        );

        let filter = HashSet::from(["AlphaMod".to_string()]);
        let report =
            install_archive_with_progress(&archive, &context, |_| {}, None, Some(&filter)).unwrap();

        assert_eq!(report.installed, 3, "AlphaMod + shared lib install");
        assert_eq!(report.skipped, 2, "BetaMod files skipped");
        assert!(context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/AlphaMod/Scripts/main.lua")
            .exists());
        assert!(context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua")
            .exists());
        assert!(!context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/BetaMod")
            .exists());
    }

    #[test]
    fn install_creates_missing_enabled_txt_for_ue4ss_mod() {
        // MissionObjectiveCounter / SRankAlert shape: Lua files but no
        // enabled.txt. The loader needs the blank file, so install creates it.
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        fs::create_dir_all(&context.game_path).unwrap();

        let archive = create_test_archive(
            temp.path(),
            vec![
                ("MissionObjectiveCounter/Scripts/main.lua", b"-- lua code"),
                ("SRankAlert/Scripts/main.lua", b"-- lua code"),
            ],
        );

        let report = install_archive(&archive, &context).unwrap();

        for mod_name in ["MissionObjectiveCounter", "SRankAlert"] {
            let enabled = context.game_path.join(format!(
                "ReadyOrNot/Binaries/Win64/ue4ss/Mods/{mod_name}/enabled.txt"
            ));
            assert!(enabled.exists(), "{mod_name} should gain enabled.txt");
            assert_eq!(fs::read(&enabled).unwrap().len(), 0, "enabled.txt is blank");
            assert!(report.installed_files.contains(&enabled));
        }
    }

    #[test]
    fn install_does_not_duplicate_existing_enabled_txt() {
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

        assert_eq!(
            report.installed, 2,
            "no extra enabled.txt should be created"
        );
        let enabled_count = report
            .installed_files
            .iter()
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.eq_ignore_ascii_case("enabled.txt"))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(enabled_count, 1);
    }

    #[test]
    fn ensure_enabled_txt_skips_shared_and_stock_mods() {
        let temp = TempDir::new().unwrap();
        let context = create_context(temp.path());
        let shared_file = context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/shared/UEHelpers/UEHelpers.lua");
        let stock_file = context
            .game_path
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/BPModLoaderMod/Scripts/main.lua");
        fs::create_dir_all(shared_file.parent().unwrap()).unwrap();
        fs::create_dir_all(stock_file.parent().unwrap()).unwrap();
        fs::write(&shared_file, b"-- shared").unwrap();
        fs::write(&stock_file, b"-- stock").unwrap();

        let mut report = InstallReport {
            installed: 2,
            skipped: 0,
            overrides_backed_up: 0,
            installed_files: vec![shared_file, stock_file],
        };
        ensure_ue4ss_enabled_files(&mut report).unwrap();

        assert_eq!(report.installed, 2, "shared/stock mods need no enabled.txt");
        assert_eq!(report.installed_files.len(), 2);
    }

    #[test]
    fn repair_staged_creates_missing_enabled_txt() {
        let temp = TempDir::new().unwrap();
        let staging_mods = temp.path().join("mods");
        let mod_scripts =
            staging_mods.join("SomeMod/ReadyOrNot/Binaries/Win64/ue4ss/Mods/SRankAlert/Scripts");
        fs::create_dir_all(&mod_scripts).unwrap();
        fs::write(mod_scripts.join("main.lua"), b"-- lua").unwrap();

        let created = repair_staged_ue4ss_enabled_files(&staging_mods).unwrap();

        assert_eq!(created.len(), 1);
        assert_eq!(fs::read(&created[0]).unwrap().len(), 0);
        assert!(created[0].ends_with("SRankAlert/enabled.txt"));

        // Second run is a no-op.
        assert!(repair_staged_ue4ss_enabled_files(&staging_mods)
            .unwrap()
            .is_empty());
    }
}
