use std::path::{Path, PathBuf};

use base64::Engine;
use tauri::State;

use crate::domain::{ImportedAsset, IndexStatus};
use crate::error::{AppError, AppResult};
use crate::index;
use crate::state::AppState;

// ---- core 实现 ----

/// 将外部图片复制进工作目录 `assets/`，返回相对路径供正文引用（FR-014a/Q4）。
pub(crate) fn import_asset_core(state: &AppState, source_path: &str) -> AppResult<ImportedAsset> {
    let root = state.current_root()?;
    let source = PathBuf::from(source_path);
    let file_name = source
        .file_name()
        .ok_or_else(|| AppError::Invalid("无效的源文件".into()))?
        .to_string_lossy()
        .to_string();

    let assets_dir = root.join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    // 去重命名，避免覆盖已有素材
    let mut target = assets_dir.join(&file_name);
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "asset".into());
    let ext = source
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let mut counter = 1;
    while target.exists() {
        target = assets_dir.join(format!("{stem}-{counter}{ext}"));
        counter += 1;
    }

    std::fs::copy(&source, &target)?;
    let final_name = target
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(file_name);

    Ok(ImportedAsset {
        relative_path: format!("assets/{final_name}"),
        file_name: final_name,
    })
}

/// 按扩展名推断图片 MIME（仅覆盖正文常见图片类型，未知回退 octet-stream）。
fn guess_image_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .map(|s| s.to_string_lossy().to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        Some("avif") => "image/avif",
        _ => "application/octet-stream",
    }
}

/// 读取工作目录内的本地图片，返回可直接用于 `<img src>` 的 base64 data URL。
/// 供预览渲染解析正文里的相对图片路径（WebView 无法直接访问本地文件系统路径）。
/// `rel_path` 为相对工作目录根的路径；做归一化并校验不越出工作目录（防目录穿越）。
pub(crate) fn read_asset_data_url_core(state: &AppState, rel_path: &str) -> AppResult<String> {
    let root = state.current_root()?;
    let normalized = rel_path.replace('\\', "/");
    let normalized = normalized.trim_start_matches("./");
    let path = root.join(normalized);

    // 防目录穿越：解析后的真实路径必须仍在工作目录根内。
    let canonical_root = std::fs::canonicalize(&root)?;
    let canonical = std::fs::canonicalize(&path)
        .map_err(|_| AppError::NotFound(format!("本地图片不存在: {rel_path}")))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(AppError::Invalid("图片路径越出工作目录".into()));
    }

    let bytes = std::fs::read(&canonical)?;
    let mime = guess_image_mime(&canonical);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

pub(crate) fn rebuild_index_core(state: &AppState) -> AppResult<IndexStatus> {
    let root = state.current_root()?;
    let conn = state.db.lock().expect("db lock");
    index::rebuild(&conn, &root)?;
    index::status(&conn, &root)
}

pub(crate) fn index_status_core(state: &AppState) -> AppResult<IndexStatus> {
    let root = state.current_root()?;
    let conn = state.db.lock().expect("db lock");
    index::status(&conn, &root)
}

// ---- Tauri command 薄封装 ----

#[tauri::command]
pub fn import_asset(state: State<AppState>, source_path: String) -> AppResult<ImportedAsset> {
    import_asset_core(state.inner(), &source_path)
}

#[tauri::command]
pub fn read_asset_data_url(state: State<AppState>, rel_path: String) -> AppResult<String> {
    read_asset_data_url_core(state.inner(), &rel_path)
}

#[tauri::command]
pub fn rebuild_index(state: State<AppState>) -> AppResult<IndexStatus> {
    rebuild_index_core(state.inner())
}

#[tauri::command]
pub fn get_index_status(state: State<AppState>) -> AppResult<IndexStatus> {
    index_status_core(state.inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::TestEnv;

    #[test]
    fn import_asset_copies_and_dedups() {
        let env = TestEnv::new();
        let src = env.base.join("pic.png");
        std::fs::write(&src, "imgdata").unwrap();

        let a1 = import_asset_core(&env.state, &src.to_string_lossy()).unwrap();
        assert_eq!(a1.relative_path, "assets/pic.png");
        assert!(env.ws.join("assets/pic.png").exists());

        // 再次导入同名 → 去重命名，不覆盖
        let a2 = import_asset_core(&env.state, &src.to_string_lossy()).unwrap();
        assert_eq!(a2.relative_path, "assets/pic-1.png");
    }

    #[test]
    fn rebuild_index_counts_articles() {
        let env = TestEnv::new();
        std::fs::write(env.ws.join("x.md"), "---\ntitle: X\n---\nbody").unwrap();
        let status = rebuild_index_core(&env.state).unwrap();
        assert_eq!(status.total, 1);
        assert!(!status.rebuilding);
    }
}
