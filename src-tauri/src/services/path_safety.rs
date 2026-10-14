//! Guards for untrusted names that end up as filesystem paths.
//!
//! Three separate inputs decide where a file lands: the name inside a mod
//! archive, the key inside a shared modpack, and the filename a download API
//! reports. All three are attacker-controlled - a modpack can arrive from a URL
//! or a `ronmm://modpack/...` link, and an archive from any mod page - and all
//! three used to reach `Path::join` unchecked.
//!
//! `Path::join` is not a containment check: joining an absolute path discards
//! the base entirely, and joining a `..` segment walks out of it. Every place
//! that turns an untrusted name into a destination goes through [`safe_join`].

use std::path::{Path, PathBuf};

use crate::models::{AppError, Result};

/// Characters treated as path separators on every platform.
///
/// Archives are authored on Windows as often as anywhere else, so a `..\..\x`
/// entry has to be rejected when it is extracted on Linux too, where the
/// backslash is otherwise an ordinary filename character.
const SEPARATORS: [char; 2] = ['/', '\\'];

fn reject(reason: &str, name: &str) -> AppError {
    AppError::Validation(format!("unsafe path {name:?}: {reason}"))
}

/// Join an untrusted relative name onto `base`, refusing anything that would
/// land outside it.
///
/// Rejects, on all platforms:
/// * `..` segments, in either separator style,
/// * absolute names (a leading `/` or `\`, or a Windows `C:` prefix) - these
///   are refused rather than resolved under `base`, so the caller can say the
///   archive named an absolute path instead of quietly relocating the file,
/// * embedded NUL bytes and `:` (drive prefixes and Windows-illegal names),
/// * names that reduce to nothing, which would target `base` itself.
///
/// A `.` segment is dropped rather than rejected, and empty segments from
/// doubled separators are collapsed, so ordinary names such as
/// `ReadyOrNot/Content/x.pak` and `a//b` resolve as users expect.
pub fn safe_join(base: &Path, relative: &str) -> Result<PathBuf> {
    if relative.starts_with(SEPARATORS) {
        return Err(reject("absolute path", relative));
    }

    let mut result = base.to_path_buf();

    for segment in relative.split(SEPARATORS) {
        match segment {
            "" | "." => continue,
            ".." => return Err(reject("parent-directory segment", relative)),
            segment => {
                if segment.contains('\0') {
                    return Err(reject("NUL byte", relative));
                }
                if segment.contains(':') {
                    return Err(reject("drive prefix or colon", relative));
                }
                result.push(segment);
            }
        }
    }

    if result == base {
        return Err(reject("empty relative path", relative));
    }

    Ok(result)
}

/// Validate a single name that will be used as one path component.
///
/// Stricter than [`safe_join`] because these names also become directory names
/// the user sees in a profile (`Cool Mod`, not `Cool/Mod`), so separators are
/// refused instead of being read as a path. Follows the rules already applied
/// to UE4SS mod names in `ue4ss::sanitize_mod_name`.
pub fn sanitize_file_component(name: &str) -> Result<String> {
    let trimmed = name.trim();

    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains(SEPARATORS)
        || trimmed.contains('\0')
        || trimmed.contains(':')
    {
        return Err(AppError::Validation(format!("invalid file name '{name}'")));
    }

    Ok(trimmed.to_string())
}

/// Turn a name reported by a mod API into one safe path component.
///
/// Unlike [`sanitize_file_component`] this repairs rather than refuses, because
/// the name comes from a mod listing the user chose and a stray `:` should not
/// fail the install. Path separators and other characters that are illegal in a
/// filename become `_`, which also means no traversal can survive. What is left
/// is refused: `.` and `..` would target the parent, and an empty name would
/// target the containing directory. Callers supply their own fallback.
pub fn sanitize_archive_filename(name: &str) -> Result<String> {
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            _ => c,
        })
        .collect();

    let trimmed = replaced.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(AppError::Validation(format!("invalid file name '{name}'")));
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(relative: &str) -> Result<PathBuf> {
        safe_join(Path::new("/staged/archives"), relative)
    }

    #[test]
    fn ordinary_names_resolve_under_the_base() {
        assert_eq!(
            joined("mod.zip").unwrap(),
            PathBuf::from("/staged/archives/mod.zip")
        );
        assert_eq!(
            joined("Cool Mod/pak_files/x.pak").unwrap(),
            PathBuf::from("/staged/archives/Cool Mod/pak_files/x.pak")
        );
    }

    #[test]
    fn redundant_separators_and_dot_segments_are_collapsed() {
        assert_eq!(
            joined("a//b/./c.pak").unwrap(),
            PathBuf::from("/staged/archives/a/b/c.pak")
        );
        assert_eq!(
            joined("./mod.zip").unwrap(),
            PathBuf::from("/staged/archives/mod.zip")
        );
    }

    #[test]
    fn parent_traversal_is_rejected() {
        for name in [
            "../escape",
            "../../../../home/user/.bashrc",
            "a/../../escape",
            "..",
            "/../escape",
        ] {
            assert!(joined(name).is_err(), "expected {name:?} to be rejected");
        }
    }

    #[test]
    fn windows_separators_are_treated_as_separators_on_every_platform() {
        // A `..\` escape authored on Windows must not pass on Linux, where the
        // backslash would otherwise be an ordinary filename character.
        assert!(joined(r"..\..\escape").is_err());
        assert!(joined(r"a\..\..\escape").is_err());
        assert!(joined(r"a\b\c.pak").is_ok());
    }

    #[test]
    fn absolute_and_drive_prefixed_names_are_rejected() {
        for name in [
            "/etc/passwd",
            r"\etc\passwd",
            r"C:\Windows\System32\config",
            "C:/Windows/System32/config",
        ] {
            assert!(joined(name).is_err(), "expected {name:?} to be rejected");
        }
    }

    #[test]
    fn nul_and_colon_names_are_rejected() {
        assert!(joined("mod\0.zip").is_err());
        assert!(joined("mod:stream.zip").is_err());
    }

    #[test]
    fn names_that_resolve_to_nothing_are_rejected() {
        // These would otherwise target `base` itself.
        for name in ["", ".", "./", "//", r".\"] {
            assert!(joined(name).is_err(), "expected {name:?} to be rejected");
        }
    }

    #[test]
    fn a_traversal_cannot_escape_even_with_a_deep_base() {
        let base = Path::new("/home/user/.config/app/staged/archives");
        assert!(safe_join(base, "../../../..").is_err());
        assert!(safe_join(base, "../../../../root/.ssh/authorized_keys").is_err());
    }

    #[test]
    fn components_reject_traversal_and_separators() {
        assert_eq!(sanitize_file_component("Cool Mod").unwrap(), "Cool Mod");
        assert_eq!(
            sanitize_file_component("  spaced.zip  ").unwrap(),
            "spaced.zip"
        );

        for name in ["", "   ", ".", "..", "a/b", r"a\b", "a\0b", "a:b", "../x"] {
            assert!(
                sanitize_file_component(name).is_err(),
                "expected {name:?} to be rejected"
            );
        }
    }

    #[test]
    fn components_keep_reserved_characters_that_are_legal_in_a_component() {
        // These are invalid on Windows but harmless here, and the existing
        // download sanitizer already rewrites them; keep the two consistent.
        assert_eq!(
            sanitize_file_component("what?really*").unwrap(),
            "what?really*"
        );
    }

    #[test]
    fn archive_filenames_are_repaired_into_one_component() {
        assert_eq!(
            sanitize_archive_filename("a/b\\c:d*e?f<g>h|i").unwrap(),
            "a_b_c_d_e_f_g_h_i"
        );
        assert_eq!(
            sanitize_archive_filename("  spaced.zip  ").unwrap(),
            "spaced.zip"
        );
        assert_eq!(
            sanitize_archive_filename("///").unwrap(),
            "___",
            "doubled separators still leave a usable name"
        );
    }

    #[test]
    fn archive_filenames_cannot_survive_as_traversal() {
        // Separators are replaced, so the only names that could still target
        // the parent or the containing directory are the bare dot forms.
        assert!(sanitize_archive_filename("..").is_err());
        assert!(sanitize_archive_filename(".").is_err());
        assert!(sanitize_archive_filename("").is_err());
        assert!(sanitize_archive_filename("   ").is_err());

        assert_eq!(
            sanitize_archive_filename("../../evil.zip").unwrap(),
            ".._.._evil.zip",
            "a leading traversal is flattened into the name, not rejected"
        );
    }
}
