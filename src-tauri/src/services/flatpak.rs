/// True inside a Flatpak sandbox, where host integration works differently:
/// deep links arrive via the exported `.desktop` file, host processes are
/// invisible without `flatpak-spawn`, and sandbox PATH lacks helpers like
/// `xdg-mime` / `update-desktop-database`.
///
/// Checks `/.flatpak-info` (always present in a sandbox) plus `FLATPAK_ID`
/// (set by Flatpak when available). Cross-platform: always false on
/// non-sandboxed hosts, including Windows.
pub fn is_flatpak_sandbox() -> bool {
    std::path::Path::new("/.flatpak-info").exists() || std::env::var("FLATPAK_ID").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_flatpak_id_env() {
        let key = "FLATPAK_ID";
        let prior = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, "uk.savagecore.ronmodmanager");
        }
        assert!(is_flatpak_sandbox());
        unsafe {
            if let Some(v) = prior {
                std::env::set_var(key, v);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}
