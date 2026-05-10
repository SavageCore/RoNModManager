use tauri::Manager;
pub mod commands;
pub mod models;
pub mod services;
pub mod state;

use crate::commands::{
    auth, collections, config, game, modpack, mods, profiles, sharing, updater, window,
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

    let mut builder = tauri::Builder::default();
    #[cfg(debug_assertions)]
    {
        builder = builder.setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
            Ok(())
        });
    }
    builder
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
            config::get_intro_skip_ini_path,
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
            collections::get_collections,
            collections::toggle_collection,
            auth::get_auth_status,
            auth::open_modio_login,
            auth::save_token,
            auth::validate_token,
            auth::logout,
            mods::get_mod_list,
            mods::get_installed_mod_groups,
            mods::install_mods,
            mods::install_local_mod,
            mods::add_modio_mod,
            mods::fetch_nexus_mod_info,
            mods::uninstall_mods,
            mods::uninstall_mod,
            mods::uninstall_archive,
            mods::update_mod_display_name,
            mods::update_mod_source_url,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
