//! PG Model Viewer — a standalone Tauri app that renders real Project: Gorgon
//! item models with the game's live dye system and a paper-doll character mode.
//!
//! Extracted from glogger. The backend is a thin slice: a minimal CDN item
//! catalog (`cdn` + `cdn_commands`), the item→appearance parser
//! (`game_data::appearance`), and the model asset pipeline (`model_assets`).

mod cdn;
mod cdn_commands;
mod game_data;
mod model_assets;

use tokio::sync::RwLock;

use cdn_commands::{ensure_game_data, get_icon_path, list_dyes, GameData};
use model_assets::{
    get_model_catalog, list_appearance_items, model_cache_root, model_viewer_status,
    resolve_base_body, resolve_item_appearance, start_model_extraction,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .manage(RwLock::new(GameData::default()))
        .invoke_handler(tauri::generate_handler![
            ensure_game_data,
            get_icon_path,
            list_dyes,
            model_viewer_status,
            model_cache_root,
            get_model_catalog,
            resolve_item_appearance,
            resolve_base_body,
            list_appearance_items,
            start_model_extraction,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the PG Model Viewer");
}
