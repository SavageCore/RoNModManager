use tauri::Manager;
pub mod commands;
pub mod models;
pub mod services;
pub mod state;

use crate::commands::{
    auth, collections, config, game, modpack, mods, profiles, sharing, sync, tags, updater, window,
};
use state::AppState;

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

    #[cfg(debug_assertions)]
    {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(false)
            .try_init();
    }
    let builder = tauri::Builder::default().setup(|app| {
        #[cfg(debug_assertions)]
        {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
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
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::update_config,
            config::verify_nexus_api_key,
            config::set_theme,
            config::apply_intro_skip,
            config::undo_intro_skip,
            config::is_intro_skip_applied,
            game::detect_game_path,
            game::set_game_path,
            game::launch_game,
            game::sync_mod_links,
            game::launch_game_with_groups,
            modpack::set_modpack_url,
            modpack::sync_modpack,
            modpack::get_modpack_collections,
            modpack::build_modpack_from_installed,
            modpack::export_modpack_to_file,
            sync::sync_modpack_to_remote,
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
            mods::fetch_nexus_mod_info,
            mods::refresh_mod_metadata,
            mods::uninstall_mods,
            mods::uninstall_mod,
            mods::uninstall_archive,
            mods::update_mod_display_name,
            mods::update_mod_source_url,
            mods::update_nexus_file_id,
            mods::read_manifest_for_archive,
            mods::get_modio_remote_info,
            mods::get_addon_map,
            mods::set_addon_map,
            profiles::list_profiles,
            profiles::get_profile,
            profiles::save_profile,
            profiles::delete_profile,
            profiles::apply_profile,
            updater::check_for_update,
            updater::install_update,
            sharing::share_modpack_via_code,
            sharing::import_from_code,
            sharing::push_modpack_update,
            sharing::import_modpack_from_file,
            window::set_window_title,
            window::save_window_state,
            window::get_window_state,
            commands::fetch::fetch_modpack_json,
            commands::fetch_archive::download_mod_archive,
            commands::fs::file_exists,
            commands::fs::get_archive_root_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
