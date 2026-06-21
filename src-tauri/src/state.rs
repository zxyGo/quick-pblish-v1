use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// 全局应用状态：当前工作目录、派生缓存连接、配置目录。
pub struct AppState {
    pub workspace: Mutex<Option<PathBuf>>,
    pub db: Mutex<Connection>,
    pub config_dir: PathBuf,
}

impl AppState {
    pub fn current_root(&self) -> AppResult<PathBuf> {
        self.workspace
            .lock()
            .expect("workspace lock poisoned")
            .clone()
            .ok_or_else(|| AppError::Invalid("尚未选择工作目录".into()))
    }
}
