pub mod commands;
pub mod models;
pub mod services;
pub mod state;

use crate::commands::{collections, config, game, modpack};
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
            collections::toggle_collection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
