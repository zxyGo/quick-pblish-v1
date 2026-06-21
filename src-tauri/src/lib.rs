mod commands;
mod domain;
mod error;
mod index;
mod state;
mod storage;

use std::sync::Mutex;

use tauri::Manager;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("无法解析应用数据目录");
            let config_dir = app
                .path()
                .app_config_dir()
                .expect("无法解析应用配置目录");
            std::fs::create_dir_all(&data_dir).ok();
            std::fs::create_dir_all(&config_dir).ok();

            let db_path = index::default_db_path(&data_dir);
            let conn = index::open(&db_path).expect("无法打开派生缓存数据库");

            app.manage(AppState {
                workspace: Mutex::new(None),
                db: Mutex::new(conn),
                config_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace::select_workspace,
            commands::workspace::switch_workspace,
            commands::workspace::get_current_workspace,
            commands::workspace::list_recent_workspaces,
            commands::article::list_articles,
            commands::article::read_article,
            commands::article::create_article,
            commands::article::save_article,
            commands::article::delete_article,
            commands::article::update_metadata,
            commands::file_tree::get_file_tree,
            commands::file_tree::create_folder,
            commands::file_tree::rename_path,
            commands::file_tree::move_path,
            commands::file_tree::delete_path,
            commands::asset::import_asset,
            commands::asset::rebuild_index,
            commands::asset::get_index_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
