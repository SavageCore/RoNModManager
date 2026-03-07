pub mod commands;
pub mod models;
pub mod services;
pub mod state;

use crate::commands::{auth, collections, config, game, modpack, mods, sharing};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            config::get_config,
            config::set_theme,
            game::detect_game_path,
            game::set_game_path,
            modpack::set_modpack_url,
            modpack::sync_modpack,
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
            sharing::share_modpack_via_code,
            sharing::import_from_code,
            sharing::push_modpack_update,
            sharing::import_modpack_from_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
