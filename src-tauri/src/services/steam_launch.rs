use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use keyvalues_serde::parser::{Obj, Value, Vdf};

use crate::models::{AppError, Result};

/// Ready or Not's Steam app id.
const READY_OR_NOT_APP_ID: &str = "1144200";

/// Launch options Proton needs so the UE4SS `dwmapi.dll` shim loads instead
/// of Wine's own. Keep in sync with the frontend copy string.
pub const UE4SS_LAUNCH_OPTION: &str = r#"WINEDLLOVERRIDES="dwmapi=n,b" %command%"#;

const LAUNCH_OPTIONS_KEY: &str = "LaunchOptions";

/// Outcome of writing the launch option (surfaced to the toast).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchOptionWrite {
    /// Already present exactly as required - file untouched.
    AlreadySet,
    /// The `dwmapi` override was merged in (existing options preserved).
    Updated,
}

/// Sections walked from the parsed root value to the per-app block:
/// `Software -> Valve -> Steam -> apps`. (The file's top-level key,
/// `UserLocalConfigStore`, lives in `Vdf.key`, not in the value tree.)
const CHAIN_KEYS: [&str; 4] = ["Software", "Valve", "Steam", "apps"];

fn apps_obj<'a>(value: &'a Value<'a>) -> Option<&'a Obj<'a>> {
    let mut value = value.get_obj()?;
    for key in CHAIN_KEYS {
        value = value.get(key)?.first()?.get_obj()?;
    }
    Some(value)
}

/// Read, mutate (in one scope so the borrow ends), and escaped-render one
/// `localconfig.vdf`, returning the new file text. `Vdf`'s `Display` escapes
/// (`\"`, `\\`, `\n`...) exactly the way Steam writes these files.
fn with_launch_option_set(text: &str, options: &str) -> Result<String> {
    let parsed =
        keyvalues_serde::parser::parse(text).map_err(|e| AppError::Validation(e.to_string()))?;
    let mut partial = parsed.into_owned();
    {
        let mut value = partial
            .value
            .get_mut_obj()
            .ok_or_else(|| AppError::Validation("localconfig root is not an object".to_string()))?;
        // NOTE: navigate with `&str` lookups and only `insert` when missing.
        // `BTreeMap::entry(Cow::Borrowed(..))` does not match an existing
        // `Cow::Owned` key here and would duplicate the whole chain.
        for key in CHAIN_KEYS {
            if !value.contains_key(key) {
                value.insert(
                    Cow::Owned(key.to_string()),
                    vec![Value::Obj(Obj::default())],
                );
            }
            value = value
                .get_mut(key)
                .expect("entry just inserted")
                .first_mut()
                .expect("chain entry must hold a value")
                .get_mut_obj()
                .expect("config chain entries must be objects");
        }
        let app = value
            .entry(Cow::Owned(READY_OR_NOT_APP_ID.to_string()))
            .or_insert_with(|| vec![Value::Obj(Obj::default())])
            .first_mut()
            .expect("app entry just inserted");
        let app_obj = app.get_mut_obj().expect("app entry must be an object");
        app_obj.insert(
            LAUNCH_OPTIONS_KEY.into(),
            vec![Value::Str(Cow::Owned(options.to_string()))],
        );
    }
    Ok(Vdf::from(partial).to_string())
}

/// Every `userdata/<id>/config/localconfig.vdf` on this machine. Steam keeps
/// one per logged-in account; the launch option must be merged into each so a
/// multi-account machine gets it regardless of who launches the game.
pub fn localconfig_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(steam_path) = steam_roots() else {
        return out;
    };
    for root in steam_path {
        let Ok(entries) = fs::read_dir(root.join("userdata")) else {
            continue;
        };
        for entry in entries.flatten() {
            let config = entry.path().join("config").join("localconfig.vdf");
            if config.is_file() && !out.contains(&config) {
                out.push(config);
            }
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn steam_roots() -> Result<Vec<PathBuf>> {
    let home = std::env::var("HOME")
        .map_err(|_| AppError::NotFound("HOME environment variable not set".to_string()))?;
    let candidates = [
        ".steam/steam",
        ".local/share/Steam",
        ".var/app/com.valvesoftware.Steam/data/Steam",
        ".var/app/com.valvesoftware.Steam/.steam/steam",
        "snap/steam/common/.steam/steam",
    ];
    Ok(candidates
        .into_iter()
        .map(|c| PathBuf::from(&home).join(c))
        // A userdata dir without steamapps is still enough for launch options.
        .filter(|p| p.join("userdata").is_dir())
        .collect())
}

#[cfg(not(target_os = "linux"))]
fn steam_roots() -> Result<Vec<PathBuf>> {
    Ok(Vec::new())
}

/// Read the current `LaunchOptions` value for Ready or Not from one
/// `localconfig.vdf`. Missing file / missing section / missing key all read
/// as `None` (Steam just runs with no launch options).
fn read_launch_options(path: &Path) -> Result<Option<String>> {
    let text = fs::read_to_string(path)?;
    let parsed =
        keyvalues_serde::parser::parse(&text).map_err(|e| AppError::Validation(e.to_string()))?;
    let vdf = Vdf::from(parsed);
    Ok(apps_obj(&vdf.value)
        .and_then(|apps| apps.get(READY_OR_NOT_APP_ID))
        .and_then(|values| values.first())
        .and_then(|app| app.get_obj())
        .and_then(|app| app.get(LAUNCH_OPTIONS_KEY))
        .and_then(|values| values.first())
        .and_then(|value| value.get_str())
        .map(|s| s.to_string()))
}

/// Merge the UE4SS override into `current`:
/// - already containing `dwmapi` (any casing/format) -> untouched (`None`)
/// - empty/missing -> exactly `UE4SS_LAUNCH_OPTION`
/// - anything else -> `<existing> <UE4SS_LAUNCH_OPTION>` (existing flags kept)
fn merge_launch_options(current: Option<&str>) -> Option<String> {
    match current.map(str::trim) {
        Some(existing) if existing.to_lowercase().contains("dwmapi") => None,
        Some("") | None => Some(UE4SS_LAUNCH_OPTION.to_string()),
        Some(existing) => Some(format!("{existing} {UE4SS_LAUNCH_OPTION}")),
    }
}

/// Ensure every local `localconfig.vdf` launches Ready or Not with the UE4SS
/// `dwmapi` override. Idempotent: files already containing a `dwmapi`
/// override are left untouched, and unrelated launch flags are preserved.
/// Steam must be closed (or it rewrites the file on exit and discards this).
///
/// Returns `(files_updated, files_already_set)`.
pub fn ensure_ue4ss_launch_option() -> Result<(usize, usize)> {
    let paths = localconfig_paths();
    if paths.is_empty() {
        return Err(AppError::NotFound(
            "No Steam userdata localconfig.vdf found".to_string(),
        ));
    }

    let mut updated = 0usize;
    let mut already_set = 0usize;
    for path in &paths {
        let current = read_launch_options(path)?;
        let Some(merged) = merge_launch_options(current.as_deref()) else {
            already_set += 1;
            continue;
        };

        let text = fs::read_to_string(path)?;
        let rendered = with_launch_option_set(&text, &merged)?;

        // Back the original up once (`.bak`, kept next to it) before writing.
        let backup = path.with_extension("vdf.ronmm.bak");
        if !backup.exists() {
            fs::copy(path, &backup)?;
        }
        fs::write(path, rendered)?;
        updated += 1;
    }

    if updated == 0 && already_set == 0 {
        return Err(AppError::Validation(
            "Could not find the Steam apps section in localconfig.vdf".to_string(),
        ));
    }

    Ok((updated, already_set))
}

/// True when any local `localconfig.vdf` already launches the game with a
/// `dwmapi` override (regardless of which account it belongs to).
pub fn ue4ss_launch_option_set() -> bool {
    localconfig_paths()
        .iter()
        .filter_map(|path| read_launch_options(path).ok())
        .flatten()
        .any(|options| options.to_lowercase().contains("dwmapi"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proof against a real-world 300KB `localconfig.vdf`: parse, mutate the
    /// RoN block, re-render, re-parse, and assert the value trees are identical
    /// except RoN's `LaunchOptions`. Set `RONMM_REAL_LOCALCONFIG` to a *copy*
    /// of your file to run it; never the live file.
    #[test]
    fn real_file_roundtrip_proof() {
        let Ok(path) = std::env::var("RONMM_REAL_LOCALCONFIG") else {
            eprintln!("skipped: RONMM_REAL_LOCALCONFIG not set");
            return;
        };
        let text = std::fs::read_to_string(&path).unwrap();
        eprintln!("original bytes: {}", text.len());
        let rendered = with_launch_option_set(&text, UE4SS_LAUNCH_OPTION).unwrap();
        eprintln!("rendered bytes: {}", rendered.len());

        let before = Vdf::from(
            keyvalues_serde::parser::parse(&text)
                .expect("original must parse")
                .into_owned(),
        );
        let after = Vdf::from(
            keyvalues_serde::parser::parse(&rendered)
                .expect("rendered must parse")
                .into_owned(),
        );
        // Same top-level key, same apps except RoN's LaunchOptions.
        assert_eq!(before.key, after.key);
        let before_apps = apps_obj(&before.value).expect("before apps");
        let after_apps = apps_obj(&after.value).expect("after apps");
        assert_eq!(
            before_apps.keys().collect::<Vec<_>>(),
            after_apps.keys().collect::<Vec<_>>(),
            "no app entries added or lost"
        );
        for (key, values) in before_apps.iter() {
            if key.as_ref() == READY_OR_NOT_APP_ID {
                continue;
            }
            let after_values = after_apps.get(key);
            assert_eq!(
                after_values.map(|v| format!("{v:?}")),
                Some(format!("{values:?}")),
                "app {key} must be untouched"
            );
        }
        // And RoN itself only differs by LaunchOptions.
        let ron_after = after_apps
            .get(READY_OR_NOT_APP_ID)
            .and_then(|v| v.first())
            .and_then(|v| v.get_obj())
            .and_then(|o| o.get(LAUNCH_OPTIONS_KEY))
            .and_then(|v| v.first())
            .and_then(|v| v.get_str())
            .map(|s| s.to_string());
        assert_eq!(ron_after.as_deref(), Some(UE4SS_LAUNCH_OPTION));
    }

    fn sample_localconfig(launch_options: Option<&str>) -> String {
        let app_block = match launch_options {
            Some(options) => format!(
                "\t\t\t\t\t\"1144200\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"1789008297\"\n\t\t\t\t\t\t\"LaunchOptions\"\t\t\"{options}\"\n\t\t\t\t\t}}"
            ),
            None => "\t\t\t\t\t\"1144200\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"1789008297\"\n\t\t\t\t\t}"
                .to_string(),
        };
        format!(
            "\"UserLocalConfigStore\"\n{{\n\t\"Software\"\n\t{{\n\t\t\"Valve\"\n\t\t{{\n\t\t\t\"Steam\"\n\t\t{{\n\t\t\t\t\"apps\"\n\t\t\t\t{{\n{app_block}\n\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}"
        )
    }

    fn launch_options_of(rendered: &str) -> Option<String> {
        let parsed = keyvalues_serde::parser::parse(rendered).expect("rendered must parse");
        let vdf = Vdf::from(parsed);
        apps_obj(&vdf.value)
            .and_then(|apps| apps.get(READY_OR_NOT_APP_ID))
            .and_then(|values| values.first())
            .and_then(|app| app.get_obj())
            .and_then(|app| app.get(LAUNCH_OPTIONS_KEY))
            .and_then(|values| values.first())
            .and_then(|value| value.get_str())
            .map(|s| s.to_string())
    }

    #[test]
    fn sets_launch_option_on_empty_app_block() {
        let rendered =
            with_launch_option_set(&sample_localconfig(None), UE4SS_LAUNCH_OPTION).unwrap();
        assert_eq!(
            launch_options_of(&rendered).as_deref(),
            Some(UE4SS_LAUNCH_OPTION)
        );
    }

    #[test]
    fn parses_escaped_quotes_and_merges_existing_flags() {
        // As Steam writes it: backslash-escaped quotes inside the quoted value.
        let existing = "gamemoderun WINEDLLOVERRIDES=\\\"dwmapi=n,b\\\" %command%";
        let rendered = with_launch_option_set(
            &sample_localconfig(Some(existing)),
            "gamemoderun WINEDLLOVERRIDES=\\\"dwmapi=n,b\\\" %command%",
        )
        .unwrap();
        assert!(launch_options_of(&rendered)
            .unwrap()
            .contains("gamemoderun"));
    }

    #[test]
    fn merge_preserves_existing_flags() {
        assert_eq!(
            merge_launch_options(None),
            Some(UE4SS_LAUNCH_OPTION.to_string())
        );
        assert_eq!(
            merge_launch_options(Some("")),
            Some(UE4SS_LAUNCH_OPTION.to_string())
        );
        assert_eq!(
            merge_launch_options(Some("gamemoderun %command%")),
            Some(format!("gamemoderun %command% {UE4SS_LAUNCH_OPTION}"))
        );
    }

    #[test]
    fn merge_leaves_existing_dwmapi_override_alone() {
        assert_eq!(merge_launch_options(Some(UE4SS_LAUNCH_OPTION)), None);
        assert_eq!(
            merge_launch_options(Some(r#"WINEDLLOVERRIDES=dwmapi %command%"#)),
            None
        );
        assert_eq!(
            merge_launch_options(Some(
                "gamemoderun WINEDLLOVERRIDES=\"dwmapi=n,b\" %command%"
            )),
            None
        );
    }

    #[test]
    fn fixture_end_to_end_sets_ron_options_and_preserves_neighbours() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/localconfig_sample.vdf"
        ))
        .unwrap();
        let rendered = with_launch_option_set(&text, UE4SS_LAUNCH_OPTION).unwrap();
        let parsed = keyvalues_serde::parser::parse(&rendered).expect("rendered must parse");
        let vdf = Vdf::from(parsed);
        let apps = apps_obj(&vdf.value).expect("apps section must survive");

        // RoN got exactly the UE4SS option (was empty).
        let ron = apps
            .get(READY_OR_NOT_APP_ID)
            .and_then(|v| v.first())
            .and_then(|v| v.get_obj())
            .and_then(|o| o.get(LAUNCH_OPTIONS_KEY))
            .and_then(|v| v.first())
            .and_then(|v| v.get_str())
            .map(|s| s.to_string());
        assert_eq!(ron.as_deref(), Some(UE4SS_LAUNCH_OPTION));

        // Neighbouring apps untouched.
        let other = apps
            .get("427410")
            .and_then(|v| v.first())
            .and_then(|v| v.get_obj())
            .and_then(|o| o.get(LAUNCH_OPTIONS_KEY))
            .and_then(|v| v.first())
            .and_then(|v| v.get_str())
            .map(|s| s.to_string());
        assert_eq!(other.as_deref(), Some("gamemoderun %command%"));
        assert!(apps.get("1151640").is_some());
    }

    #[test]
    fn fixture_end_to_end_merges_when_existing_flags_present() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/localconfig_sample.vdf"
        ))
        .unwrap();
        let with_flags = text.replace(
            "\"LaunchOptions\"\t\t\"\"",
            "\"LaunchOptions\"\t\t\"gamemoderun %command%\"",
        );
        let rendered = with_launch_option_set(
            &with_flags,
            &format!("gamemoderun %command% {UE4SS_LAUNCH_OPTION}"),
        )
        .unwrap();
        let ron = launch_options_of(&rendered).expect("RoN options must exist");
        assert!(ron.starts_with("gamemoderun %command%"));
        assert!(ron.contains("dwmapi"));
    }
}
