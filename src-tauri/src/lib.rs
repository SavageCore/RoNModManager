#[cfg(not(debug_assertions))]
use tauri::Emitter;
use tauri::Manager;
pub mod commands;
pub mod models;
pub mod services;
pub mod state;

use crate::commands::{
    auth, collections, config, game, modpack, mods, profiles, sharing, sync, tags, updater, window,
};
use crate::services::game_watch;
use state::{default_config_path, load_config_from_path, AppState};

/// Resolve the initial backend log level: `RUST_LOG` (if set and parseable)
/// overrides the persisted setting. Falls back to the configured level, then
/// to `Info` as a last resort so logging always works even on a fresh or
/// corrupt config file.
fn initial_log_level() -> log::LevelFilter {
    if let Ok(env) = std::env::var("RUST_LOG") {
        if let Ok(level) = env.parse::<log::LevelFilter>() {
            return level;
        }
    }
    default_config_path()
        .ok()
        .and_then(|path| load_config_from_path(&path).ok())
        .map(|config| log::LevelFilter::from(config.log_level))
        .unwrap_or(log::LevelFilter::Info)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    {
        // WebKitGTK on Wayland can fail on some drivers with dmabuf/GBM.
        // Keep Wayland enabled while forcing a safer renderer path.
        if std::env::var("WAYLAND_DISPLAY").is_ok()
            && std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err()
        {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    // Init the backend logger in both debug and release so the Flatpak
    // build (where env_logger is otherwise disabled) emits diagnostics.
    let level = initial_log_level();
    let _ = env_logger::builder()
        .filter_level(level)
        .is_test(false)
        .try_init();
    let builder = tauri::Builder::default().setup(|app| {
        // When link-on-launch-only is enabled, the game folder must be stock at
        // rest. A previous session (or a pre-upgrade install) may have left it
        // modded, so restore to stock on every startup. Best-effort: never block
        // or fail startup over this.
        {
            let state = app.state::<AppState>();
            let watched_path = state.get_config().ok().and_then(|c| {
                if c.link_on_launch_only {
                    c.game_path.clone()
                } else {
                    None
                }
            });
            if let Some(game_path) = watched_path {
                if game_watch::is_game_running() {
                    // Don't yank links from under a running game; the watcher
                    // restores stock once it exits.
                    game_watch::spawn_game_exit_watcher(app.handle().clone(), game_path);
                } else {
                    game::cleanup_to_stock_on_exit(&state);
                }
            }
        }

        #[cfg(debug_assertions)]
        {
            if std::env::var("SCREENSHOT_MODE").is_err()
                && std::env::var("WIZARD_SCREENSHOT").is_err()
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
        }

        #[cfg(not(debug_assertions))]
        {
            use tauri_plugin_deep_link::DeepLinkExt;
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            if crate::services::flatpak::is_flatpak_sandbox() {
                log::info!(
                    "Flatpak sandbox detected; skipping runtime deep-link registration (handled by exported .desktop file)"
                );
            } else if let Err(e) = app.deep_link().register_all() {
                log::warn!(
                    "Failed to register deep-link handlers (need xdg-mime/update-desktop-database): {e}"
                );
            }
        }

        #[cfg(not(debug_assertions))]
        {
            let urls: Vec<String> = std::env::args()
                .skip(1)
                .filter(|a| a.starts_with("ronmm://"))
                .collect();
            if !urls.is_empty() {
                log::info!("Startup deep-link URLs: {:?}", urls);
            }
            app.manage(window::StartupUrls(std::sync::Mutex::new(Some(urls))));
        }

        #[cfg(debug_assertions)]
        {
            app.manage(window::StartupUrls(std::sync::Mutex::new(None)));
        }

        use tauri::menu::{Menu, MenuItem};
        use tauri::tray::{TrayIconBuilder, TrayIconEvent};

        let show = MenuItem::with_id(app, "show", "Show RoN Mod Manager", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&show, &quit])?;

        TrayIconBuilder::new()
            .icon(app.default_window_icon().unwrap().clone())
            .menu(&menu)
            .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                }
            })
            .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event: TrayIconEvent| {
                if let TrayIconEvent::Click { .. } = event {
                    let app = tray.app_handle();
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            })
            .build(app)?;

        Ok(())
    });
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(
        |app: &tauri::AppHandle, argv: Vec<String>, _cwd: String| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
            let urls: Vec<String> = argv
                .into_iter()
                .filter(|a| a.starts_with("ronmm://"))
                .collect();
            if !urls.is_empty() {
                let _ = app.emit("deep-link://new-url", urls);
            }
        },
    ));
    let app = builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::update_config,
            config::verify_nexus_api_key,
            config::verify_modio_api_key,
            config::set_theme,
            config::apply_intro_skip,
            config::undo_intro_skip,
            config::is_intro_skip_applied,
            config::get_gpu_profiles,
            config::detect_gpu_profile,
            config::apply_optimization,
            config::get_applied_optimization_profile,
            config::disable_optimization,
            game::detect_game_path,
            game::set_game_path,
            game::sync_mod_links,
            game::launch_game_with_groups,
            game::ensure_game_stock,
            game::launch_vanilla_game,
            game::is_game_running,
            game::suppress_exit_cleanup,
            modpack::set_modpack_url,
            modpack::sync_modpack,
            modpack::get_modpack_collections,
            modpack::build_modpack_from_installed,
            modpack::export_modpack_to_file,
            modpack::apply_modpack_profile_metadata,
            sync::sync_modpack_to_remote,
            sync::test_sync_auth,
            collections::get_collections,
            collections::get_collection_mods,
            collections::create_collection,
            collections::add_mod_to_collection,
            collections::remove_mod_from_collection,
            collections::delete_collection,
            collections::toggle_collection,
            collections::rename_collection,
            collections::get_collection_colors,
            collections::set_collection_color,
            tags::get_tags,
            tags::set_mod_tags,
            tags::delete_tag,
            tags::get_broken_mods,
            tags::set_mod_broken,
            tags::clear_mod_broken,
            tags::get_no_world_gen_mods,
            tags::set_mod_no_world_gen,
            tags::clear_mod_no_world_gen,
            auth::get_auth_status,
            auth::open_modio_login,
            auth::save_token,
            auth::validate_token,
            auth::logout,
            mods::get_mod_list,
            mods::get_installed_mod_groups,
            mods::install_mods,
            mods::install_local_mod,
            mods::get_archive_pak_files,
            mods::add_modio_mod,
            mods::add_nexus_mod,
            mods::list_nexus_file_options,
            mods::cancel_nexus_download,
            mods::check_nexus_premium,
            mods::fetch_nexus_mod_info,
            mods::refresh_mod_metadata,
            mods::check_mod_updates,
            mods::uninstall_mods,
            mods::uninstall_mod,
            mods::uninstall_archive,
            mods::update_mod_display_name,
            mods::update_mod_source_url,
            mods::update_nexus_file_id,
            mods::replace_mod_archive,
            mods::read_manifest_for_archive,
            mods::get_modio_remote_info,
            mods::get_addon_map,
            mods::set_addon_map,
            profiles::list_profiles,
            profiles::get_profile,
            profiles::save_profile,
            profiles::delete_profile,
            profiles::rename_profile,
            profiles::duplicate_profile,
            profiles::apply_profile,
            profiles::get_modpack_meta,
            profiles::set_modpack_meta,
            profiles::get_sync_details,
            profiles::set_sync_details,
            updater::check_for_update,
            updater::install_update,
            updater::is_flatpak,
            sharing::share_modpack_via_code,
            sharing::import_from_code,
            sharing::push_modpack_update,
            sharing::import_modpack_from_file,
            window::set_window_title,
            window::save_window_state,
            window::get_window_state,
            window::manage_window_geometry,
            window::is_screenshot_mode,
            window::is_wizard_screenshot_mode,
            window::screenshot_theme,
            window::get_startup_urls,
            commands::fetch::fetch_modpack_json,
            commands::fetch_archive::download_mod_archive,
            commands::fs::file_exists,
            commands::fs::get_archive_root_path,
            commands::fs::reveal_in_file_manager,
            commands::fs::open_dir,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle: &tauri::AppHandle, event: tauri::RunEvent| {
        if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = &event {
            // Best-effort stock cleanup on real quit. Skipped on the
            // launch-close path (see game::suppress_exit_cleanup) and when the
            // feature is disabled. Never blocks shutdown.
            let state = app_handle.state::<AppState>();
            game::cleanup_to_stock_on_exit(&state);
        }
    });
}
