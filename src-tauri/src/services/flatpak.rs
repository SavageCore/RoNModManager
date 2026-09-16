/// True inside a Flatpak sandbox, where host integration works differently:
/// deep links arrive via the exported `.desktop` file, host processes are
/// invisible without `flatpak-spawn`, and sandbox PATH lacks helpers like
/// `xdg-mime` / `update-desktop-database`.
///
/// Checks `/.flatpak-info` (always present in a sandbox) plus `FLATPAK_ID`
/// (set by Flatpak when available). Cross-platform: always false on
/// non-sandboxed hosts, including Windows.
pub fn is_flatpak_sandbox() -> bool {
    sandbox_from(std::env::var("FLATPAK_ID").ok().as_deref())
}

/// The detection rule itself, split out so it can be tested without mutating
/// the process-wide `FLATPAK_ID`, which other concurrently running tests read.
fn sandbox_from(flatpak_id: Option<&str>) -> bool {
    std::path::Path::new("/.flatpak-info").exists() || flatpak_id.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_flatpak_id_env() {
        assert!(sandbox_from(Some("uk.savagecore.ronmodmanager")));
    }

    #[test]
    fn no_flatpak_id_is_not_a_sandbox() {
        // Without the marker file this is false; a real sandbox always has it.
        assert_eq!(
            sandbox_from(None),
            std::path::Path::new("/.flatpak-info").exists()
        );
    }
}
