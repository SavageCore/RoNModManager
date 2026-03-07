pub mod commands;
pub mod models;
pub mod services;
pub mod state;

use crate::commands::{auth, collections, config, game, modpack, mods, profiles, sharing};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::set_theme,
            config::apply_intro_skip,
            config::is_intro_skip_applied,
            game::detect_game_path,
            game::set_game_path,
            game::launch_game,
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
            mods::install_mods,
            mods::uninstall_mods,
            profiles::list_profiles,
            profiles::get_profile,
            profiles::save_profile,
            profiles::delete_profile,
            profiles::apply_profile,
            sharing::share_modpack_via_code,
            sharing::import_from_code,
            sharing::push_modpack_update,
            sharing::import_modpack_from_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
