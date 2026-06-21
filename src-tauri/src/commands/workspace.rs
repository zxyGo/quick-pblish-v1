use std::path::{Path, PathBuf};

use chrono::Utc;
use tauri::State;

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

/// 激活某工作目录：校验可访问、持久化、重建该目录缓存。
fn activate(state: &State<AppState>, path: PathBuf) -> AppResult<Workspace> {
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "工作目录不存在: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(AppError::Invalid(format!(
            "不是目录: {}",
            path.display()
        )));
    }
    // 写入测试以确认权限
    std::fs::read_dir(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            AppError::Permission(format!("无权访问: {}", path.display()))
        }
        _ => AppError::from(e),
    })?;

    {
        let conn = state.db.lock().expect("db lock");
        index::rebuild(&conn, &path)?;
    }
    {
        let mut ws = state.workspace.lock().expect("workspace lock");
        *ws = Some(path.clone());
    }
    let mut cfg = config::load(&state.config_dir);
    config::set_current(&mut cfg, &path.to_string_lossy().replace('\\', "/"));
    config::save(&state.config_dir, &cfg)?;
    Ok(to_workspace(&path))
}

#[tauri::command]
pub fn select_workspace(state: State<AppState>, path: String) -> AppResult<Workspace> {
    activate(&state, PathBuf::from(path))
}

#[tauri::command]
pub fn switch_workspace(state: State<AppState>, path: String) -> AppResult<Workspace> {
    activate(&state, PathBuf::from(path))
}

#[tauri::command]
pub fn get_current_workspace(state: State<AppState>) -> AppResult<Option<Workspace>> {
    let cfg = config::load(&state.config_dir);
    let Some(current) = cfg.current else {
        return Ok(None);
    };
    let path = PathBuf::from(&current);
    if !path.is_dir() {
        // 目录不可访问：返回 None，前端引导重新选择（FR-003）
        return Ok(None);
    }
    // 自动激活上次目录
    activate(&state, path).map(Some)
}

#[tauri::command]
pub fn list_recent_workspaces(state: State<AppState>) -> AppResult<Vec<Workspace>> {
    let cfg = config::load(&state.config_dir);
    Ok(cfg
        .recent
        .iter()
        .map(|p| to_workspace(Path::new(p)))
        .collect())
}
