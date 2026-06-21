use std::path::Path;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, AppResult};

/// 监听工作目录的外部文件变化，向前端广播 `workspace_changed` 事件
/// （contracts/file-tree.md；支撑 FR-019 的一致性与列表/文件树实时刷新）。
/// 返回的 watcher 句柄需被持有以保持监听存活。
pub fn watch(app: AppHandle, root: &Path) -> AppResult<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let paths: Vec<String> = event
                .paths
                .iter()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect();
            let _ = app.emit("workspace_changed", paths);
        }
    })
    .map_err(|e| AppError::Io(format!("watcher: {e}")))?;

    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|e| AppError::Io(format!("watch: {e}")))?;
    Ok(watcher)
}
