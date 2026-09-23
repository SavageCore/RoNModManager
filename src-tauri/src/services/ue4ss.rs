use std::path::Path;

use emmylua_parser::{LuaLanguageLevel, LuaParser, ParserConfig};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::models::{ProgressEvent, Result};
use crate::services::{downloader, installer};
use crate::state::app_data_root;

/// Managed UE4SS runtime: the `experimental-latest` rolling release, which is
/// the only build that launches Ready or Not reliably (stock v3.0.1 crashes
/// on startup). Its layout differs from stable: `dwmapi.dll` sits at the
/// Win64 root, everything else lives under `ue4ss/`:
///
/// ```text
/// Win64/dwmapi.dll
/// Win64/ue4ss/UE4SS.dll
/// Win64/ue4ss/UE4SS-settings.ini
/// Win64/ue4ss/Mods/<mod>/...
/// ```
///
/// The asset filename embeds a commit hash (`UE4SS_v3.0.1-1133-gb4cefa18.zip`)
/// that changes on every upstream push, so it is resolved at install time via
/// the GitHub releases API and kept verbatim as the archive name.
const UE4SS_RELEASE_TAG: &str = "experimental-latest";
const UE4SS_RELEASE_API: &str =
    "https://api.github.com/repos/UE4SS-RE/RE-UE4SS/releases/tags/experimental-latest";
/// Prefix of the usable runtime asset. Excludes `zDEV-...` (debug build) and
/// `zCustomGameConfigs`/`zMapGenBP` (unrelated tooling).
const UE4SS_ASSET_PREFIX: &str = "UE4SS_v3.0.1-";
/// Prefix identifying any managed runtime archive, stable or experimental.
/// `get_installed_mod_groups` hides these from the mod list (silent
/// dependency); sync keeps linking them from the profile's enabled list.
pub(crate) const UE4SS_ARCHIVE_PREFIX: &str = "UE4SS_";

/// True when `archive_name` is a managed UE4SS runtime archive (any build).
pub(crate) fn is_managed_archive(archive_name: &str) -> bool {
    let stem = archive_name.strip_suffix(".zip").unwrap_or(archive_name);
    stem.starts_with(UE4SS_ARCHIVE_PREFIX)
}

/// Win64-relative path of the runtime DLL in the experimental layout.
pub(crate) const RUNTIME_DLL: &str = "ReadyOrNot/Binaries/Win64/ue4ss/UE4SS.dll";
/// Win64-relative path of the runtime settings in the experimental layout.
pub(crate) const RUNTIME_INI: &str = "ReadyOrNot/Binaries/Win64/ue4ss/UE4SS-settings.ini";

/// True if UE4SS's core DLL is already present in the live game's `ue4ss/`
/// folder (as a real file, or a managed symlink pointing into the staged
/// install).
pub fn is_installed(game_path: &Path) -> bool {
    game_path.join(RUNTIME_DLL).exists()
}

/// Which console the UE4SS debug output uses. The stock ini ships GUI mode
/// on (`ConsoleEnabled = 0, GuiConsoleEnabled = 1, GuiConsoleVisible = 0`);
/// text mode flips `ConsoleEnabled` on and the GUI pair off; none turns all
/// three off for a quiet game with no console at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ue4ssConsoleMode {
    Text,
    Gui,
    None,
}

impl Ue4ssConsoleMode {
    fn triple(self) -> (bool, bool, bool) {
        match self {
            // (ConsoleEnabled, GuiConsoleEnabled, GuiConsoleVisible)
            Ue4ssConsoleMode::Text => (true, false, false),
            Ue4ssConsoleMode::Gui => (false, true, true),
            Ue4ssConsoleMode::None => (false, false, false),
        }
    }
}

/// Crash-safe settings applied to the staged `UE4SS-settings.ini` after every
/// runtime install, per the unofficial modding guide's UE4SS page: the
/// upstream default `bUseUObjectArrayCache = true` crashes Ready or Not on
/// startup, and stale UE4SS 2.x `xinput1_3.dll` shims crash alongside the
/// newer loader. The guide's working set is:
///
/// ```ini
/// bUseUObjectArrayCache = false
/// MajorVersion = 5
/// MinorVersion = 3
/// GraphicsAPI = dx11
/// HookBeginPlay = 0
/// ```
///
/// Surgical: only these keys are touched, comments, ordering and line endings
/// preserved.
const CRASH_FIX_KEY: &str = "bUseUObjectArrayCache";
const STALE_SHIM: &str = "xinput1_3.dll";
/// Ready or Not runs on UE 5.3 - pin the engine version override so UE4SS
/// doesn't misdetect it (stock ini leaves both blank).
pub const GUIDE_MAJOR_VERSION: &str = "5";
pub const GUIDE_MINOR_VERSION: &str = "3";
/// Stock ini uses `opengl`; the guide's working value is `dx11`.
pub const GUIDE_GRAPHICS_API: &str = "dx11";
/// Stock ini hooks BeginPlay (`1`); the guide disables it (`0`).
pub const GUIDE_HOOK_BEGIN_PLAY: &str = "0";

/// Graphics API values UE4SS accepts for `GraphicsAPI` (`[Debug]` section).
pub const GRAPHICS_API_OPTIONS: &[&str] = &["dx11", "dx12", "vulkan", "opengl"];

/// Current UE4SS settings read from the staged ini: crash fixes + console mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssSettings {
    pub use_object_array_cache: bool,
    pub engine_major_version: String,
    pub engine_minor_version: String,
    pub graphics_api: String,
    pub hook_begin_play: bool,
    pub console_mode: Ue4ssConsoleMode,
    pub settings_present: bool,
}

impl Ue4ssSettings {
    /// The guide's working defaults for a fresh install.
    pub fn guide_defaults() -> Self {
        Ue4ssSettings {
            use_object_array_cache: false,
            engine_major_version: GUIDE_MAJOR_VERSION.to_string(),
            engine_minor_version: GUIDE_MINOR_VERSION.to_string(),
            graphics_api: GUIDE_GRAPHICS_API.to_string(),
            hook_begin_play: false,
            console_mode: Ue4ssConsoleMode::Gui,
            settings_present: true,
        }
    }

    /// Stock (unpatched) values: cache on, blank engine version, opengl,
    /// BeginPlay hooked. Used when the runtime isn't installed.
    fn stock_defaults() -> Self {
        Ue4ssSettings {
            use_object_array_cache: true,
            engine_major_version: String::new(),
            engine_minor_version: String::new(),
            graphics_api: "opengl".to_string(),
            hook_begin_play: true,
            console_mode: Ue4ssConsoleMode::Gui,
            settings_present: false,
        }
    }
}

/// Parse one `0`/`1`/`true`/`false` ini value (case-insensitive).
fn parse_ini_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Read `key = value` for `section` from ini text. Ignores comment lines
/// (`;`/`#`), is case-insensitive on section and key, and returns the last
/// match (later duplicates win, matching UE4SS behavior).
fn read_ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    let mut found = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('[') {
            in_section = header
                .split(']')
                .next()
                .map(|s| s.eq_ignore_ascii_case(section))
                .unwrap_or(false);
            continue;
        }
        if !in_section {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let name = parts.next()?.trim();
        let value = parts.next()?.trim();
        if name.eq_ignore_ascii_case(key) {
            found = Some(value.to_string());
        }
    }
    found
}

/// Set (or append) `key = value` inside `section`, preserving everything else
/// byte-for-byte, including comments and CRLF line endings. Appends a missing
/// section (or key) at the end of the file.
fn set_ini_value(text: &str, section: &str, key: &str, value: &str) -> String {
    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut section_found = false;
    let mut key_written = false;

    for line in text.split(line_ending) {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // Leaving the target section without having written the key:
            // append it as the section's last line.
            if in_section && !key_written {
                out.push(format!("{key} = {value}"));
                key_written = true;
            }
            in_section = trimmed
                .strip_prefix('[')
                .and_then(|h| h.split(']').next())
                .map(|s| s.eq_ignore_ascii_case(section))
                .unwrap_or(false);
            if in_section {
                section_found = true;
            }
            out.push(line.to_string());
            continue;
        }
        let is_target_key = in_section
            && !key_written
            && !trimmed.is_empty()
            && !trimmed.starts_with(';')
            && !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .map(|(name, _)| name.trim().eq_ignore_ascii_case(key))
                .unwrap_or(false);
        if is_target_key {
            out.push(format!("{key} = {value}"));
            key_written = true;
            continue;
        }
        out.push(line.to_string());
    }

    if !key_written {
        if section_found {
            // Target section was last in the file: append the key at the end.
            out.push(format!("{key} = {value}"));
        } else {
            // No such section at all: append both.
            if !out.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
                out.push(String::new());
            }
            out.push(format!("[{section}]"));
            out.push(format!("{key} = {value}"));
        }
    }

    out.join(line_ending)
}

/// Path of the ini to read/patch: the staged copy behind the live
/// `Win64/ue4ss/UE4SS-settings.ini` symlink, or the staged copy directly. The
/// live folder can't be trusted on its own - with link-on-launch-only it is
/// stock at rest, so the live ini is gone while the runtime is still
/// installed. Falls back to the staged copy under the managed runtime's
/// install key (stable or experimental archive stem), or `None` when the
/// runtime isn't installed at all.
fn staged_settings_path(game_path: &Path) -> Option<std::path::PathBuf> {
    let live = game_path.join(RUNTIME_INI);
    if live.exists() {
        match std::fs::read_link(&live) {
            Ok(target) => {
                let resolved = if target.is_absolute() {
                    target
                } else {
                    live.parent().unwrap_or(Path::new("")).join(target)
                };
                if resolved.exists() {
                    return Some(resolved);
                }
            }
            // Real file, not a symlink (manual install): patch it in place.
            Err(_) => return Some(live),
        }
    }
    // Live folder is stock (or the symlink is dangling): use the staged copy
    // of whichever managed runtime archive is installed.
    staged_runtime_ini()
}

/// Staged ini of the installed managed runtime, if any.
fn staged_runtime_ini() -> Option<std::path::PathBuf> {
    let mods = app_data_root().ok()?.join("staged").join("mods");
    let entries = std::fs::read_dir(&mods).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Archive name -> install key: file stem, matching `archive_install_key`.
        let key = name.strip_suffix(".zip").unwrap_or(&name);
        if !is_managed_archive(&name) && !is_managed_archive(key) {
            continue;
        }
        let ini = mods
            .join(entry.file_name())
            .join("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS-settings.ini");
        if ini.exists() {
            return Some(ini);
        }
        // Stable-layout staged copy (pre-migration): still patchable.
        let legacy = mods
            .join(entry.file_name())
            .join("ReadyOrNot/Binaries/Win64/UE4SS-settings.ini");
        if legacy.exists() {
            return Some(legacy);
        }
    }
    None
}

/// Win64-root files from the stable v3.0.1 layout that must go when moving to
/// experimental. The `Mods/` leftovers land in two places depending on which
/// layout installed them: `Win64/Mods/<stock>` (stable runtime, the old
/// code's wrapper-stripping path) or `Win64/ue4ss/Mods/<stock>`
/// (experimental runtime archives, which keep the subtree verbatim).
/// `dwmapi.dll` always stays - both layouts need it at the Win64 root.
///
/// Live symlinks/copies, the stable staged key, and the stable
/// manifest/archive/profile entries are removed. The live root-`Mods/` sweep
/// stays gated on looking like runtime residue (never a user's mods); the
/// stable staged key is runtime-owned wholesale so it needs no gating - and
/// gating it on live state would be racy under link-on-launch-only, where
/// the live folder is stock at rest while residue still sits staged.
fn remove_stable_leftovers(game_path: &Path) {
    const ROOT_FILES: &[&str] = &[
        "ReadyOrNot/Binaries/Win64/UE4SS.dll",
        "ReadyOrNot/Binaries/Win64/UE4SS-settings.ini",
        "ReadyOrNot/Binaries/Win64/README.md",
        "ReadyOrNot/Binaries/Win64/Changelog.md",
    ];
    const ROOT_MODS: &[&str] = &[
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
        "mods.txt",
    ];
    // `Win64/ue4ss/`-relative residue from experimental runtime archives:
    // stock helper mods + runtime metadata that a mod bundle's own install
    // must never have shipped (bundles install only the user mod + shared
    // lib). `Mods/shared` and the user mods themselves are never touched.
    const UE4SS_SUBTREE_RESIDUE: &[&str] = &[
        "ReadyOrNot/Binaries/Win64/ue4ss/UE4SS_SDK_Backends",
        "ReadyOrNot/Binaries/Win64/ue4ss/LICENSE",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/mods.txt",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/mods.json",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/ActorDumperMod",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/BPML_GenericFunctions",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/BPModLoaderMod",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/CheatManagerEnablerMod",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/ConsoleCommandsMod",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/ConsoleEnablerMod",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/Keybinds",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/LineTraceMod",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/SplitScreenMod",
        "ReadyOrNot/Binaries/Win64/ue4ss/Mods/jsbLuaProfilerMod",
    ];
    let mut candidates: Vec<std::path::PathBuf> = ROOT_FILES
        .iter()
        .chain(UE4SS_SUBTREE_RESIDUE.iter())
        .map(|r| game_path.join(r))
        .collect();
    // Only sweep the live root Mods/ tree when it looks like stable runtime
    // residue (a stock helper or mods.txt present), never a user's mods.
    let live_mods = game_path.join("ReadyOrNot/Binaries/Win64/Mods");
    let is_stable_mods = ROOT_MODS.iter().any(|m| live_mods.join(m).exists());
    if is_stable_mods {
        for m in ROOT_MODS {
            candidates.push(live_mods.join(m));
        }
    }
    if let Ok(root) = app_data_root() {
        // Only the stable key is swept by filename: its whole tree is
        // runtime-owned, while every other key (user mods, experimental
        // runtime) is manifest-owned and must not lose files to a name
        // list. `remove_stable_registrations` (below) deletes the stable
        // key wholesale through its manifest.
        let base = root
            .join("staged")
            .join("mods")
            .join("UE4SS_v3.0.1")
            .join("ReadyOrNot/Binaries/Win64");
        for r in ROOT_FILES.iter().chain(UE4SS_SUBTREE_RESIDUE.iter()) {
            if let Ok(rel) = Path::new(r).strip_prefix("ReadyOrNot/Binaries/Win64") {
                candidates.push(base.join(rel));
            }
        }
        for m in ROOT_MODS {
            candidates.push(base.join("Mods").join(m));
        }
    }
    for candidate in candidates {
        if candidate.is_symlink() || candidate.exists() {
            let is_dir = candidate.is_dir() && !candidate.is_symlink();
            let removed = if is_dir {
                std::fs::remove_dir_all(&candidate).is_ok()
            } else {
                std::fs::remove_file(&candidate).is_ok()
            };
            if removed {
                log::warn!(
                    "ue4ss: removed stable-layout leftover {}",
                    candidate.display()
                );
            }
        }
    }
    remove_stable_registrations();
    migrate_misrooted_user_mods();
}

/// User mods staged at the legacy root `Win64/Mods/<mod>/` belong under the
/// runtime at `Win64/ue4ss/Mods/<mod>/` - the experimental loader only reads
/// the subtree. Moves each non-stock entry across (including `shared/`,
/// merging into the runtime's copy), rewrites the owning manifests to the
/// new paths, and removes the now-empty root `Mods/` dirs. Best-effort:
/// manifests that fail to load or save are skipped, never deleted.
fn migrate_misrooted_user_mods() {
    const STOCK: &[&str] = &[
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
        "mods.txt",
        "mods.json",
    ];
    let Ok(root) = app_data_root() else {
        return;
    };
    let staging_root = root.join("staged");
    let manager = crate::services::manifest::ManifestManager::new(&staging_root);
    let Ok(manifests) = manager.list_all_manifests() else {
        return;
    };
    for manifest in manifests.values() {
        let mut moved: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        for staged in &manifest.installed_files {
            let Ok(rel) = staged.strip_prefix(staging_root.join("mods")) else {
                continue;
            };
            let mut comps = rel.components();
            let key = match comps.next().and_then(|c| c.as_os_str().to_str()) {
                Some(k) => k.to_string(),
                None => continue,
            };
            let rest: std::path::PathBuf = comps.collect();
            let Ok(win_rel) = rest.strip_prefix("ReadyOrNot/Binaries/Win64/Mods") else {
                continue;
            };
            let mod_name = match win_rel
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
            {
                Some(n) => n,
                None => continue,
            };
            if STOCK.iter().any(|s| s.eq_ignore_ascii_case(mod_name)) {
                continue;
            }
            let dest = staging_root
                .join("mods")
                .join(&key)
                .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods")
                .join(win_rel);
            if dest.exists() {
                continue;
            }
            if let Some(parent) = dest.parent() {
                if std::fs::create_dir_all(parent).is_err() {
                    continue;
                }
            }
            // A directory move would be cheaper, but files move one by one:
            // manifests track individual files, and `shared/` must merge
            // into the runtime's copy rather than replace it.
            if std::fs::rename(staged, &dest).is_ok() {
                moved.push((staged.clone(), dest));
            } else if staged.is_file()
                && dest.exists()
                && staged
                    .file_name()
                    .map(|n| n.eq_ignore_ascii_case("UEHelpers.lua"))
                    .unwrap_or(false)
            {
                // Newest-wins for the shared Lua lib: keep whichever copy is
                // newer, drop the other, still rewrite the manifest.
                let staged_mtime = std::fs::metadata(staged).and_then(|m| m.modified()).ok();
                let dest_mtime = std::fs::metadata(&dest).and_then(|m| m.modified()).ok();
                let staged_newer = staged_mtime > dest_mtime;
                if staged_newer && std::fs::copy(staged, &dest).is_ok() {
                    let _ = std::fs::remove_file(staged);
                    moved.push((staged.clone(), dest));
                } else if !staged_newer && std::fs::remove_file(staged).is_ok() {
                    moved.push((staged.clone(), dest));
                }
            }
        }
        if moved.is_empty() {
            continue;
        }
        let Ok(Some(mut fresh)) = manager.load_manifest(&manifest.source_archive) else {
            continue;
        };
        for (old, new) in &moved {
            for f in fresh.installed_files.iter_mut() {
                if f == old {
                    *f = new.clone();
                }
            }
        }
        if manager.save_manifest(&fresh).is_ok() {
            for (old, new) in &moved {
                log::warn!("ue4ss: migrated {} -> {}", old.display(), new.display());
            }
        }
        // Drop the now-empty legacy `Win64/Mods/<mod>/...` parents (up to the
        // key root) so the old location reads as uninstalled.
        // `remove_dir` only succeeds when empty, so populated dirs stop us.
        for (old, _) in &moved {
            let mut dir = old.parent().map(|p| p.to_path_buf());
            while let Some(d) = dir.take() {
                let under_key = d
                    .strip_prefix(staging_root.join("mods"))
                    .map(|r| r.components().count() > 1)
                    .unwrap_or(false);
                if !under_key || std::fs::remove_dir(&d).is_err() {
                    break;
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }
}

/// Drop the stable runtime's registrations so nothing re-links or re-lists
/// it. `UE4SS_v3.0.1.zip`'s manifest owns every stable file (root DLL/ini,
/// docs, root `Mods/` stock helpers), so deleting through the manifest
/// removes them all - including ones a filename list would miss. Then the
/// cached archive, profile entries, and the staged key itself go.
/// Best-effort: every step is skipped on error so a half-present stable
/// install can never block the migration.
fn remove_stable_registrations() {
    const STABLE_ARCHIVE: &str = "UE4SS_v3.0.1.zip";
    const STABLE_KEY: &str = "UE4SS_v3.0.1";
    let Ok(root) = app_data_root() else {
        return;
    };
    let staging_root = root.join("staged");
    let manager = crate::services::manifest::ManifestManager::new(&staging_root);
    if let Ok(Some(manifest)) = manager.load_manifest(STABLE_ARCHIVE) {
        for file_path in &manifest.installed_files {
            if file_path.exists() {
                let _ = std::fs::remove_file(file_path);
            }
        }
        let _ = manager.delete_manifest(STABLE_ARCHIVE);
        log::warn!("ue4ss: removed stable manifest {STABLE_ARCHIVE}");
    }
    let archive = staging_root.join("archives").join(STABLE_ARCHIVE);
    if archive.exists() && std::fs::remove_file(&archive).is_ok() {
        log::warn!("ue4ss: removed stable archive {}", archive.display());
    }
    if let Ok(profiles) = crate::services::profiles::list_profiles() {
        for mut profile in profiles {
            if !profile
                .installed_mod_names
                .iter()
                .any(|n| n == STABLE_ARCHIVE)
            {
                continue;
            }
            profile.installed_mod_names.retain(|n| n != STABLE_ARCHIVE);
            if crate::services::profiles::save_profile(&profile).is_ok() {
                log::warn!(
                    "ue4ss: removed stable runtime from profile '{}'",
                    profile.name
                );
            }
        }
    }
    let dir = staging_root.join("mods").join(STABLE_KEY);
    // The manifest pass only deletes files; the key's now-empty dirs
    // (`Win64/`, `Win64/Mods/<stock>/Scripts/`, ...) are left behind. The
    // whole key is stable-runtime-only, so remove it wholesale - whatever
    // survives the manifest pass (dwmapi.dll, empty dirs) goes too.
    if dir.exists() && std::fs::remove_dir_all(&dir).is_ok() {
        log::warn!("ue4ss: removed stable staged key {}", dir.display());
    }
}

/// A UE4SS script mod's `config.json` - e.g. SRankAlert's settings file,
/// which the mod itself creates on first launch beside `Scripts/`. Missing
/// fields fall back to the mod's defaults, so `{}` is a valid config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssModConfig {
    pub mod_name: String,
    pub exists: bool,
    pub content: Option<String>,
    pub path: Option<std::path::PathBuf>,
}

/// Reader cap matching the mod side (SRankAlert refuses files over 256 KiB)
/// - anything bigger is almost certainly a mistake.
const MOD_CONFIG_MAX_BYTES: u64 = 256 * 1024;

/// Guard against path traversal in mod names coming from the UI: a mod dir
/// name is a single plain component.
fn sanitize_mod_name(mod_name: &str) -> Result<String> {
    let name = mod_name.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\'])
        || name.contains('\0')
    {
        return Err(crate::models::AppError::Validation(format!(
            "invalid UE4SS mod name '{mod_name}'"
        )));
    }
    Ok(name.to_string())
}

/// Staged dir of the UE4SS user mod `mod_name` under any install key (exact
/// match first, then case-insensitive).
fn staged_mod_dir(mod_name: &str) -> Option<std::path::PathBuf> {
    let mods = app_data_root().ok()?.join("staged").join("mods");
    let mut ci_match = None;
    for key in std::fs::read_dir(&mods).ok()?.flatten() {
        let parent = key.path().join("ReadyOrNot/Binaries/Win64/ue4ss/Mods");
        let exact = parent.join(mod_name);
        if exact.is_dir() {
            return Some(exact);
        }
        if ci_match.is_none() {
            if let Ok(entries) = std::fs::read_dir(&parent) {
                for entry in entries.flatten() {
                    if entry.path().is_dir()
                        && entry
                            .file_name()
                            .to_str()
                            .map(|n| n.eq_ignore_ascii_case(mod_name))
                            .unwrap_or(false)
                    {
                        ci_match = Some(entry.path());
                        break;
                    }
                }
            }
        }
    }
    ci_match
}

/// Resolve the on-disk dir of the UE4SS user mod `mod_name`: the staged copy
/// when present (managed install - with link-on-launch-only the live folder
/// is stock at rest, and sync creates live parents as real dirs, so live
/// alone can't be trusted), then the target of a live symlink, then a real
/// (manually installed) live dir.
pub fn mod_dir(game_path: &Path, mod_name: &str) -> Option<std::path::PathBuf> {
    // Managed install: edit the staged copy that sync links live.
    if let Some(staged) = staged_mod_dir(mod_name) {
        return Some(staged);
    }
    let live = game_path
        .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods")
        .join(mod_name);
    if live.is_symlink() {
        if let Ok(target) = std::fs::read_link(&live) {
            let resolved = if target.is_absolute() {
                target
            } else {
                live.parent().unwrap_or(Path::new("")).join(target)
            };
            if resolved.is_dir() {
                return Some(resolved);
            }
        }
        return None;
    }
    if live.is_dir() {
        // Real dir, not a symlink: manual install - edit it in place.
        return Some(live);
    }
    None
}

/// Read a UE4SS mod's `config.json`. `exists` is false before the first game
/// launch creates the file (or before the user creates one through the
/// editor) - the mod then runs on its built-in defaults.
pub fn read_mod_config(game_path: &Path, mod_name: &str) -> Result<Ue4ssModConfig> {
    let name = sanitize_mod_name(mod_name)?;
    let dir = mod_dir(game_path, &name);
    let path = dir.as_ref().map(|d| d.join("config.json"));
    let content = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
    let exists = path.as_ref().map(|p| p.exists()).unwrap_or(false);
    Ok(Ue4ssModConfig {
        mod_name: name,
        exists,
        content,
        path,
    })
}

/// Write (or create) a UE4SS mod's `config.json`. `content` must be a JSON
/// object within 256 KiB - the mod ignores unknown fields and falls back to
/// defaults per field, but broken JSON disables the mod's safe features for
/// that session, so it is rejected here.
///
/// Writes the staged copy, links it live when the mod is currently enabled,
/// and backfills the install manifest so sync links it on future syncs and
/// uninstall removes it. Returns the staged path written.
pub fn write_mod_config(
    game_path: &Path,
    mod_name: &str,
    content: &str,
) -> Result<std::path::PathBuf> {
    if content.len() as u64 > MOD_CONFIG_MAX_BYTES {
        return Err(crate::models::AppError::Validation(
            "config.json exceeds 256 KiB".to_string(),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| crate::models::AppError::Validation(format!("invalid JSON: {e}")))?;
    if !value.is_object() {
        return Err(crate::models::AppError::Validation(
            "config.json must contain one JSON object".to_string(),
        ));
    }
    let name = sanitize_mod_name(mod_name)?;
    let Some(dir) = mod_dir(game_path, &name) else {
        return Err(crate::models::AppError::Validation(format!(
            "UE4SS mod '{name}' is not installed"
        )));
    };
    // A manual install lives live as real files; managed installs edit the
    // staged copy that sync links live.
    let staged_mods_root = app_data_root().map(|r| r.join("staged").join("mods")).ok();
    let is_staged = staged_mods_root
        .as_ref()
        .map(|root| dir.starts_with(root))
        .unwrap_or(false);
    let staged_path = dir.join("config.json");
    if let Some(parent) = staged_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&staged_path, content)?;

    if !is_staged {
        return Ok(staged_path);
    }

    backfill_manifest_for_staged(&staged_path);
    link_staged_config_live(game_path, &name, "config.json", &staged_path);
    Ok(staged_path)
}

/// Track a newly created staged config file in its install manifest so sync
/// links it on future syncs and uninstall removes it.
fn backfill_manifest_for_staged(staged_path: &Path) {
    let Ok(root) = app_data_root().map(|r| r.join("staged")) else {
        return;
    };
    let mods_root = root.join("mods");
    let Ok(rel) = staged_path.strip_prefix(&mods_root) else {
        return;
    };
    let Some(key) = rel.components().next().and_then(|c| c.as_os_str().to_str()) else {
        return;
    };
    let manager = crate::services::manifest::ManifestManager::new(&root);
    let Ok(manifests) = manager.list_all_manifests() else {
        return;
    };
    for manifest in manifests.values() {
        if crate::commands::mods::archive_install_key(&manifest.source_archive) == key
            && !manifest
                .installed_files
                .iter()
                .any(|f| f.as_path() == staged_path)
        {
            if let Ok(Some(mut fresh)) = manager.load_manifest(&manifest.source_archive) {
                fresh.installed_files.push(staged_path.to_path_buf());
                let _ = manager.save_manifest(&fresh);
            }
            break;
        }
    }
}

/// Link a staged mod config file live now when the mod is enabled (its live
/// dir exists) so the edit applies on next launch without waiting for a
/// sync. A live symlink already points at the staged file - nothing to do
/// there. `rel` is the mod-dir-relative path (`config.json` or
/// `Scripts/config.lua`).
fn link_staged_config_live(game_path: &Path, mod_name: &str, rel: &str, staged_path: &Path) {
    let live_path = game_path
        .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods")
        .join(mod_name)
        .join(rel);
    if live_path.is_symlink() {
        return;
    }
    if let Some(live_parent) = live_path.parent() {
        if live_parent.is_dir() {
            if live_path.exists() {
                let _ = std::fs::remove_file(&live_path);
            }
            link_staged_live(staged_path, &live_path);
        }
    }
}

/// Relative paths of the Lua config files inside a UE4SS mod dir: the user
/// config and the shipped defaults some mods (SquadEight) seed it from.
pub const LUA_CONFIG_REL: &str = "Scripts/config.lua";
pub const LUA_DEFAULT_CONFIG_REL: &str = "Scripts/config.default.lua";

/// A UE4SS script mod's editable Lua config file (`Scripts/config.lua`,
/// seeded from `Scripts/config.default.lua` when the user file doesn't
/// exist yet). Edited as raw text, like `config.json` - the backend only
/// rejects text with Lua syntax errors.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ue4ssLuaConfig {
    pub mod_name: String,
    /// A `config.lua` exists (shipped or user-created).
    pub exists: bool,
    /// No `config.lua`, but a `config.default.lua` to seed one from.
    pub from_default: bool,
    /// Raw file text of the user config, or of the shipped defaults when
    /// `from_default`.
    pub content: Option<String>,
    pub path: Option<std::path::PathBuf>,
    /// A pre-edit backup exists and Revert is available.
    pub can_revert: bool,
}

/// Backup path for a mod's Lua config: under the key's backup dir for
/// managed installs (never inside the mod dir, so no loader ever sees it),
/// beside the file for manual installs.
fn lua_backup_path(dir: &Path, mod_name: &str) -> Option<std::path::PathBuf> {
    let safe: String = mod_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if let Ok(root) = app_data_root().map(|r| r.join("staged")) {
        if let Ok(rel) = dir.strip_prefix(root.join("mods")) {
            if let Some(key) = rel.components().next().and_then(|c| c.as_os_str().to_str()) {
                return Some(
                    root.join("backups")
                        .join(key)
                        .join("ue4ss_lua")
                        .join(format!("{safe}_config.lua.bak")),
                );
            }
        }
    }
    Some(dir.join("Scripts/config.lua.ronmm-bak"))
}

/// Reject text with Lua syntax errors, reporting the first few with line
/// numbers. Parsed at the Lua 5.4 level (the UE4SS runtime).
fn validate_lua_syntax(text: &str) -> Result<()> {
    let tree = LuaParser::parse(text, ParserConfig::with_level(LuaLanguageLevel::Lua54));
    if !tree.has_syntax_errors() {
        return Ok(());
    }
    let mut details: Vec<String> = Vec::new();
    for err in tree.get_errors().iter().take(3) {
        let line = text[..usize::from(err.range.start()).min(text.len())]
            .chars()
            .filter(|c| *c == '\n')
            .count()
            + 1;
        details.push(format!("line {line}: {}", err.message));
    }
    Err(crate::models::AppError::Validation(format!(
        "invalid Lua: {}",
        details.join("; ")
    )))
}

/// Read a UE4SS mod's Lua config file (`Scripts/config.lua`, seeded from
/// `Scripts/config.default.lua` when the user file doesn't exist yet).
/// Returns the raw text - unlike the old tailored form, a file with syntax
/// errors still opens for editing; only saving validates.
pub fn read_lua_config(game_path: &Path, mod_name: &str) -> Result<Ue4ssLuaConfig> {
    let name = sanitize_mod_name(mod_name)?;
    let dir = mod_dir(game_path, &name);
    let config_path = dir.as_ref().map(|d| d.join(LUA_CONFIG_REL));
    let default_path = dir.as_ref().map(|d| d.join(LUA_DEFAULT_CONFIG_REL));
    let source = config_path
        .as_ref()
        .filter(|p| p.exists())
        .or_else(|| default_path.as_ref().filter(|p| p.exists()));
    let Some(source) = source else {
        return Ok(Ue4ssLuaConfig {
            mod_name: name,
            exists: false,
            from_default: false,
            content: None,
            path: None,
            can_revert: false,
        });
    };
    let text = std::fs::read_to_string(source)?;
    if text.len() as u64 > MOD_CONFIG_MAX_BYTES {
        return Err(crate::models::AppError::Validation(
            "Lua config exceeds 256 KiB".to_string(),
        ));
    }
    let from_default = Some(source) == default_path.as_ref();
    let can_revert = dir
        .as_ref()
        .and_then(|d| lua_backup_path(d, &name))
        .map(|b| b.exists())
        .unwrap_or(false);
    Ok(Ue4ssLuaConfig {
        mod_name: name,
        exists: !from_default,
        from_default,
        content: Some(text),
        path: config_path,
        can_revert,
    })
}

/// Write (or create) a UE4SS mod's Lua config file as raw text. Anything
/// with Lua syntax errors is rejected before it can reach the game.
///
/// The pre-edit content is backed up once (revertible), the manifest is
/// backfilled, and the file is linked live when the mod is enabled.
/// Returns the staged path written.
pub fn write_lua_config(
    game_path: &Path,
    mod_name: &str,
    content: &str,
) -> Result<std::path::PathBuf> {
    if content.len() as u64 > MOD_CONFIG_MAX_BYTES {
        return Err(crate::models::AppError::Validation(
            "Lua config exceeds 256 KiB".to_string(),
        ));
    }
    validate_lua_syntax(content)?;
    let name = sanitize_mod_name(mod_name)?;
    let Some(dir) = mod_dir(game_path, &name) else {
        return Err(crate::models::AppError::Validation(format!(
            "UE4SS mod '{name}' is not installed"
        )));
    };
    let config_path = dir.join(LUA_CONFIG_REL);
    if !config_path.exists() && !dir.join(LUA_DEFAULT_CONFIG_REL).exists() {
        return Err(crate::models::AppError::Validation(format!(
            "UE4SS mod '{name}' has no editable Lua config"
        )));
    }
    if config_path.exists() {
        if let Ok(current) = std::fs::read_to_string(&config_path) {
            if current == content {
                return Ok(config_path);
            }
        }
        // Back up the pre-edit user file once, so Revert restores shipped state.
        if let Some(backup) = lua_backup_path(&dir, &name) {
            if !backup.exists() {
                if let Some(parent) = backup.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&config_path, &backup)?;
            }
        }
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, content)?;

    let staged_mods_root = app_data_root().map(|r| r.join("staged").join("mods")).ok();
    let is_staged = staged_mods_root
        .as_ref()
        .map(|root| dir.starts_with(root))
        .unwrap_or(false);
    if is_staged {
        backfill_manifest_for_staged(&config_path);
        link_staged_config_live(game_path, &name, LUA_CONFIG_REL, &config_path);
    }
    Ok(config_path)
}

/// Restore a UE4SS mod's Lua config from its pre-edit backup.
pub fn revert_lua_config(game_path: &Path, mod_name: &str) -> Result<std::path::PathBuf> {
    let name = sanitize_mod_name(mod_name)?;
    let Some(dir) = mod_dir(game_path, &name) else {
        return Err(crate::models::AppError::Validation(format!(
            "UE4SS mod '{name}' is not installed"
        )));
    };
    let backup = lua_backup_path(&dir, &name)
        .filter(|b| b.exists())
        .ok_or_else(|| {
            crate::models::AppError::Validation(format!(
                "UE4SS mod '{name}' has no config backup to revert"
            ))
        })?;
    let content = std::fs::read(&backup)?;
    let text = String::from_utf8(content).map_err(|_| {
        crate::models::AppError::Validation("config backup is not valid UTF-8".to_string())
    })?;
    validate_lua_syntax(&text)?;
    let config_path = dir.join(LUA_CONFIG_REL);
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, text)?;
    let staged_mods_root = app_data_root().map(|r| r.join("staged").join("mods")).ok();
    if staged_mods_root
        .as_ref()
        .map(|root| dir.starts_with(root))
        .unwrap_or(false)
    {
        link_staged_config_live(game_path, &name, LUA_CONFIG_REL, &config_path);
    }
    Ok(config_path)
}

/// One staged UE4SS user-config file snapshotted before an (update)
/// install: `Scripts/config.lua` or the mod-root `config.json`.
pub struct StagedLuaSnapshot {
    pub mod_name: String,
    pub staged_path: std::path::PathBuf,
    pub content: Vec<u8>,
}

/// Snapshot every staged `rel` config file under a staging key dir before
/// extraction, so user edits can survive the update below.
fn snapshot_staged_mod_configs(staging_key_dir: &Path, rel: &str) -> Vec<StagedLuaSnapshot> {
    let mut snapshots = Vec::new();
    let mods_root = staging_key_dir.join("ReadyOrNot/Binaries/Win64/ue4ss/Mods");
    let Ok(entries) = std::fs::read_dir(&mods_root) else {
        return snapshots;
    };
    for entry in entries.flatten() {
        let mod_dir = entry.path();
        if !mod_dir.is_dir() {
            continue;
        }
        let config_path = mod_dir.join(rel);
        if !config_path.is_file() {
            continue;
        }
        if let (Some(name), Ok(content)) = (
            entry.file_name().to_str().map(str::to_string),
            std::fs::read(&config_path),
        ) {
            snapshots.push(StagedLuaSnapshot {
                mod_name: name,
                staged_path: config_path,
                content,
            });
        }
    }
    snapshots
}

/// Snapshot every `Scripts/config.lua` under a staging key dir before
/// extraction, so user edits can survive the update below.
pub fn snapshot_staged_lua_configs(staging_key_dir: &Path) -> Vec<StagedLuaSnapshot> {
    snapshot_staged_mod_configs(staging_key_dir, LUA_CONFIG_REL)
}

/// Snapshot every mod-root `config.json` under a staging key dir before
/// extraction, so user edits can survive the update below. Seeded configs
/// (cloned from `config.example.json` at install) are covered too: once
/// the user edits one it is indistinguishable from any shipped file, which
/// is exactly what the sidecar hashes are for.
pub fn snapshot_staged_json_configs(staging_key_dir: &Path) -> Vec<StagedLuaSnapshot> {
    snapshot_staged_mod_configs(staging_key_dir, "config.json")
}

/// Restore user-edited Lua configs an update install overwrote.
///
/// Ship hashes live in `<backup_root>/ue4ss_ship/<mod>.sha256` (recorded on
/// every install). A staged file whose pre-install content differed from the
/// recorded ship hash was user-edited: if the install replaced it, the
/// snapshot goes back and the sidecar tracks the new ship hash. Files without
/// a sidecar are recorded as-is (first sighting assumes unmodified).
/// Returns the number of restored files.
pub fn preserve_user_lua_configs(
    backup_root_for_key: &Path,
    snapshots: &[StagedLuaSnapshot],
) -> usize {
    preserve_user_mod_configs(backup_root_for_key, snapshots, "ue4ss_ship")
}

/// Restore user-edited `config.json` files an update install overwrote.
/// Same sidecar protocol as the Lua variant, under `ue4ss_ship_json` so the
/// two config kinds never share a sidecar. Returns the number of restored
/// files.
pub fn preserve_user_json_configs(
    backup_root_for_key: &Path,
    snapshots: &[StagedLuaSnapshot],
) -> usize {
    preserve_user_mod_configs(backup_root_for_key, snapshots, "ue4ss_ship_json")
}

fn preserve_user_mod_configs(
    backup_root_for_key: &Path,
    snapshots: &[StagedLuaSnapshot],
    ship_subdir: &str,
) -> usize {
    use crate::services::hasher::crc32_bytes;
    let ship_dir = backup_root_for_key.join(ship_subdir);
    let mut restored = 0;
    for snapshot in snapshots {
        let Ok(current) = std::fs::read(&snapshot.staged_path) else {
            continue;
        };
        let safe: String = snapshot
            .mod_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let sidecar = ship_dir.join(format!("{safe}.sha256"));
        let current_hash = format!("{:08x}", crc32_bytes(&current));
        let snapshot_hash = format!("{:08x}", crc32_bytes(&snapshot.content));
        let recorded = std::fs::read_to_string(&sidecar)
            .ok()
            .map(|s| s.trim().to_string());
        match recorded {
            None => {
                // First sighting: assume unmodified, track the ship hash.
                if let Some(parent) = sidecar.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&sidecar, &current_hash);
            }
            Some(recorded) if snapshot_hash != recorded && current != snapshot.content => {
                // User-edited file the install just overwrote: put it back.
                if std::fs::write(&snapshot.staged_path, &snapshot.content).is_ok() {
                    restored += 1;
                    log::warn!(
                        "ue4ss: preserved user-edited {} (update shipped a new copy)",
                        snapshot.staged_path.display()
                    );
                }
                let _ = std::fs::write(&sidecar, &current_hash);
            }
            _ => {
                let _ = std::fs::write(&sidecar, &current_hash);
            }
        }
    }
    restored
}

/// Symlink a staged file live, falling back to copy where symlinks are not
/// permitted (Windows without Developer Mode). Best-effort: sync re-links on
/// the next run anyway.
fn link_staged_live(staged: &Path, live: &Path) {
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(staged, live);
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_file(staged, live).is_err()
            && std::fs::hard_link(staged, live).is_err()
        {
            let _ = std::fs::copy(staged, live);
        }
    }
}

/// Read the current UE4SS settings from the staged ini.
pub fn read_settings(game_path: &Path) -> Ue4ssSettings {
    let Some(path) = staged_settings_path(game_path) else {
        return Ue4ssSettings::stock_defaults();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ue4ssSettings::stock_defaults();
    };
    let cache = read_ini_value(&text, "General", CRASH_FIX_KEY)
        .and_then(|v| parse_ini_bool(&v))
        .unwrap_or(true);
    let console = read_ini_value(&text, "Debug", "ConsoleEnabled").and_then(|v| parse_ini_bool(&v));
    let gui = read_ini_value(&text, "Debug", "GuiConsoleEnabled").and_then(|v| parse_ini_bool(&v));
    let mode = match (console, gui) {
        (Some(true), _) => Ue4ssConsoleMode::Text,
        (Some(false), Some(false)) => Ue4ssConsoleMode::None,
        _ => Ue4ssConsoleMode::Gui,
    };
    Ue4ssSettings {
        use_object_array_cache: cache,
        engine_major_version: read_ini_value(&text, "EngineVersionOverride", "MajorVersion")
            .unwrap_or_default(),
        engine_minor_version: read_ini_value(&text, "EngineVersionOverride", "MinorVersion")
            .unwrap_or_default(),
        graphics_api: read_ini_value(&text, "Debug", "GraphicsAPI")
            .unwrap_or_else(|| "opengl".to_string()),
        hook_begin_play: read_ini_value(&text, "Hooks", "HookBeginPlay")
            .and_then(|v| parse_ini_bool(&v))
            .unwrap_or(true),
        console_mode: mode,
        settings_present: true,
    }
}

/// Apply settings to the staged ini. Called after every runtime install and
/// from the settings UI. The `Ue4ssSettings` value carries the full desired
/// state (guide defaults for fresh installs); only differing keys are
/// rewritten. Returns `true` when the file was rewritten.
pub fn apply_settings(game_path: &Path, settings: &Ue4ssSettings) -> Result<bool> {
    let Some(path) = staged_settings_path(game_path) else {
        return Err(crate::models::AppError::Validation(
            "UE4SS is not installed - no UE4SS-settings.ini found".to_string(),
        ));
    };
    let text = std::fs::read_to_string(&path)?;
    let (console, gui, visible) = settings.console_mode.triple();
    let bool_str = |b: bool| if b { "1" } else { "0" };
    let mut patched = set_ini_value(
        &text,
        "General",
        CRASH_FIX_KEY,
        bool_str(settings.use_object_array_cache),
    );
    patched = set_ini_value(
        &patched,
        "EngineVersionOverride",
        "MajorVersion",
        &settings.engine_major_version,
    );
    patched = set_ini_value(
        &patched,
        "EngineVersionOverride",
        "MinorVersion",
        &settings.engine_minor_version,
    );
    patched = set_ini_value(&patched, "Debug", "GraphicsAPI", &settings.graphics_api);
    patched = set_ini_value(
        &patched,
        "Hooks",
        "HookBeginPlay",
        bool_str(settings.hook_begin_play),
    );
    patched = set_ini_value(&patched, "Debug", "ConsoleEnabled", bool_str(console));
    patched = set_ini_value(&patched, "Debug", "GuiConsoleEnabled", bool_str(gui));
    patched = set_ini_value(&patched, "Debug", "GuiConsoleVisible", bool_str(visible));
    if patched == text {
        return Ok(false);
    }
    std::fs::write(&path, patched)?;
    log::info!(
        "ue4ss settings: {}={} engine={}.{} gfx={} hook_begin_play={} console={:?} in {}",
        CRASH_FIX_KEY,
        bool_str(settings.use_object_array_cache),
        settings.engine_major_version,
        settings.engine_minor_version,
        settings.graphics_api,
        bool_str(settings.hook_begin_play),
        settings.console_mode,
        path.display()
    );
    Ok(true)
}

/// Remove stale UE4SS 2.x loader shims (`xinput1_3.dll`) from both the live
/// Win64 folder and every staged copy. They crash the game alongside the
/// newer loader. Returns the number of files removed.
pub fn remove_stale_shims(game_path: &Path) -> usize {
    let mut removed = 0;
    let live = game_path.join(format!("ReadyOrNot/Binaries/Win64/{STALE_SHIM}"));
    for candidate in std::iter::once(live).chain(
        app_data_root()
            .ok()
            .and_then(|root| {
                std::fs::read_dir(root.join("staged").join("mods"))
                    .ok()
                    .map(|entries| {
                        entries.flatten().map(|e| {
                            e.path()
                                .join(format!("ReadyOrNot/Binaries/Win64/{STALE_SHIM}"))
                        })
                    })
            })
            .into_iter()
            .flatten(),
    ) {
        if (candidate.is_symlink() || candidate.exists())
            && std::fs::remove_file(&candidate).is_ok()
        {
            log::warn!(
                "ue4ss: removed stale UE4SS 2.x shim {}",
                candidate.display()
            );
            removed += 1;
        }
    }
    removed
}

/// Stock helper mod names shipped in the official release zip. Shared with
/// `remove_stable_leftovers` and the live residue sweep below.
const STOCK_MODS: &[&str] = &[
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

/// UE4SS runtime byproducts that are safe to delete from the live game
/// folder on stock cleanup. `UE4SS.log` is deliberately absent - it is the
/// script-debugging trail and must be kept. `dwmapi.dll` and any still-linked
/// tracked file are never touched.
const LIVE_RESIDUE_FILES: &[&str] = &[
    "ReadyOrNot/Binaries/Win64/imgui.ini",
    "ReadyOrNot/Binaries/Win64/ue4ss/imgui.ini",
    "ReadyOrNot/Binaries/Win64/ue4ss/LICENSE",
    "ReadyOrNot/Binaries/Win64/ue4ss/Mods/mods.txt",
    "ReadyOrNot/Binaries/Win64/ue4ss/Mods/mods.json",
];

/// Sweep live `Win64/ue4ss/` residue after stock restore. Deletes allowlisted
/// runtime byproducts (imgui.ini, SDK backends dir, stock helpers), prunes
/// empty parent dirs up to the game root, and removes `ue4ss/` itself when
/// nothing worth keeping (i.e. no `UE4SS.log`) remains. Best-effort: never
/// fails the caller. Returns the number of entries removed.
pub fn sweep_live_residue(game_path: &Path) -> usize {
    let mut removed = 0;
    let win64 = game_path.join("ReadyOrNot/Binaries/Win64");
    let ue4ss_dir = win64.join("ue4ss");
    for rel in LIVE_RESIDUE_FILES {
        let candidate = game_path.join(rel);
        if (candidate.is_symlink() || candidate.exists())
            && std::fs::remove_file(&candidate).is_ok()
        {
            log::warn!("ue4ss: removed live residue {}", candidate.display());
            removed += 1;
        }
    }
    for name in STOCK_MODS {
        let dir = ue4ss_dir.join("Mods").join(name);
        if dir.is_dir() && !dir.is_symlink() && std::fs::remove_dir_all(&dir).is_ok() {
            log::warn!("ue4ss: removed live stock helper {}", dir.display());
            removed += 1;
        }
    }
    let sdk = ue4ss_dir.join("UE4SS_SDK_Backends");
    if sdk.is_dir() && !sdk.is_symlink() && std::fs::remove_dir_all(&sdk).is_ok() {
        log::warn!("ue4ss: removed live SDK backends {}", sdk.display());
        removed += 1;
    }
    // Prune empty parents up to (not including) the Win64 root.
    let mut pruned = prune_empty_up(ue4ss_dir.join("Mods"), &win64);
    pruned += prune_empty_up(ue4ss_dir.clone(), &win64);
    removed += pruned;
    // Drop ue4ss/ itself when nothing worth keeping remains. UE4SS.log
    // keeps the folder alive for script debugging.
    if !ue4ss_dir.join("UE4SS.log").exists()
        && ue4ss_dir.is_dir()
        && std::fs::remove_dir(&ue4ss_dir).is_ok()
    {
        log::warn!(
            "ue4ss: removed empty live ue4ss dir {}",
            ue4ss_dir.display()
        );
        removed += 1;
    }
    removed
}

/// Remove `start` and its empty parents up to (not including) `stop`.
/// `remove_dir` only succeeds when empty, so populated dirs stop the walk.
fn prune_empty_up(mut current: std::path::PathBuf, stop: &Path) -> usize {
    let mut removed = 0;
    // Also walk the subtree below start first: empty leaf dirs (e.g.
    // Mods/<mod>/Scripts) must go before their parents read as empty.
    if let Ok(entries) = std::fs::read_dir(&current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !path.is_symlink() {
                removed += prune_empty_up(path, stop);
            }
        }
    }
    while current.starts_with(stop) && current != *stop {
        match std::fs::remove_dir(&current) {
            Ok(()) => {
                removed += 1;
                match current.parent() {
                    Some(p) => current = p.to_path_buf(),
                    None => break,
                }
            }
            Err(_) => break,
        }
    }
    removed
}

/// Resolve the current experimental runtime asset via the GitHub releases
/// API. Returns `(archive_name, download_url)`; the name embeds the upstream
/// commit hash and is kept verbatim so the staged copy identifies its build.
async fn resolve_experimental_asset(client: &reqwest::Client) -> Result<(String, String)> {
    #[derive(serde::Deserialize)]
    struct ReleaseAsset {
        name: String,
        browser_download_url: String,
    }
    #[derive(serde::Deserialize)]
    struct Release {
        assets: Vec<ReleaseAsset>,
    }
    let release: Release = client
        .get(UE4SS_RELEASE_API)
        .header("User-Agent", "RoNModManager")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| crate::models::AppError::Io(std::io::Error::other(e.to_string())))?
        .json::<Release>()
        .await
        .map_err(|e| {
            crate::models::AppError::Validation(format!(
                "failed to read UE4SS {UE4SS_RELEASE_TAG} release info: {e}"
            ))
        })?;
    release
        .assets
        .into_iter()
        .find(|a| {
            a.name.starts_with(UE4SS_ASSET_PREFIX)
                && a.name.ends_with(".zip")
                && !a.name.starts_with("zDEV-")
        })
        .map(|a| (a.name, a.browser_download_url))
        .ok_or_else(|| {
            crate::models::AppError::Validation(format!(
                "no UE4SS runtime asset found in release {UE4SS_RELEASE_TAG}"
            ))
        })
}

/// Download and install the experimental UE4SS release if it isn't already
/// present, migrate away any stable-layout leftovers, and enable it in the
/// active profile so it links in on the next sync.
///
/// The runtime always comes from the `experimental-latest` pre-release on
/// GitHub: the current asset is resolved via the releases API and
/// downloaded. Safe to call unconditionally before installing a
/// UE4SS-dependent mod - a no-op once UE4SS is on disk.
pub async fn ensure_installed(
    app: &AppHandle,
    client: &reqwest::Client,
    game_path: &Path,
    temp_root: &Path,
) -> Result<()> {
    // Migrate stable-layout leftovers first: a Win64-root `UE4SS.dll` from
    // v3.0.1 would load instead of (or alongside) the experimental one under
    // `ue4ss/`. Staged copies, the stable manifest/archive, and profile
    // entries are swept too - nothing may re-link or re-list stable.
    remove_stable_leftovers(game_path);

    if is_installed(game_path) {
        return Ok(());
    }

    let staging_root = app_data_root()?.join("staged");
    let archives_root = staging_root.join("archives");
    std::fs::create_dir_all(&archives_root)?;

    let (archive_name, download_url) = resolve_experimental_asset(client).await?;

    let _ = app.emit(
        "install_progress",
        &ProgressEvent {
            operation: "download".to_string(),
            file: archive_name.clone(),
            percent: 0.0,
            message: "Installing UE4SS experimental (required by this mod)...".to_string(),
            total_bytes: None,
            processed_bytes: None,
        },
    );

    let archive_path = archives_root.join(&archive_name);
    let content_hash = downloader::download_file(client, &download_url, &archive_path).await?;

    let context = installer::InstallContext {
        game_path: game_path.to_path_buf(),
        mods_path: staging_root.join("mods"),
        savegames_path: staging_root.join("savegames"),
        backup_path: staging_root.join("backups"),
    };

    // This is a plain UE4SS runtime archive (no user mod inside), so
    // `detect_ue4ss_layout` reports `bundles_runtime = true` and
    // `install_downloaded_file`'s own UE4SS check is a no-op here - no
    // recursion.
    crate::commands::mods::install_downloaded_file(
        &archive_path,
        &context,
        app,
        client,
        temp_root,
        None,
        None,
        Some(content_hash),
    )
    .await?;

    let active_profile =
        crate::state::load_config_from_path(&crate::state::app_config_root()?.join("config.json"))
            .map(|config| config.active_profile)
            .unwrap_or(None);
    let _ =
        crate::commands::mods::add_mod_to_active_profile(active_profile.as_deref(), &archive_name);

    // Guide's working defaults on every fresh runtime install (stock ini
    // crashes Ready or Not on startup), plus a sweep for stale UE4SS 2.x
    // `xinput1_3.dll` shims that crash alongside the new loader.
    // Best-effort only - a failed patch must not fail the install.
    if let Err(e) = apply_settings(game_path, &Ue4ssSettings::guide_defaults()) {
        log::warn!("ue4ss: failed to apply crash-safe settings: {e}");
    }
    remove_stale_shims(game_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_installed_detects_dll() {
        let dir = TempDir::new().unwrap();
        assert!(!is_installed(dir.path()));

        // Experimental layout: DLL lives under ue4ss/.
        let dll = dir.path().join("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS.dll");
        fs::create_dir_all(dll.parent().unwrap()).unwrap();
        fs::write(&dll, b"fake").unwrap();
        assert!(is_installed(dir.path()));

        // A stale stable-layout DLL at the Win64 root does NOT count.
        let stale_dir = TempDir::new().unwrap();
        let stale = stale_dir.path().join("ReadyOrNot/Binaries/Win64/UE4SS.dll");
        fs::create_dir_all(stale.parent().unwrap()).unwrap();
        fs::write(&stale, b"fake").unwrap();
        assert!(!is_installed(stale_dir.path()));
    }

    #[test]
    fn managed_archive_matches_any_runtime_build() {
        assert!(is_managed_archive("UE4SS_v3.0.1.zip"));
        assert!(is_managed_archive("UE4SS_v3.0.1-1133-gb4cefa18.zip"));
        assert!(!is_managed_archive("RoNVoiceCommanderMod.V1.1.zip"));
        assert!(!is_managed_archive("some-mod.zip"));
    }

    fn write_game_ini(dir: &TempDir, text: &str) -> std::path::PathBuf {
        let ini = dir
            .path()
            .join("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS-settings.ini");
        fs::create_dir_all(ini.parent().unwrap()).unwrap();
        fs::write(&ini, text).unwrap();
        ini
    }

    #[test]
    fn set_ini_value_rewrites_key_preserving_comments_and_crlf() {
        let text = "; header comment\r\n[General]\r\n; crash note\r\nbUseUObjectArrayCache = true\r\n\r\n[Debug]\r\nConsoleEnabled = 0\r\n";
        let patched = set_ini_value(text, "General", "bUseUObjectArrayCache", "false");
        assert!(patched.contains("; header comment\r\n"));
        assert!(patched.contains("; crash note\r\n"));
        assert!(patched.contains("bUseUObjectArrayCache = false\r\n"));
        assert!(!patched.contains("bUseUObjectArrayCache = true"));
        assert!(patched.contains("ConsoleEnabled = 0\r\n"));
    }

    #[test]
    fn set_ini_value_appends_missing_key_and_section() {
        let text = "[General]\nUseCache = 1\n";
        let patched = set_ini_value(text, "General", "bUseUObjectArrayCache", "false");
        assert!(patched.contains("UseCache = 1\n"));
        assert!(patched.ends_with("bUseUObjectArrayCache = false"));

        let no_section = "[General]\nUseCache = 1\n";
        let patched = set_ini_value(no_section, "Debug", "ConsoleEnabled", "1");
        assert!(patched.contains("[Debug]\nConsoleEnabled = 1"));
    }

    #[test]
    fn read_ini_value_finds_last_match_case_insensitive() {
        let text =
            "[General]\nbuseuobjectarraycache = True\n[general]\nBUSEUOBJECTARRAYCACHE = 0\n";
        assert_eq!(
            read_ini_value(text, "general", "BUseUObjectArrayCache"),
            Some("0".to_string())
        );
        assert_eq!(read_ini_value(text, "Debug", "ConsoleEnabled"), None);
    }

    #[test]
    fn read_settings_reports_gui_default_and_text_mode() {
        let dir = TempDir::new().unwrap();
        // No ini at all: stock defaults, not present.
        let settings = read_settings(dir.path());
        assert!(!settings.settings_present);
        assert!(settings.use_object_array_cache);
        assert_eq!(settings.console_mode, Ue4ssConsoleMode::Gui);

        write_game_ini(
            &dir,
            "[General]\nbUseUObjectArrayCache = false\n[Debug]\nConsoleEnabled = 1\nGuiConsoleEnabled = 0\nGuiConsoleVisible = 0\n",
        );
        let settings = read_settings(dir.path());
        assert!(settings.settings_present);
        assert!(!settings.use_object_array_cache);
        assert_eq!(settings.console_mode, Ue4ssConsoleMode::Text);
    }

    fn guide_test_settings(console_mode: Ue4ssConsoleMode) -> Ue4ssSettings {
        Ue4ssSettings {
            use_object_array_cache: false,
            engine_major_version: GUIDE_MAJOR_VERSION.to_string(),
            engine_minor_version: GUIDE_MINOR_VERSION.to_string(),
            graphics_api: GUIDE_GRAPHICS_API.to_string(),
            hook_begin_play: false,
            console_mode,
            settings_present: true,
        }
    }

    #[test]
    fn apply_settings_patches_guide_defaults_and_console_triple() {
        let dir = TempDir::new().unwrap();
        write_game_ini(
            &dir,
            "[General]\nbUseUObjectArrayCache = true\n[Debug]\nConsoleEnabled = 0\nGuiConsoleEnabled = 1\nGuiConsoleVisible = 0\nGraphicsAPI = opengl\n[Hooks]\nHookBeginPlay = 1\n",
        );
        assert!(apply_settings(dir.path(), &guide_test_settings(Ue4ssConsoleMode::Text)).unwrap());
        let text = fs::read_to_string(
            dir.path()
                .join("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS-settings.ini"),
        )
        .unwrap();
        // apply_settings writes canonical 0/1 ini values.
        assert!(text.contains("bUseUObjectArrayCache = 0"));
        assert!(text.contains("MajorVersion = 5"));
        assert!(text.contains("MinorVersion = 3"));
        assert!(text.contains("GraphicsAPI = dx11"));
        assert!(text.contains("HookBeginPlay = 0"));
        assert!(text.contains("ConsoleEnabled = 1"));
        assert!(text.contains("GuiConsoleEnabled = 0"));
        assert!(text.contains("GuiConsoleVisible = 0"));
        let reread = read_settings(dir.path());
        assert!(!reread.use_object_array_cache);
        assert_eq!(reread.engine_major_version, "5");
        assert_eq!(reread.engine_minor_version, "3");
        assert_eq!(reread.graphics_api, "dx11");
        assert!(!reread.hook_begin_play);
        // Second run is a no-op.
        assert!(!apply_settings(dir.path(), &guide_test_settings(Ue4ssConsoleMode::Text)).unwrap());
        // None turns the whole triple off.
        assert!(apply_settings(dir.path(), &guide_test_settings(Ue4ssConsoleMode::None)).unwrap());
        let text = fs::read_to_string(
            dir.path()
                .join("ReadyOrNot/Binaries/Win64/ue4ss/UE4SS-settings.ini"),
        )
        .unwrap();
        assert!(text.contains("ConsoleEnabled = 0"));
        assert!(text.contains("GuiConsoleEnabled = 0"));
        assert!(text.contains("GuiConsoleVisible = 0"));
        assert_eq!(
            read_settings(dir.path()).console_mode,
            Ue4ssConsoleMode::None
        );
    }

    #[test]
    fn guide_defaults_match_modding_guide_working_set() {
        let defaults = Ue4ssSettings::guide_defaults();
        assert!(!defaults.use_object_array_cache);
        assert_eq!(defaults.engine_major_version, "5");
        assert_eq!(defaults.engine_minor_version, "3");
        assert_eq!(defaults.graphics_api, "dx11");
        assert!(!defaults.hook_begin_play);
    }

    #[test]
    fn staged_path_prefers_live_symlink_target() {
        let dir = TempDir::new().unwrap();
        let win64 = dir.path().join("ReadyOrNot/Binaries/Win64/ue4ss");
        fs::create_dir_all(&win64).unwrap();
        // Live real file (manual install): used directly.
        let live = win64.join("UE4SS-settings.ini");
        fs::write(&live, b"[General]\n").unwrap();
        assert_eq!(staged_settings_path(dir.path()), Some(live));
    }

    #[test]
    fn remove_stale_shims_deletes_live_and_staged_copies() {
        let dir = TempDir::new().unwrap();
        let live = dir.path().join("ReadyOrNot/Binaries/Win64/xinput1_3.dll");
        fs::create_dir_all(live.parent().unwrap()).unwrap();
        fs::write(&live, b"stale").unwrap();
        assert_eq!(remove_stale_shims(dir.path()), 1);
        assert!(!live.exists());
        assert_eq!(remove_stale_shims(dir.path()), 0);
    }

    #[test]
    fn sweep_live_residue_keeps_log_clears_rest() {
        let dir = TempDir::new().unwrap();
        let win64 = dir.path().join("ReadyOrNot/Binaries/Win64");
        let ue4ss = win64.join("ue4ss");
        fs::create_dir_all(ue4ss.join("Mods/SomeMod/Scripts")).unwrap();
        fs::create_dir_all(ue4ss.join("Mods/BPModLoaderMod/Scripts")).unwrap();
        fs::create_dir_all(ue4ss.join("UE4SS_SDK_Backends")).unwrap();
        fs::write(ue4ss.join("UE4SS.log"), b"log").unwrap();
        fs::write(ue4ss.join("imgui.ini"), b"x").unwrap();
        fs::write(win64.join("imgui.ini"), b"x").unwrap();
        fs::write(ue4ss.join("Mods/mods.txt"), b"x").unwrap();
        assert!(sweep_live_residue(dir.path()) > 0);
        assert!(ue4ss.join("UE4SS.log").exists());
        assert!(!ue4ss.join("imgui.ini").exists());
        assert!(!win64.join("imgui.ini").exists());
        assert!(!ue4ss.join("UE4SS_SDK_Backends").exists());
        assert!(!ue4ss.join("Mods/BPModLoaderMod").exists());
        assert!(!ue4ss.join("Mods/SomeMod").exists());
        assert!(ue4ss.is_dir());
    }

    #[test]
    fn sweep_live_residue_removes_empty_ue4ss_dir_without_log() {
        let dir = TempDir::new().unwrap();
        let ue4ss = dir.path().join("ReadyOrNot/Binaries/Win64/ue4ss");
        fs::create_dir_all(ue4ss.join("Mods/SomeMod/Scripts")).unwrap();
        fs::write(ue4ss.join("imgui.ini"), b"x").unwrap();
        sweep_live_residue(dir.path());
        assert!(!ue4ss.exists());
    }

    /// Staged mod dir for mod-config tests, unique per test via `key` (the
    /// isolated tree is shared process-wide - callers hold
    /// `shared_tree_guard` for the whole test body).
    fn cfg_test_staging(key: &str, mod_name: &str) -> std::path::PathBuf {
        use crate::test_support::isolated_root;
        // Pin HOME/XDG first so app_data_root() lands in the isolated tree.
        let _ = isolated_root();
        let mod_dir = app_data_root()
            .unwrap()
            .join("staged")
            .join("mods")
            .join(key)
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods")
            .join(mod_name);
        std::fs::create_dir_all(mod_dir.join("Scripts")).unwrap();
        mod_dir
    }

    #[test]
    fn mod_config_missing_before_first_run() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-cfg-missing");
        cfg_test_staging("CfgMissingKey", "CfgMissingMod");

        let cfg = read_mod_config(game.path(), "CfgMissingMod").unwrap();

        assert!(!cfg.exists);
        assert!(cfg.content.is_none());
        assert!(cfg.path.is_some());
    }

    #[test]
    fn mod_config_round_trip_through_staged_copy() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-cfg-roundtrip");
        cfg_test_staging("CfgRoundtripKey", "CfgRoundtripMod");

        let staged = write_mod_config(game.path(), "CfgRoundtripMod", "{\"SCALE\": 1.25}").unwrap();
        assert!(staged.exists());
        assert_eq!(
            std::fs::read_to_string(&staged).unwrap(),
            "{\"SCALE\": 1.25}"
        );

        let cfg = read_mod_config(game.path(), "CfgRoundtripMod").unwrap();
        assert!(cfg.exists);
        assert_eq!(cfg.content.as_deref(), Some("{\"SCALE\": 1.25}"));

        // A partial config is valid - missing fields use mod defaults.
        write_mod_config(game.path(), "CfgRoundtripMod", "{}").unwrap();
        assert_eq!(
            read_mod_config(game.path(), "CfgRoundtripMod")
                .unwrap()
                .content
                .as_deref(),
            Some("{}")
        );
    }

    #[test]
    fn mod_config_write_links_live_when_mod_enabled() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-cfg-livelink");
        cfg_test_staging("CfgLivelinkKey", "CfgLivelinkMod");
        // Simulate an enabled mod: live dir exists, no config yet.
        let live_mod = game
            .path()
            .join("ReadyOrNot/Binaries/Win64/ue4ss/Mods/CfgLivelinkMod");
        std::fs::create_dir_all(live_mod.join("Scripts")).unwrap();

        write_mod_config(game.path(), "CfgLivelinkMod", "{\"SCALE\": 1}").unwrap();

        let live_cfg = live_mod.join("config.json");
        assert!(live_cfg.is_symlink());
        assert_eq!(
            std::fs::read_to_string(&live_cfg).unwrap(),
            "{\"SCALE\": 1}"
        );
    }

    #[test]
    fn mod_config_write_backfills_manifest() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-cfg-manifest");
        let mod_dir = cfg_test_staging("CfgManifestKey", "CfgManifestMod");
        let lua = mod_dir.join("Scripts/main.lua");
        std::fs::write(&lua, b"-- lua").unwrap();

        let staging_root = app_data_root().unwrap().join("staged");
        let manager = crate::services::manifest::ManifestManager::new(&staging_root);
        manager
            .save_manifest(&crate::services::manifest::InstallManifest {
                source_archive: "CfgManifestKey.zip".to_string(),
                display_name: None,
                source_url: None,
                installed_files: vec![lua],
                installed_at: 0,
                content_hash: None,
                nexus_file_id: None,
                installed_version: None,
            })
            .unwrap();

        let staged = write_mod_config(game.path(), "CfgManifestMod", "{\"SCALE\": 1}").unwrap();

        let manifest = manager
            .load_manifest("CfgManifestKey.zip")
            .unwrap()
            .unwrap();
        assert!(manifest.installed_files.contains(&staged));
        let _ = manager.delete_manifest("CfgManifestKey.zip");
    }

    #[test]
    fn mod_config_rejects_bad_content_and_names() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-cfg-reject");
        cfg_test_staging("CfgRejectKey", "CfgRejectMod");

        assert!(write_mod_config(game.path(), "CfgRejectMod", "{broken").is_err());
        assert!(write_mod_config(game.path(), "CfgRejectMod", "[1, 2]").is_err());
        assert!(write_mod_config(game.path(), "CfgRejectMod", "\"str\"").is_err());
        assert!(write_mod_config(game.path(), "CfgRejectMod", &"x".repeat(300 * 1024)).is_err());
        assert!(read_mod_config(game.path(), "../evil").is_err());
        assert!(read_mod_config(game.path(), "a/b").is_err());
        assert!(read_mod_config(game.path(), "").is_err());
        assert!(write_mod_config(game.path(), "NoSuchMod", "{}").is_err());
        // Nothing was written for the rejected inputs.
        assert!(!read_mod_config(game.path(), "CfgRejectMod").unwrap().exists);
    }

    const LUA_TEST_CONFIG: &str = "-- Test config.\nreturn {\n    -- write the report\n    report_key = Key.F7,\n    enabled = true,\n    watch_ms = 2000,\n    folder = \"RoundReports\",\n}\n";

    #[test]
    fn lua_config_reads_raw_text() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-lua-read");
        let mod_dir = cfg_test_staging("CfgLuaReadKey", "CfgLuaReadMod");
        std::fs::write(mod_dir.join("Scripts/config.lua"), LUA_TEST_CONFIG).unwrap();

        let cfg = read_lua_config(game.path(), "CfgLuaReadMod").unwrap();

        assert!(cfg.exists);
        assert!(!cfg.from_default);
        assert!(!cfg.can_revert);
        assert_eq!(cfg.content.as_deref(), Some(LUA_TEST_CONFIG));
        assert!(cfg.path.unwrap().ends_with("Scripts/config.lua"));
    }

    #[test]
    fn lua_config_seeds_from_default() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-lua-seed");
        cfg_test_staging("CfgLuaSeedKey", "CfgLuaSeedMod");
        let mod_dir = app_data_root()
            .unwrap()
            .join("staged/mods/CfgLuaSeedKey/ReadyOrNot/Binaries/Win64/ue4ss/Mods/CfgLuaSeedMod");
        std::fs::write(mod_dir.join("Scripts/config.default.lua"), LUA_TEST_CONFIG).unwrap();

        let cfg = read_lua_config(game.path(), "CfgLuaSeedMod").unwrap();
        assert!(!cfg.exists);
        assert!(cfg.from_default);
        assert_eq!(cfg.content.as_deref(), Some(LUA_TEST_CONFIG));

        // Saving creates the user config from the edited defaults.
        let edited = LUA_TEST_CONFIG.replace("watch_ms = 2000", "watch_ms = 500");
        let staged = write_lua_config(game.path(), "CfgLuaSeedMod", &edited).unwrap();
        assert!(staged.ends_with("Scripts/config.lua"));
        assert_eq!(std::fs::read_to_string(&staged).unwrap(), edited);
        // The shipped defaults are untouched.
        assert!(
            std::fs::read_to_string(mod_dir.join("Scripts/config.default.lua"))
                .unwrap()
                .contains("watch_ms = 2000")
        );
    }

    #[test]
    fn lua_config_write_and_revert() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-lua-revert");
        let mod_dir = cfg_test_staging("CfgLuaRevertKey", "CfgLuaRevertMod");
        std::fs::write(mod_dir.join("Scripts/config.lua"), LUA_TEST_CONFIG).unwrap();

        let edited = LUA_TEST_CONFIG.replace("enabled = true", "enabled = false");
        write_lua_config(game.path(), "CfgLuaRevertMod", &edited).unwrap();
        let staged = mod_dir.join("Scripts/config.lua");
        assert!(std::fs::read_to_string(&staged)
            .unwrap()
            .contains("enabled = false"));
        assert!(
            read_lua_config(game.path(), "CfgLuaRevertMod")
                .unwrap()
                .can_revert
        );

        // Broken Lua never reaches the file.
        assert!(write_lua_config(game.path(), "CfgLuaRevertMod", "return { broken = ").is_err());
        assert!(std::fs::read_to_string(&staged)
            .unwrap()
            .contains("watch_ms = 2000"));

        // Saving identical content is a no-op success.
        write_lua_config(game.path(), "CfgLuaRevertMod", &edited).unwrap();

        revert_lua_config(game.path(), "CfgLuaRevertMod").unwrap();
        let restored = std::fs::read_to_string(&staged).unwrap();
        assert!(restored.contains("enabled = true"));
        // Reverting with no backup errors.
        assert!(revert_lua_config(game.path(), "CfgLuaReadMod").is_err());
    }

    #[test]
    fn lua_config_rejects_missing_mod_and_files() {
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let game = scratch_dir("ue4ss-lua-missing");
        cfg_test_staging("CfgLuaMissingKey", "CfgLuaMissingMod");

        let cfg = read_lua_config(game.path(), "CfgLuaMissingMod").unwrap();
        assert!(!cfg.exists && !cfg.from_default && cfg.content.is_none());
        assert!(write_lua_config(game.path(), "CfgLuaMissingMod", LUA_TEST_CONFIG).is_err());
        assert!(write_lua_config(game.path(), "NoSuchMod", LUA_TEST_CONFIG).is_err());
        assert!(read_lua_config(game.path(), "NoSuchMod")
            .unwrap()
            .path
            .is_none());
    }

    #[test]
    fn preserve_restores_user_edited_lua_config() {
        use crate::services::hasher::crc32_bytes;
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let _game = scratch_dir("ue4ss-lua-preserve");
        let mod_dir = cfg_test_staging("CfgLuaPreserveKey", "CfgLuaPreserveMod");
        let key_dir = mod_dir
            .ancestors()
            .find(|a| a.ends_with("CfgLuaPreserveKey"))
            .unwrap()
            .to_path_buf();
        let backup_root = app_data_root()
            .unwrap()
            .join("staged/backups/CfgLuaPreserveKey");

        // Staged file holds user edits; sidecar records the older ship hash.
        let user_content = LUA_TEST_CONFIG.replace("watch_ms = 2000", "watch_ms = 500");
        std::fs::write(mod_dir.join("Scripts/config.lua"), &user_content).unwrap();
        let ship_content = LUA_TEST_CONFIG;
        std::fs::create_dir_all(backup_root.join("ue4ss_ship")).unwrap();
        std::fs::write(
            backup_root.join("ue4ss_ship/CfgLuaPreserveMod.sha256"),
            format!("{:08x}", crc32_bytes(ship_content.as_bytes())),
        )
        .unwrap();

        let snapshots = snapshot_staged_lua_configs(&key_dir);
        assert_eq!(snapshots.len(), 1);

        // The update overwrites with its own shipped copy...
        std::fs::write(
            mod_dir.join("Scripts/config.lua"),
            "return { watch_ms = 250 }",
        )
        .unwrap();
        // ...and preserve puts the user content back.
        assert_eq!(preserve_user_lua_configs(&backup_root, &snapshots), 1);
        assert_eq!(
            std::fs::read_to_string(mod_dir.join("Scripts/config.lua")).unwrap(),
            user_content
        );

        // Unmodified files are left alone and re-tracked: stage the ship
        // content the sidecar already records, then update over it.
        std::fs::write(
            mod_dir.join("Scripts/config.lua"),
            "return { watch_ms = 300 }",
        )
        .unwrap();
        std::fs::write(
            backup_root.join("ue4ss_ship/CfgLuaPreserveMod.sha256"),
            format!(
                "{:08x}",
                crc32_bytes("return { watch_ms = 300 }".as_bytes())
            ),
        )
        .unwrap();
        let snapshots = snapshot_staged_lua_configs(&key_dir);
        std::fs::write(
            mod_dir.join("Scripts/config.lua"),
            "return { watch_ms = 350 }",
        )
        .unwrap();
        assert_eq!(preserve_user_lua_configs(&backup_root, &snapshots), 0);
        assert!(std::fs::read_to_string(mod_dir.join("Scripts/config.lua"))
            .unwrap()
            .contains("watch_ms = 350"));
    }

    #[test]
    fn preserve_restores_user_edited_json_config() {
        use crate::services::hasher::crc32_bytes;
        use crate::test_support::{scratch_dir, shared_tree_guard};
        let _guard = shared_tree_guard();
        let _game = scratch_dir("ue4ss-json-preserve");
        let mod_dir = cfg_test_staging("CfgJsonPreserveKey", "CfgJsonPreserveMod");
        let key_dir = mod_dir
            .ancestors()
            .find(|a| a.ends_with("CfgJsonPreserveKey"))
            .unwrap()
            .to_path_buf();
        let backup_root = app_data_root()
            .unwrap()
            .join("staged/backups/CfgJsonPreserveKey");

        // Staged file holds user edits over the seeded example content;
        // the sidecar records the older ship hash.
        let user_content = "{\"seeded\":true,\"watch_ms\":500}";
        std::fs::write(mod_dir.join("config.json"), user_content).unwrap();
        std::fs::create_dir_all(backup_root.join("ue4ss_ship_json")).unwrap();
        std::fs::write(
            backup_root.join("ue4ss_ship_json/CfgJsonPreserveMod.sha256"),
            format!("{:08x}", crc32_bytes(b"{\"seeded\":true}")),
        )
        .unwrap();

        let snapshots = snapshot_staged_json_configs(&key_dir);
        assert_eq!(snapshots.len(), 1);

        // The update re-seeds from the example...
        std::fs::write(mod_dir.join("config.json"), "{\"seeded\":true}").unwrap();
        // ...and preserve puts the user content back.
        assert_eq!(preserve_user_json_configs(&backup_root, &snapshots), 1);
        assert_eq!(
            std::fs::read_to_string(mod_dir.join("config.json")).unwrap(),
            user_content
        );

        // Unmodified files are left alone: stage the ship content the
        // sidecar already records, then update over it.
        std::fs::write(mod_dir.join("config.json"), "{\"seeded\":true}").unwrap();
        let snapshots = snapshot_staged_json_configs(&key_dir);
        std::fs::write(mod_dir.join("config.json"), "{\"seeded\":true,\"v\":2}").unwrap();
        assert_eq!(preserve_user_json_configs(&backup_root, &snapshots), 0);
        assert!(std::fs::read_to_string(mod_dir.join("config.json"))
            .unwrap()
            .contains("\"v\":2"));
    }
}
