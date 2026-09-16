use crate::models::{AppError, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

// Minimal binary-VDF support for Steam shortcuts.vdf.
// Format: sequence of (u8 type, cstring name, payload), terminated by 0x08.
// Types we use: 0x00 = object, 0x01 = string, 0x02 = int32. 0x08 = end.

#[derive(Debug, Clone)]
pub struct ShortcutEntry {
    pub app_name: String,
    pub exe: String,
    pub start_dir: String,
    pub launch_options: String,
    pub icon: String,
}

impl ShortcutEntry {
    pub fn for_profile(profile: &str, exe: &str, start_dir: &str, vanilla: bool) -> Self {
        let safe = profile.replace('"', "");
        let launch_options = if vanilla {
            format!("--profile \"{safe}\" --launch --vanilla --hide")
        } else {
            format!("--profile \"{safe}\" --launch --hide")
        };
        Self {
            app_name: format!("RoN - {safe}"),
            exe: exe.to_string(),
            start_dir: start_dir.to_string(),
            launch_options,
            icon: String::new(),
        }
    }
    fn fields(&self) -> Vec<(&str, &str)> {
        vec![
            ("AppName", self.app_name.as_str()),
            ("Exe", self.exe.as_str()),
            ("StartDir", self.start_dir.as_str()),
            ("LaunchOptions", self.launch_options.as_str()),
            ("ShortcutPath", ""),
            ("Icon", self.icon.as_str()),
        ]
    }
}

fn read_cstring(data: &[u8], pos: &mut usize) -> Result<String> {
    let start = *pos;
    while *pos < data.len() && data[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= data.len() {
        return Err(AppError::Validation("Truncated shortcuts.vdf".to_string()));
    }
    let s = String::from_utf8_lossy(&data[start..*pos]).to_string();
    *pos += 1;
    Ok(s)
}

fn write_cstring(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

/// Raw children of one VDF object: (field type, name, raw payload bytes).
type VdfChildren = Vec<(u8, String, Vec<u8>)>;

/// Parse top-level "shortcuts" -> index -> string/int fields. Unknown nesting is skipped best-effort.
pub fn parse_shortcuts(data: &[u8]) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let mut pos = 0usize;
    let mut result: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    // Expect 0x00 "shortcuts"
    if data.is_empty() {
        return Ok(result);
    }
    // Generic recursive parse limited to 3 levels.
    fn parse_obj(data: &[u8], pos: &mut usize, depth: u8) -> Result<(String, VdfChildren)> {
        if *pos >= data.len() {
            return Err(AppError::Validation("Truncated VDF".to_string()));
        }
        let t = data[*pos];
        *pos += 1;
        if t == 0x08 {
            return Ok((String::new(), vec![]));
        }
        let name = read_cstring(data, pos)?;
        let mut children = Vec::new();
        if t == 0x00 {
            loop {
                if *pos >= data.len() {
                    break;
                }
                if data[*pos] == 0x08 {
                    *pos += 1;
                    break;
                }
                let ct = data[*pos];
                *pos += 1;
                let cn = read_cstring(data, pos)?;
                if ct == 0x00 {
                    if depth < 3 {
                        // recurse: collect raw by re-parsing
                        let mut sub: Vec<(u8, String, Vec<u8>)> = Vec::new();
                        loop {
                            if *pos >= data.len() || data[*pos] == 0x08 {
                                if *pos < data.len() {
                                    *pos += 1;
                                }
                                break;
                            }
                            let st = data[*pos];
                            *pos += 1;
                            let sn = read_cstring(data, pos)?;
                            if st == 0x00 {
                                // Nested object (e.g. empty "tags"): consume to its 0x08 end.
                                let mut depth = 1usize;
                                while depth > 0 {
                                    if *pos >= data.len() {
                                        return Err(AppError::Validation(
                                            "Truncated VDF".to_string(),
                                        ));
                                    }
                                    let t = data[*pos];
                                    *pos += 1;
                                    if t == 0x08 {
                                        depth -= 1;
                                    } else if t == 0x00 {
                                        let _ = read_cstring(data, pos)?;
                                        depth += 1;
                                    } else if t == 0x01 {
                                        let _ = read_cstring(data, pos)?;
                                        let _ = read_cstring(data, pos)?;
                                    } else if t == 0x02 {
                                        *pos += 4;
                                    } else {
                                        return Err(AppError::Validation(
                                            "Unsupported VDF field type".to_string(),
                                        ));
                                    }
                                }
                                let _ = sn;
                                continue;
                            } else if st == 0x01 {
                                let sv = read_cstring(data, pos)?;
                                let mut raw = Vec::new();
                                raw.extend_from_slice(sv.as_bytes());
                                raw.push(0);
                                sub.push((st, sn, raw));
                            } else if st == 0x02 {
                                let mut raw = vec![0u8; 4];
                                raw.copy_from_slice(&data[*pos..*pos + 4]);
                                *pos += 4;
                                sub.push((st, sn, raw));
                            } else {
                                // skip unknown: bail
                                return Err(AppError::Validation(
                                    "Unsupported VDF field type".to_string(),
                                ));
                            }
                        }
                        // encode sub back as raw bytes for storage
                        let mut raw = Vec::new();
                        for (st, sn, rv) in sub {
                            raw.push(st);
                            write_cstring(&mut raw, &sn);
                            raw.extend_from_slice(&rv);
                        }
                        children.push((ct, cn, raw));
                    } else {
                        return Err(AppError::Validation("VDF too deep".to_string()));
                    }
                } else if ct == 0x01 {
                    let v = read_cstring(data, pos)?;
                    let mut raw = Vec::new();
                    raw.extend_from_slice(v.as_bytes());
                    raw.push(0);
                    children.push((ct, cn, raw));
                } else if ct == 0x02 {
                    let mut raw = vec![0u8; 4];
                    raw.copy_from_slice(&data[*pos..*pos + 4]);
                    *pos += 4;
                    children.push((ct, cn, raw));
                } else {
                    return Err(AppError::Validation(
                        "Unsupported VDF field type".to_string(),
                    ));
                }
            }
        }
        Ok((name, children))
    }

    let (top_name, entries) = parse_obj(data, &mut pos, 0)?;
    if top_name != "shortcuts" {
        return Ok(result);
    }
    for (_, idx, raw) in entries {
        // raw contains sequence of (type,name,value) without end marker
        let mut p = 0usize;
        let mut map = BTreeMap::new();
        while p < raw.len() {
            let t = raw[p];
            p += 1;
            let end = raw[p..]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| AppError::Validation("Bad VDF".to_string()))?;
            let name = String::from_utf8_lossy(&raw[p..p + end]).to_string();
            p += end + 1;
            if t == 0x01 {
                let end2 = raw[p..]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or_else(|| AppError::Validation("Bad VDF".to_string()))?;
                let val = String::from_utf8_lossy(&raw[p..p + end2]).to_string();
                p += end2 + 1;
                map.insert(name, val);
            } else if t == 0x02 {
                let v =
                    i32::from_le_bytes([raw[p], raw[p + 1], raw[p + 2], raw[p + 3]]).to_string();
                p += 4;
                map.insert(name, v);
            } else {
                break;
            }
        }
        result.insert(idx, map);
    }
    Ok(result)
}

pub fn serialize_shortcuts(entries: &BTreeMap<String, BTreeMap<String, String>>) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x00);
    write_cstring(&mut out, "shortcuts");
    // Preserve insertion order by numeric index sort
    let mut keys: Vec<&String> = entries.keys().collect();
    keys.sort_by_key(|k| k.parse::<u32>().unwrap_or(u32::MAX));
    for k in keys {
        out.push(0x00);
        write_cstring(&mut out, k);
        if let Some(map) = entries.get(k) {
            for (fk, fv) in map {
                // Int-ish fields stored as int32
                if [
                    "IsHidden",
                    "AllowDesktopConfig",
                    "AllowOverlay",
                    "OpenVR",
                    "Devkit",
                    "LastPlayTime",
                ]
                .contains(&fk.as_str())
                {
                    out.push(0x02);
                    write_cstring(&mut out, fk);
                    let n: i32 = fv.parse().unwrap_or(0);
                    out.extend_from_slice(&n.to_le_bytes());
                } else {
                    out.push(0x01);
                    write_cstring(&mut out, fk);
                    write_cstring(&mut out, fv);
                }
            }
            // Tags array (empty object) required by Steam
            out.push(0x00);
            write_cstring(&mut out, "tags");
            out.push(0x08);
        }
        out.push(0x08);
    }
    out.push(0x08);
    out.push(0x08);
    out
}

pub const STEAM_RUNNING_MSG: &str = "Steam is still running (check tray) - quit Steam, then retry.";

pub const APP_ID: &str = "uk.savagecore.ronmodmanager";

/// Host HOME: in a Flatpak sandbox `dirs::home_dir()` points at the sandbox
/// home, while native Steam lives under the real host HOME. Ask the host.
#[cfg(target_os = "linux")]
pub fn host_home_dir() -> Option<PathBuf> {
    if super::flatpak::is_flatpak_sandbox() {
        if let Ok(out) = std::process::Command::new("flatpak-spawn")
            .args(["--host", "sh", "-c", "printf %s \"$HOME\""])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(PathBuf::from(s));
                }
            }
        }
    }
    dirs::home_dir()
}

#[cfg(not(target_os = "linux"))]
pub fn host_home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

fn steam_userdata_dirs() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        for sub in [
            "SOFTWARE\\Wow6432Node\\Valve\\Steam",
            "SOFTWARE\\Valve\\Steam",
        ] {
            if let Ok(k) = hklm.open_subkey(sub) {
                if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                    roots.push(PathBuf::from(p));
                }
            }
        }
        if let Ok(lp) = std::env::var("PROGRAMFILES(X86)") {
            roots.push(PathBuf::from(lp).join("Steam"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = host_home_dir() {
            for p in [
                home.join(".steam/steam"),
                home.join(".local/share/Steam"),
                home.join(".var/app/com.valvesoftware.Steam/.steam/steam"),
                home.join("snap/steam/common/.steam/steam"),
            ] {
                roots.push(p);
            }
        }
    }
    let mut out = Vec::new();
    let sandboxed = super::flatpak::is_flatpak_sandbox();
    for r in roots {
        log::info!("Checking Steam root for shortcuts: {}", r.display());
        let ud = r.join("userdata");
        // In a sandbox the host userdata dir is invisible to direct fs calls,
        // so list it via flatpak-spawn --host.
        if sandboxed {
            #[cfg(target_os = "linux")]
            {
                let ud_s = ud.to_string_lossy().to_string();
                let Ok(listing) = std::process::Command::new("flatpak-spawn")
                    .args(["--host", "ls", "-1", &ud_s])
                    .output()
                else {
                    continue;
                };
                if !listing.status.success() {
                    continue;
                }
                for line in String::from_utf8_lossy(&listing.stdout).lines() {
                    let name = line.trim();
                    if name.is_empty() {
                        continue;
                    }
                    out.push(ud.join(name).join("config").join("shortcuts.vdf"));
                }
                continue;
            }
        }
        if let Ok(rd) = std::fs::read_dir(&ud) {
            for e in rd.flatten() {
                let cfg = e.path().join("config").join("shortcuts.vdf");
                // Include parent dir even if file missing so we can create it.
                if e.path().is_dir() {
                    out.push(cfg);
                }
            }
        }
    }
    // Dedupe (symlinked .steam/steam vs .local/share/Steam).
    out.sort();
    out.dedup();
    out
}

pub fn is_steam_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq steam.exe", "/NH"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .to_lowercase()
                    .contains("steam.exe")
            })
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        // Check both `steam` and `steamwebhelper`: closing the Steam window
        // leaves it lingering in the tray, which users mistake for "closed".
        // Log the raw result so tray-lingering is diagnosable.
        fn pgrep_host(name: &str, sandboxed: bool) -> bool {
            let out = if sandboxed {
                std::process::Command::new("flatpak-spawn")
                    .args(["--host", "pgrep", "-x", name])
                    .output()
            } else {
                std::process::Command::new("pgrep")
                    .args(["-x", name])
                    .output()
            };
            match out {
                Ok(o) => {
                    let hit = o.status.success();
                    log::info!(
                        "steam process check pgrep -x {} (sandbox={}): running={}",
                        name,
                        sandboxed,
                        hit
                    );
                    hit
                }
                Err(e) => {
                    log::warn!("steam process check failed for {}: {}", name, e);
                    false
                }
            }
        }
        let sandboxed = super::flatpak::is_flatpak_sandbox();
        pgrep_host("steam", sandboxed) || pgrep_host("steamwebhelper", sandboxed)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

fn read_host_file(p: &PathBuf) -> Result<Option<Vec<u8>>> {
    if super::flatpak::is_flatpak_sandbox() {
        #[cfg(target_os = "linux")]
        {
            let s = p.to_string_lossy().to_string();
            let out = std::process::Command::new("flatpak-spawn")
                .args(["--host", "cat", &s])
                .output()
                .map_err(AppError::Io)?;
            if out.status.success() {
                return Ok(Some(out.stdout));
            }
            return Ok(None);
        }
    }
    if !p.exists() {
        return Ok(None);
    }
    Ok(Some(std::fs::read(p).map_err(AppError::Io)?))
}

fn write_host_file(p: &PathBuf, data: &[u8]) -> Result<()> {
    // Backup once (backup file sits next to the target, with .ronmm.bak suffix).
    let bak_name = format!(
        "{}.ronmm.bak",
        p.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    );
    let bak = p.with_file_name(bak_name);
    #[cfg(target_os = "linux")]
    if super::flatpak::is_flatpak_sandbox() {
        // Host write via flatpak-spawn: stage in temp, then backup-once + move.
        let host_tmp = format!("/tmp/ronmm-shortcuts-{}.vdf", std::process::id());
        // Write bytes locally first (base64 to survive the shell hop).
        let staging =
            std::env::temp_dir().join(format!("ronmm-shortcuts-{}.b64", std::process::id()));
        {
            use std::io::Write as _;
            const CHARS: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = String::new();
            for chunk in data.chunks(3) {
                let n = (chunk[0] as u32) << 16
                    | (*chunk.get(1).unwrap_or(&0) as u32) << 8
                    | (*chunk.get(2).unwrap_or(&0) as u32);
                for k in 0..4 {
                    if k <= chunk.len() {
                        s.push(CHARS[((n >> (18 - 6 * k)) & 63) as usize] as char);
                    } else {
                        s.push('=');
                    }
                }
            }
            let mut f = std::fs::File::create(&staging).map_err(AppError::Io)?;
            f.write_all(s.as_bytes()).map_err(AppError::Io)?;
        }
        // Decode staging into host tmp via host shell redirect.
        let b64 = std::fs::read_to_string(&staging).map_err(AppError::Io)?;
        let _ = std::fs::remove_file(&staging);
        let target = p.to_string_lossy().to_string();
        let bak_s = bak.to_string_lossy().to_string();
        let script = format!(
            "echo '{b64}' | base64 -d > '{tmp}' && mkdir -p \"$(dirname '{t}')\" && [ -e '{b}' ] || [ ! -e '{t}' ] || cp '{t}' '{b}'; cat '{tmp}' > '{t}' && rm -f '{tmp}'",
            b64 = b64, tmp = host_tmp, t = target, b = bak_s
        );
        let out = std::process::Command::new("flatpak-spawn")
            .args(["--host", "sh", "-c", &script])
            .output()
            .map_err(AppError::Io)?;
        if out.status.success() {
            log::info!("Wrote host Steam shortcuts: {}", target);
            return Ok(());
        }
        return Err(AppError::Validation(format!(
            "Sandbox blocked host write to {}. {}",
            target,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }
    if !bak.exists() && p.exists() {
        let _ = std::fs::copy(p, &bak);
    }
    std::fs::write(p, data).map_err(AppError::Io)?;
    Ok(())
}

/// Ask Steam to quit gracefully so shortcuts.vdf can be edited.
/// Never launches Steam: if nothing is running this is a no-op success, since
/// `steam -shutdown` on a stopped client would boot Steam just to quit it.
pub fn quit_steam() -> Result<String> {
    if !is_steam_running() {
        return Ok("Steam is not running - nothing to quit.".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("taskkill")
            .args(["/IM", "steam.exe"])
            .output()
            .map_err(AppError::Io)?;
        if out.status.success() || is_steam_running() == false {
            return Ok("Steam quit requested.".to_string());
        }
        return Err(AppError::Validation(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    #[cfg(target_os = "linux")]
    {
        // `steam -shutdown` is the graceful path for native Steam.
        let args = ["steam", "-shutdown"];
        let out = if super::flatpak::is_flatpak_sandbox() {
            std::process::Command::new("flatpak-spawn")
                .args(["--host", args[0], args[1]])
                .output()
                .map_err(AppError::Io)?
        } else {
            std::process::Command::new(args[0])
                .arg(args[1])
                .output()
                .map_err(AppError::Io)?
        };
        if out.status.success() {
            return Ok("Steam shutdown requested. Wait a few seconds, then retry.".to_string());
        }
        Err(AppError::Validation(format!(
            "Could not quit Steam: {}. Quit it manually (check tray).",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err(AppError::Validation(
            "Quitting Steam is not supported on this OS".to_string(),
        ))
    }
}

pub fn shortcut_targets(profile: &str, vanilla: bool) -> Result<(String, String, ShortcutEntry)> {
    // In a Flatpak sandbox current_exe() is /app/bin/... which the host Steam
    // client cannot resolve. Point host-side entries at the flatpak runner.
    if super::flatpak::is_flatpak_sandbox() {
        let safe = profile.replace('"', "");
        let launch_options = if vanilla {
            format!("run {APP_ID} --profile \"{safe}\" --launch --vanilla --hide")
        } else {
            format!("run {APP_ID} --profile \"{safe}\" --launch --hide")
        };
        let entry = ShortcutEntry {
            app_name: format!("RoN - {safe}"),
            exe: "/usr/bin/flatpak".to_string(),
            start_dir: "/".to_string(),
            launch_options,
            icon: String::new(),
        };
        return Ok(("/usr/bin/flatpak".to_string(), "/".to_string(), entry));
    }
    let exe = std::env::current_exe().map_err(AppError::Io)?;
    let start_dir = exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let entry = ShortcutEntry::for_profile(profile, &exe.to_string_lossy(), &start_dir, vanilla);
    Ok((exe.to_string_lossy().to_string(), start_dir, entry))
}

pub fn add_to_steam(profile: &str, vanilla: bool) -> Result<Vec<PathBuf>> {
    if is_steam_running() {
        return Err(AppError::Validation(STEAM_RUNNING_MSG.to_string()));
    }
    let (_, _, entry) = shortcut_targets(profile, vanilla)?;
    let files = steam_userdata_dirs();
    if files.is_empty() {
        return Err(AppError::NotFound(
            "No Steam userdata shortcuts.vdf found".to_string(),
        ));
    }
    let mut updated = Vec::new();
    for f in files {
        let existing = read_host_file(&f)?.unwrap_or_default();
        let mut map = if existing.is_empty() {
            BTreeMap::new()
        } else {
            parse_shortcuts(&existing).unwrap_or_default()
        };
        // Remove any existing entry with same AppName, then append.
        map.retain(|_, v| {
            v.get("AppName")
                .map(|s| s != &entry.app_name)
                .unwrap_or(true)
        });
        let next: u32 = map
            .keys()
            .filter_map(|k| k.parse::<u32>().ok())
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        let mut fields = BTreeMap::new();
        for (k, v) in entry.fields() {
            fields.insert(k.to_string(), v.to_string());
        }
        fields.insert("IsHidden".into(), "0".into());
        fields.insert("AllowDesktopConfig".into(), "1".into());
        fields.insert("AllowOverlay".into(), "1".into());
        fields.insert("OpenVR".into(), "0".into());
        fields.insert("Devkit".into(), "0".into());
        fields.insert("LastPlayTime".into(), "0".into());
        map.insert(next.to_string(), fields);
        let bytes = serialize_shortcuts(&map);
        write_host_file(&f, &bytes)?;
        updated.push(f);
    }
    Ok(updated)
}

pub fn remove_from_steam(profile: &str) -> Result<Vec<PathBuf>> {
    if is_steam_running() {
        return Err(AppError::Validation(STEAM_RUNNING_MSG.to_string()));
    }
    let target = format!("RoN - {}", profile.replace('"', ""));
    let mut updated = Vec::new();
    for f in steam_userdata_dirs() {
        let Some(existing) = read_host_file(&f)? else {
            continue;
        };
        if existing.is_empty() {
            continue;
        }
        let mut map = parse_shortcuts(&existing).unwrap_or_default();
        let before = map.len();
        map.retain(|_, v| v.get("AppName").map(|s| s != &target).unwrap_or(true));
        if map.len() != before {
            // Re-index to keep 0..n contiguous
            let vals: Vec<_> = {
                let mut ks: Vec<_> = map.keys().cloned().collect();
                ks.sort_by_key(|k| k.parse::<u32>().unwrap_or(u32::MAX));
                ks.into_iter().filter_map(|k| map.remove(&k)).collect()
            };
            let remapped: BTreeMap<String, BTreeMap<String, String>> = vals
                .into_iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v))
                .collect();
            write_host_file(&f, &serialize_shortcuts(&remapped))?;
            updated.push(f);
        }
    }
    Ok(updated)
}

pub fn steam_status(profile: &str) -> Result<bool> {
    let target = format!("RoN - {}", profile.replace('"', ""));
    for f in steam_userdata_dirs() {
        if let Some(data) = read_host_file(&f)? {
            if data.is_empty() {
                continue;
            }
            if let Ok(map) = parse_shortcuts(&data) {
                if map
                    .values()
                    .any(|v| v.get("AppName").map(|s| s == &target).unwrap_or(false))
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trips_entries() {
        let mut map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut f = BTreeMap::new();
        f.insert("AppName".into(), "RoN - Foo".into());
        f.insert("Exe".into(), "/usr/bin/ronmodmanager".into());
        f.insert("IsHidden".into(), "0".into());
        map.insert("0".into(), f);
        let bytes = serialize_shortcuts(&map);
        let parsed = parse_shortcuts(&bytes).unwrap();
        assert_eq!(parsed["0"]["AppName"], "RoN - Foo");
        assert_eq!(parsed["0"]["Exe"], "/usr/bin/ronmodmanager");
    }
    #[test]
    fn builds_launch_options() {
        let e = ShortcutEntry::for_profile("Foo", "/exe", "/dir", false);
        assert_eq!(e.app_name, "RoN - Foo");
        assert!(e
            .launch_options
            .contains("--profile \"Foo\" --launch --hide"));
    }

    mod filesystem {
        use super::*;
        use crate::test_support::{isolated_root, shared_tree_guard};
        use std::fs;
        use std::sync::MutexGuard;

        fn shortcuts_path() -> PathBuf {
            isolated_root().join(".steam/steam/userdata/1/config/shortcuts.vdf")
        }

        /// The userdata tree is shared process state, so the file-touching
        /// tests below serialise on the Steam-tree guard (which the tests that
        /// assert no Steam install exists also take) and start from a clean
        /// file.
        fn reset_userdata() -> (MutexGuard<'static, ()>, PathBuf) {
            let guard = shared_tree_guard();
            let path = shortcuts_path();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"").unwrap();
            (guard, path)
        }

        fn store_entry(app_name: &str) {
            let mut map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
            let mut fields = BTreeMap::new();
            fields.insert("AppName".to_string(), app_name.to_string());
            fields.insert("Exe".to_string(), "/usr/bin/ronmodmanager".to_string());
            map.insert("0".to_string(), fields);
            write_host_file(&shortcuts_path(), &serialize_shortcuts(&map)).unwrap();
        }

        #[test]
        fn shortcut_targets_point_at_this_binary() {
            isolated_root();

            let (exe, start_dir, entry) = shortcut_targets("Some \"Profile\"", false).unwrap();

            assert_eq!(entry.app_name, "RoN - Some Profile");
            assert!(entry.launch_options.contains("--launch --hide"));
            assert!(!entry.launch_options.contains("--vanilla"));
            assert_eq!(entry.exe, exe);
            assert!(exe.contains("ronmodmanager"));
            assert!(!start_dir.is_empty());

            let (_, _, vanilla) = shortcut_targets("Some Profile", true).unwrap();
            assert!(vanilla.launch_options.contains("--vanilla"));
        }

        #[test]
        fn host_files_round_trip_and_missing_files_read_as_none() {
            isolated_root();
            let dir = isolated_root().join("host-files");
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join("shortcuts.vdf");

            assert!(read_host_file(&path).unwrap().is_none());

            write_host_file(&path, b"payload").unwrap();
            assert_eq!(read_host_file(&path).unwrap().unwrap(), b"payload");
        }

        #[test]
        fn steam_status_is_false_without_a_matching_entry() {
            let (_guard, _path) = reset_userdata();

            assert!(!steam_status("Unknown Profile").unwrap());

            store_entry("RoN - Known Profile");
            assert!(steam_status("Known Profile").unwrap());
            assert!(!steam_status("Someone Else").unwrap());
        }

        #[test]
        fn profiles_are_added_to_and_removed_from_steam_shortcuts() {
            let (_guard, path) = reset_userdata();
            if is_steam_running() {
                // Steam rewrites shortcuts.vdf on exit, so the real code refuses
                // to touch it while the client is open - same here.
                eprintln!("skipped: Steam is running");
                return;
            }

            let updated = add_to_steam("Filesystem Profile", false).unwrap();
            assert_eq!(updated, vec![path.clone()]);
            assert!(steam_status("Filesystem Profile").unwrap());

            // Re-adding replaces the existing entry instead of duplicating it.
            add_to_steam("Filesystem Profile", false).unwrap();
            let stored = parse_shortcuts(&read_host_file(&path).unwrap().unwrap()).unwrap();
            let matches = stored
                .values()
                .filter(|v| {
                    v.get("AppName")
                        .map(|n| n == "RoN - Filesystem Profile")
                        .unwrap_or(false)
                })
                .count();
            assert_eq!(matches, 1);

            let removed = remove_from_steam("Filesystem Profile").unwrap();
            assert_eq!(removed, vec![path.clone()]);
            assert!(!steam_status("Filesystem Profile").unwrap());
        }

        #[test]
        fn removing_an_absent_profile_leaves_steam_alone() {
            let (_guard, path) = reset_userdata();
            store_entry("RoN - Kept Profile");
            if is_steam_running() {
                eprintln!("skipped: Steam is running");
                return;
            }

            let updated = remove_from_steam("Not In Steam").unwrap();

            assert!(updated.is_empty());
            assert!(steam_status("Kept Profile").unwrap());
            assert_eq!(
                parse_shortcuts(&read_host_file(&path).unwrap().unwrap())
                    .unwrap()
                    .len(),
                1
            );
        }
    }
}
