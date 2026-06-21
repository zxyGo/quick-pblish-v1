//! 命令层测试支撑：构造一个带临时工作目录与派生缓存的 `AppState`，无需 Tauri 运行时。

use std::path::PathBuf;
use std::sync::Mutex;

use crate::state::AppState;

pub struct TestEnv {
    pub ws: PathBuf,
    pub base: PathBuf,
    pub state: AppState,
}

impl TestEnv {
    pub fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "qp-cmd-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let ws = base.join("ws");
        let cfg = base.join("cfg");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&cfg).unwrap();
        let db = crate::index::open(&cfg.join("index.sqlite")).unwrap();
        let state = AppState {
            workspace: Mutex::new(Some(ws.clone())),
            db: Mutex::new(db),
            config_dir: cfg,
            watcher: Mutex::new(None),
        };
        Self { ws, base, state }
    }

    /// 不预设工作目录的环境（用于测试 workspace 激活流程）。
    pub fn empty() -> Self {
        let env = Self::new();
        *env.state.workspace.lock().unwrap() = None;
        env
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}
