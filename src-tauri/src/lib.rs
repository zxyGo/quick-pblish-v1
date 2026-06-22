mod adapters;
mod commands;
mod domain;
mod error;
mod index;
mod publish;
mod state;
mod storage;
mod watcher;

use std::sync::Mutex;

use tauri::Manager;

use commands::publish::PublishState;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("无法解析应用数据目录");
            let config_dir = app.path().app_config_dir().expect("无法解析应用配置目录");
            std::fs::create_dir_all(&data_dir).ok();
            std::fs::create_dir_all(&config_dir).ok();

            let db_path = index::default_db_path(&data_dir);
            let conn = index::open(&db_path).expect("无法打开派生缓存数据库");
            // 同步历史表（派生缓存，FR-018）
            publish::history::init(&conn).expect("无法初始化同步历史表");

            app.manage(AppState {
                workspace: Mutex::new(None),
                db: Mutex::new(conn),
                config_dir,
                watcher: Mutex::new(None),
            });
            // 发布功能状态：会话密文存于 app_data/sessions（FR-005）
            app.manage(PublishState::new(data_dir.join("sessions")));
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
            // 002-multi-platform-publish
            commands::publish::list_platforms,
            commands::publish::connect_platform,
            commands::publish::confirm_connection,
            commands::publish::get_platform_status,
            commands::publish::disconnect_platform,
            commands::publish::report_eval_result,
            commands::publish::sync_article,
            commands::publish::retry_sync,
            commands::publish::get_sync_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
