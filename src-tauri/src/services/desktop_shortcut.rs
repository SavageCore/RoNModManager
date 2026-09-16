use super::launch_args::slugify;
use crate::models::{AppError, Result};
use std::path::PathBuf;

pub fn shortcut_slug(profile: &str) -> String {
    format!("ronmm-{}", slugify(profile))
}

fn current_exe_and_args(profile: &str, vanilla: bool) -> Result<(PathBuf, String)> {
    let exe = std::env::current_exe().map_err(AppError::Io)?;
    let args = if vanilla {
        format!(
            "--profile \"{}\" --launch --vanilla --hide",
            profile.replace('"', "")
        )
    } else {
        format!("--profile \"{}\" --launch --hide", profile.replace('"', ""))
    };
    Ok((exe, args))
}

#[cfg(target_os = "linux")]
fn run_host(cmd: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
    if super::flatpak::is_flatpak_sandbox() {
        let mut full = vec!["--host", cmd];
        full.extend(args);
        std::process::Command::new("flatpak-spawn")
            .args(&full)
            .output()
    } else {
        std::process::Command::new(cmd).args(args).output()
    }
}

#[cfg(target_os = "linux")]
fn write_file_maybe_host(path: &PathBuf, content: &str) -> Result<()> {
    if super::flatpak::is_flatpak_sandbox() {
        // Best effort: write via flatpak-spawn --host sh -c with base64 to avoid quoting issues.
        use std::fmt::Write as _;
        let b64 = {
            const CHARS: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let bytes = content.as_bytes();
            let mut s = String::new();
            for chunk in bytes.chunks(3) {
                let n = (chunk[0] as u32) << 16
                    | (*chunk.get(1).unwrap_or(&0) as u32) << 8
                    | (*chunk.get(2).unwrap_or(&0) as u32);
                for k in 0..4 {
                    if k <= chunk.len() {
                        let _ = write!(s, "{}", CHARS[((n >> (18 - 6 * k)) & 63) as usize] as char);
                    } else {
                        s.push('=');
                    }
                }
            }
            s
        };
        let script = format!("mkdir -p \"$(dirname \"{}\")\" && echo \"{}\" | base64 -d > \"{}\" && chmod 755 \"{}\"", path.display(), b64, path.display(), path.display());
        let out = std::process::Command::new("flatpak-spawn")
            .args(["--host", "sh", "-c", &script])
            .output()
            .map_err(AppError::Io)?;
        if out.status.success() {
            return Ok(());
        }
        return Err(AppError::Validation(
            "Flatpak sandbox blocked host write. Copy the .desktop content manually to ~/.local/share/applications/".to_string(),
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }
    std::fs::write(path, content).map_err(AppError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

pub const APP_ID: &str = "uk.savagecore.ronmodmanager";

/// Host-resolvable launch prefix. In a Flatpak sandbox `current_exe()` is
/// /app/bin/... which the host cannot resolve, so emit `flatpak run`.
pub fn desktop_exec_prefix() -> Option<String> {
    if super::flatpak::is_flatpak_sandbox() {
        return Some(format!("flatpak run {}", APP_ID));
    }
    None
}

pub fn desktop_file_content(profile: &str, exe: &str, vanilla: bool) -> String {
    let slug = shortcut_slug(profile);
    let args = if vanilla {
        "--launch --vanilla --hide"
    } else {
        "--launch --hide"
    };
    let safe = profile.replace('"', "");
    let exec = match desktop_exec_prefix() {
        Some(prefix) => format!("{} --profile \"{}\" {} %U", prefix, safe, args),
        None => format!("\"{}\" --profile \"{}\" {} %U", exe, safe, args),
    };
    format!(
        "[Desktop Entry]\nType=Application\nName=RoN - {safe}\nComment=Launch Ready or Not with profile {safe} via RoN Mod Manager\nExec={exec}\nIcon={APP_ID}\nTerminal=false\nCategories=Game;Utility;\nMimeType=x-scheme-handler/ronmm;\nStartupWMClass=ronmodmanager\nX-RoNMM-Profile={safe}\nX-RoNMM-Slug={slug}\n"
    )
}

#[cfg(target_os = "linux")]
fn host_home_dir() -> Option<PathBuf> {
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

pub fn create_desktop_shortcut(
    profile: &str,
    vanilla: bool,
    desktop_copy: bool,
) -> Result<PathBuf> {
    let (exe, _) = current_exe_and_args(profile, vanilla)?;
    let exe_str = exe.to_string_lossy().to_string();
    let content = desktop_file_content(profile, &exe_str, vanilla);
    let slug = shortcut_slug(profile);

    #[cfg(target_os = "linux")]
    let shortcut_path: PathBuf = {
        let home =
            host_home_dir().ok_or_else(|| AppError::NotFound("HOME not found".to_string()))?;
        let apps_path = home
            .join(".local/share/applications")
            .join(format!("{}.desktop", slug));
        if let Err(e) = write_file_maybe_host(&apps_path, &content) {
            // Fallback: write into app config dir so UI can offer manual export.
            let fallback = crate::state::app_config_root()
                .map(|p| p.join("shortcuts").join(format!("{}.desktop", slug)))
                .map_err(|e| AppError::Validation(e.to_string()))?;
            if let Some(parent) = fallback.parent() {
                std::fs::create_dir_all(parent).map_err(AppError::Io)?;
            }
            std::fs::write(&fallback, &content).map_err(AppError::Io)?;
            return Err(e);
        }
        let apps_dir = home.join(".local/share/applications");
        let apps_dir_str = apps_dir.to_string_lossy();
        let _ = run_host("update-desktop-database", &[apps_dir_str.as_ref()]);
        let apps_path_str = apps_path.to_string_lossy();
        let _ = run_host("desktop-file-validate", &[apps_path_str.as_ref()]);
        if desktop_copy {
            let desk = dirs::desktop_dir().unwrap_or(home.join("Desktop"));
            let dest = desk.join(format!("RoN - {}.desktop", profile.replace('/', "-")));
            let _ = write_file_maybe_host(&dest, &content);
            let dest_str = dest.to_string_lossy();
            let _ = run_host(
                "gio",
                &["set", dest_str.as_ref(), "metadata::trusted", "true"],
            );
        }
        apps_path
    };

    #[cfg(target_os = "windows")]
    let shortcut_path: PathBuf = {
        let desktop = dirs::desktop_dir()
            .ok_or_else(|| AppError::NotFound("Desktop not found".to_string()))?;
        let lnk = desktop.join(format!(
            "RoN - {}.lnk",
            profile.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
        ));
        create_windows_lnk(&lnk, &exe, profile, vanilla)?;
        lnk
    };

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    return Err(AppError::Validation(
        "Desktop shortcuts not supported on this OS".to_string(),
    ));

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    Ok(shortcut_path)
}

#[cfg(target_os = "windows")]
fn create_windows_lnk(
    lnk: &std::path::Path,
    exe: &PathBuf,
    profile: &str,
    vanilla: bool,
) -> Result<()> {
    let (_, args) = current_exe_and_args(profile, vanilla)?;
    let workdir = exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    // Use PowerShell WScript.Shell to write a real .lnk without extra deps.
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Arguments='{}';$s.WorkingDirectory='{}';$s.Description='Launch Ready or Not with profile {}';$s.Save()",
        lnk.to_string_lossy().replace('\'', "''"),
        exe.to_string_lossy().replace('\'', "''"),
        args.replace('\'', "''"),
        workdir.replace('\'', "''"),
        profile.replace('\'', "''"),
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(AppError::Io)?;
    if out.status.success() && lnk.exists() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Failed to create shortcut: {}",
            String::from_utf8_lossy(&out.stderr)
        )))
    }
}

pub fn shortcut_status(profile: &str) -> (Option<PathBuf>, bool) {
    #[cfg(target_os = "linux")]
    {
        let slug = shortcut_slug(profile);
        let mut found = None;
        if let Some(home) = dirs::home_dir() {
            let p = home
                .join(".local/share/applications")
                .join(format!("{}.desktop", slug));
            if p.exists() {
                found = Some(p);
            } else if let Some(d) = dirs::desktop_dir() {
                let alt = d.join(format!("RoN - {}.desktop", profile.replace('/', "-")));
                if alt.exists() {
                    found = Some(alt);
                }
            }
        }
        (found, false)
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(d) = dirs::desktop_dir() {
            let p = d.join(format!(
                "RoN - {}.lnk",
                profile.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
            ));
            if p.exists() {
                return (Some(p), false);
            }
        }
        (None, false)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        (None, false)
    }
}

pub fn remove_desktop_shortcut(profile: &str) -> Result<bool> {
    let mut removed = false;
    #[cfg(target_os = "linux")]
    {
        let slug = shortcut_slug(profile);
        let mut candidates = Vec::new();
        if let Some(home) = dirs::home_dir() {
            candidates.push(
                home.join(".local/share/applications")
                    .join(format!("{}.desktop", slug)),
            );
            if let Some(d) = dirs::desktop_dir() {
                candidates.push(d.join(format!("RoN - {}.desktop", profile.replace('/', "-"))));
            }
        }
        // Flatpak fallback copy in config dir.
        if let Ok(root) = crate::state::app_config_root() {
            candidates.push(root.join("shortcuts").join(format!("{}.desktop", slug)));
        }
        for p in candidates {
            if p.exists() {
                // In sandbox, host files can't be unlinked directly; try host rm.
                if super::flatpak::is_flatpak_sandbox()
                    && !p.starts_with(crate::state::app_config_root().unwrap_or_default())
                {
                    let s = p.to_string_lossy().to_string();
                    let _ = std::process::Command::new("flatpak-spawn")
                        .args(["--host", "rm", "-f", &s])
                        .output();
                    removed = true;
                } else if std::fs::remove_file(&p).is_ok() {
                    removed = true;
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(d) = dirs::desktop_dir() {
            let p = d.join(format!(
                "RoN - {}.lnk",
                profile.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
            ));
            if p.exists() && std::fs::remove_file(&p).is_ok() {
                removed = true;
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::isolated_root;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Profile names are unique per test on purpose: `isolated_root` redirects
    /// HOME for the whole process, so two tests sharing a name would fight over
    /// the same `.desktop` files.
    fn unique_profile(tag: &str) -> String {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        format!("Cov {tag} {}", NEXT.fetch_add(1, Ordering::Relaxed))
    }

    #[cfg(target_os = "linux")]
    fn home() -> PathBuf {
        dirs::home_dir().expect("isolated_root sets HOME")
    }

    #[cfg(target_os = "linux")]
    fn apps_path(profile: &str) -> PathBuf {
        home()
            .join(".local/share/applications")
            .join(format!("{}.desktop", shortcut_slug(profile)))
    }

    #[cfg(target_os = "linux")]
    fn desktop_path(profile: &str) -> PathBuf {
        dirs::desktop_dir()
            .expect("user-dirs.dirs seeds a desktop directory")
            .join(format!("RoN - {}.desktop", profile.replace('/', "-")))
    }

    fn current_exe_string() -> String {
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn renders_desktop_entry() {
        let c = desktop_file_content("My Prof", "/usr/bin/ronmodmanager", false);
        assert!(c.contains("Name=RoN - My Prof"));
        assert!(c.contains("--profile \"My Prof\" --launch --hide"));
        assert!(c.contains("ronmm-my-prof"));
        // Sandbox prefix never leaks /app/bin into host .desktop files.
        assert!(desktop_exec_prefix().is_none() || !c.contains("/app/bin"));
    }

    #[test]
    fn shortcut_slug_lowercases_and_dashes_the_profile() {
        assert_eq!(shortcut_slug("My Prof"), "ronmm-my-prof");
        assert_eq!(
            shortcut_slug("  Upper_Case Name  "),
            "ronmm-upper-case-name"
        );
        assert_eq!(shortcut_slug("!!!"), "ronmm-profile");
    }

    #[test]
    fn desktop_entry_carries_profile_metadata_and_strips_quotes() {
        let c = desktop_file_content("Cov \"Quoted\" Prof", "/usr/bin/ronmm", false);
        assert!(c.starts_with("[Desktop Entry]\nType=Application\n"));
        assert!(c.contains("Name=RoN - Cov Quoted Prof\n"));
        assert!(
            c.contains("Exec=\"/usr/bin/ronmm\" --profile \"Cov Quoted Prof\" --launch --hide %U")
        );
        assert!(c.contains(&format!("Icon={APP_ID}")));
        assert!(c.contains("X-RoNMM-Profile=Cov Quoted Prof"));
        assert!(c.contains("X-RoNMM-Slug=ronmm-cov-quoted-prof"));
        assert!(c.contains("MimeType=x-scheme-handler/ronmm;"));
        assert!(!c.contains("--vanilla"));
    }

    #[test]
    fn desktop_entry_marks_vanilla_launch() {
        let c = desktop_file_content("Vanilla Prof", "/usr/bin/ronmm", true);
        assert!(c.contains("--profile \"Vanilla Prof\" --launch --vanilla --hide %U"));
        assert!(
            c.contains("Comment=Launch Ready or Not with profile Vanilla Prof via RoN Mod Manager")
        );
    }

    #[test]
    fn current_exe_and_args_builds_hidden_launch_arguments() {
        assert_eq!(
            current_exe_and_args("Cov \"Q\" Prof", false).unwrap(),
            (
                std::env::current_exe().unwrap(),
                "--profile \"Cov Q Prof\" --launch --hide".to_string()
            )
        );
        assert_eq!(
            current_exe_and_args("Cov Q Prof", true).unwrap().1,
            "--profile \"Cov Q Prof\" --launch --vanilla --hide"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn run_host_propagates_exit_status_and_missing_program() {
        let out = run_host("sh", &["-c", "exit 7"]).unwrap();
        assert_eq!(out.status.code(), Some(7));
        assert!(run_host("ronmm-no-such-program-xyz", &[]).is_err());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn create_writes_launcher_into_home_applications_dir() {
        isolated_root();
        let profile = unique_profile("Create");
        let path = create_desktop_shortcut(&profile, false, false).unwrap();

        assert_eq!(path, apps_path(&profile));
        let expected = desktop_file_content(&profile, &current_exe_string(), false);
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755, "launchers must be executable");
        }

        // Re-creating an existing shortcut is idempotent.
        assert_eq!(
            create_desktop_shortcut(&profile, false, false).unwrap(),
            path
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn create_records_vanilla_launch_in_the_installed_entry() {
        isolated_root();
        let profile = unique_profile("Vanilla");
        let path = create_desktop_shortcut(&profile, true, false).unwrap();
        let content = fs::read_to_string(&path).unwrap();

        assert!(content.contains("--launch --vanilla --hide"));
        assert_eq!(
            content,
            desktop_file_content(&profile, &current_exe_string(), true)
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn create_with_desktop_copy_places_a_copy_on_the_desktop() {
        isolated_root();
        let profile = unique_profile("Copy");
        let path = create_desktop_shortcut(&profile, false, true).unwrap();

        let copy = desktop_path(&profile);
        assert!(copy.exists());
        assert_eq!(
            fs::read_to_string(&copy).unwrap(),
            fs::read_to_string(&path).unwrap()
        );
        // The applications entry is still the one status reports.
        assert_eq!(shortcut_status(&profile), (Some(path), false));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn create_falls_back_to_app_config_dir_when_the_home_write_fails() {
        isolated_root();
        let profile = unique_profile("Blocked");
        let blocked = apps_path(&profile);
        // A directory where the .desktop file belongs makes the host write fail.
        fs::create_dir_all(&blocked).unwrap();

        let err = create_desktop_shortcut(&profile, false, false).unwrap_err();
        assert!(
            matches!(err, AppError::Io(_)),
            "expected I/O error, got {err:?}"
        );

        let fallback = crate::state::app_config_root()
            .unwrap()
            .join("shortcuts")
            .join(format!("{}.desktop", shortcut_slug(&profile)));
        assert_eq!(
            fs::read_to_string(&fallback).unwrap(),
            desktop_file_content(&profile, &current_exe_string(), false)
        );

        fs::remove_dir_all(&blocked).unwrap();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn shortcut_status_reports_absent_and_installed_launchers() {
        isolated_root();
        assert_eq!(shortcut_status(&unique_profile("Absent")), (None, false));

        let profile = unique_profile("Installed");
        let path = create_desktop_shortcut(&profile, false, false).unwrap();
        assert_eq!(shortcut_status(&profile), (Some(path), false));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn shortcut_status_falls_back_to_the_desktop_copy() {
        isolated_root();
        let profile = unique_profile("DesktopOnly");
        let apps = create_desktop_shortcut(&profile, false, true).unwrap();

        fs::remove_file(&apps).unwrap();
        assert_eq!(
            shortcut_status(&profile),
            (Some(desktop_path(&profile)), false)
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn remove_deletes_home_and_desktop_launchers() {
        isolated_root();
        // Nothing installed: a no-op, not an error.
        assert!(!remove_desktop_shortcut(&unique_profile("RemoveAbsent")).unwrap());

        let profile = unique_profile("RemoveBoth");
        let apps = create_desktop_shortcut(&profile, false, true).unwrap();
        let copy = desktop_path(&profile);
        assert!(apps.exists() && copy.exists());

        assert!(remove_desktop_shortcut(&profile).unwrap());
        assert!(!apps.exists());
        assert!(!copy.exists());
        // Second removal reports nothing left to delete.
        assert!(!remove_desktop_shortcut(&profile).unwrap());
    }
}
