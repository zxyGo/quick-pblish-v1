use std::path::{Path, PathBuf};

use chrono::Utc;
use tauri::{AppHandle, State};

use crate::domain::Workspace;
use crate::error::{AppError, AppResult};
use crate::index;
use crate::state::AppState;
use crate::storage::config;

fn to_workspace(path: &Path) -> Workspace {
    Workspace {
        path: path.to_string_lossy().replace('\\', "/"),
        name: path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        last_opened: Utc::now().to_rfc3339(),
    }
}

/// 激活某工作目录的核心逻辑（不含文件监听，便于测试）：
/// 校验可访问、重建缓存、设为当前、持久化配置。
pub(crate) fn activate_core(state: &AppState, path: &Path) -> AppResult<Workspace> {
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "工作目录不存在: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(AppError::Invalid(format!("不是目录: {}", path.display())));
    }
    std::fs::read_dir(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            AppError::Permission(format!("无权访问: {}", path.display()))
        }
        _ => AppError::from(e),
    })?;

    {
        let conn = state.db.lock().expect("db lock");
        index::rebuild(&conn, path)?;
    }
    {
        let mut ws = state.workspace.lock().expect("workspace lock");
        *ws = Some(path.to_path_buf());
    }
    let mut cfg = config::load(&state.config_dir);
    config::set_current(&mut cfg, &path.to_string_lossy().replace('\\', "/"));
    config::save(&state.config_dir, &cfg)?;
    Ok(to_workspace(path))
}

pub(crate) fn get_current_core(state: &AppState) -> AppResult<Option<Workspace>> {
    let cfg = config::load(&state.config_dir);
    let Some(current) = cfg.current else {
        return Ok(None);
    };
    let path = PathBuf::from(&current);
    if !path.is_dir() {
        // 目录不可访问：返回 None，前端引导重新选择（FR-003）
        return Ok(None);
    }
    activate_core(state, &path).map(Some)
}

pub(crate) fn list_recent_core(state: &AppState) -> Vec<Workspace> {
    let cfg = config::load(&state.config_dir);
    cfg.recent
        .iter()
        .map(|p| to_workspace(Path::new(p)))
        .collect()
}

/// 替换文件监听器以监听新目录（旧 watcher 被 drop 自动停止）。
fn restart_watcher(app: &AppHandle, state: &AppState, path: &Path) {
    match crate::watcher::watch(app.clone(), path) {
        Ok(w) => *state.watcher.lock().expect("watcher lock") = Some(w),
        Err(_) => *state.watcher.lock().expect("watcher lock") = None,
    }
}

// ---- Tauri command 薄封装 ----

#[tauri::command]
pub fn select_workspace(
    app: AppHandle,
    state: State<AppState>,
    path: String,
) -> AppResult<Workspace> {
    let p = PathBuf::from(path);
    let ws = activate_core(state.inner(), &p)?;
    restart_watcher(&app, state.inner(), &p);
    Ok(ws)
}

#[tauri::command]
pub fn switch_workspace(
    app: AppHandle,
    state: State<AppState>,
    path: String,
) -> AppResult<Workspace> {
    let p = PathBuf::from(path);
    let ws = activate_core(state.inner(), &p)?;
    restart_watcher(&app, state.inner(), &p);
    Ok(ws)
}

#[tauri::command]
pub fn get_current_workspace(
    app: AppHandle,
    state: State<AppState>,
) -> AppResult<Option<Workspace>> {
    let result = get_current_core(state.inner())?;
    if let Some(ws) = &result {
        restart_watcher(&app, state.inner(), Path::new(&ws.path));
    }
    Ok(result)
}

#[tauri::command]
pub fn list_recent_workspaces(state: State<AppState>) -> AppResult<Vec<Workspace>> {
    Ok(list_recent_core(state.inner()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestEnv;

    #[test]
    fn select_persists_and_get_current_reloads() {
        let env = TestEnv::empty();
        let ws = activate_core(&env.state, &env.ws).unwrap();
        assert_eq!(ws.path, env.ws.to_string_lossy().replace('\\', "/"));

        // current 已持久化，可重新读出
        let current = get_current_core(&env.state).unwrap();
        assert!(current.is_some());

        // recent 包含该目录
        let recent = list_recent_core(&env.state);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn activate_missing_dir_returns_not_found() {
        let env = TestEnv::empty();
        let missing = env.base.join("does-not-exist");
        assert!(matches!(
            activate_core(&env.state, &missing),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn get_current_none_when_dir_unavailable() {
        let env = TestEnv::empty();
        activate_core(&env.state, &env.ws).unwrap();
        // 删除工作目录，模拟外部移除
        std::fs::remove_dir_all(&env.ws).unwrap();
        let current = get_current_core(&env.state).unwrap();
        assert!(current.is_none());
    }
}
