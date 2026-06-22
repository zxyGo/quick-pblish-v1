//! 002-multi-platform-publish 契约入口（contracts/platform.md、contracts/publish.md）。
//!
//! 说明：登录态提取与 WebView 结果回传（probe/eval）的 IPC 接线为联调项（见 webview.rs）。
//! 本层已完成：会话加密存储、连接状态、同步编排调度、进度事件、历史写入的端到端骨架。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Utc;
use tauri::{AppHandle, Emitter, State};

use crate::adapters::{adapter_for, PlatformId, PublishAdapter};
use crate::error::{AppError, AppResult};
use crate::publish::session::{OsKeyProvider, SessionStore};
use crate::publish::sync::{run_batch, ImageLoader};
use crate::publish::webview::{PlatformBridge, TauriBridge};
use crate::publish::{history, PlatformConnection, PlatformStatus, SyncJob, SyncRecord, SyncRequest};
use crate::state::AppState;

/// 发布功能状态：会话存储 + 连接缓存（账号标识等运行时信息）。
pub struct PublishState {
    pub session: SessionStore<OsKeyProvider>,
    pub accounts: Mutex<HashMap<PlatformId, Option<String>>>,
}

impl PublishState {
    pub fn new(session_dir: PathBuf) -> Self {
        PublishState {
            session: SessionStore::new(session_dir, OsKeyProvider),
            accounts: Mutex::new(HashMap::new()),
        }
    }

    fn connection(&self, platform: PlatformId) -> PlatformConnection {
        let status = if self.session.exists(platform) {
            PlatformStatus::Connected
        } else {
            PlatformStatus::Disconnected
        };
        let account = self
            .accounts
            .lock()
            .expect("accounts lock")
            .get(&platform)
            .cloned()
            .flatten();
        PlatformConnection {
            platform,
            status,
            account_label: account,
            last_checked_at: Some(Utc::now().to_rfc3339()),
        }
    }
}

/// 文件系统图片加载：把 HTML 中本地 `assets/` 引用解析为字节（FR-010）。
struct FsImageLoader {
    workspace_root: PathBuf,
    article_dir: PathBuf,
}

impl ImageLoader for FsImageLoader {
    fn load(&self, src: &str) -> AppResult<Option<(String, Vec<u8>)>> {
        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
            return Ok(None);
        }
        // 依次相对文章目录、工作目录根解析
        let candidates = [self.article_dir.join(src), self.workspace_root.join(src)];
        for cand in candidates {
            if cand.is_file() {
                let bytes = std::fs::read(&cand)?;
                let filename = cand
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "image".into());
                return Ok(Some((filename, bytes)));
            }
        }
        // 本地引用但文件缺失 → 视为错误（避免产出带本地路径/坏图的草稿，FR-010a/SC-005）
        Err(AppError::NotFound(format!("本地图片不存在: {src}")))
    }
}

// ---- 平台连接命令（contracts/platform.md） ----

#[tauri::command]
pub fn list_platforms(publish: State<PublishState>) -> AppResult<Vec<PlatformConnection>> {
    Ok(PlatformId::all()
        .into_iter()
        .map(|p| publish.connection(p))
        .collect())
}

#[tauri::command]
pub async fn connect_platform(
    app: AppHandle,
    publish: State<'_, PublishState>,
    platform: PlatformId,
) -> AppResult<PlatformConnection> {
    // 打开平台登录 WebView，用户在其中完成登录（FR-001）。
    // 必须为 async 命令：同步命令在主线程执行，而 WebView2 的 build() 需事件循环转动
    // 才能完成异步创建，在主线程内会死锁（空白窗口、卡死）。async → worker 线程执行。
    let adapter = adapter_for(platform);
    let login_url = adapter.login_url().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        TauriBridge::new(app).open_login(platform, &login_url)
    })
    .await
    .map_err(|e| AppError::Io(format!("open_login join: {e}")))??;
    Ok(publish.connection(platform))
}

#[tauri::command]
pub fn get_platform_status(
    publish: State<PublishState>,
    platform: PlatformId,
) -> AppResult<PlatformConnection> {
    // 完整实现应注入 probe_login_js 实时探测（FR-006）；当前据会话存在性推断状态。
    Ok(publish.connection(platform))
}

#[tauri::command]
pub fn disconnect_platform(
    app: AppHandle,
    publish: State<PublishState>,
    platform: PlatformId,
) -> AppResult<()> {
    publish.session.clear(platform)?; // 删密文 + 删 OS 密钥（FR-004）
    publish.accounts.lock().expect("accounts lock").remove(&platform);
    let _ = TauriBridge::new(app).close(platform);
    Ok(())
}

/// 用户在登录窗口完成登录后调用：探测登录态，成功则记连接标记（FR-001/006）。
/// 探测经注入 JS 在平台 WebView 执行，需在 spawn_blocking 线程内阻塞等待回传。
#[tauri::command]
pub async fn confirm_connection(
    app: AppHandle,
    publish: State<'_, PublishState>,
    platform: PlatformId,
) -> AppResult<PlatformConnection> {
    let app2 = app.clone();
    let probe = tauri::async_runtime::spawn_blocking(move || {
        let bridge = TauriBridge::new(app2.clone());
        let adapter = adapter_for(platform);
        bridge.eval(platform, &adapter.probe_login_js())
    })
    .await
    .map_err(|e| AppError::Io(format!("probe join: {e}")))??;

    let logged_in = probe
        .get("loggedIn")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    let account = probe
        .get("account")
        .and_then(|a| a.as_str())
        .map(|s| s.to_string());

    if !logged_in {
        return Err(AppError::Auth(
            "尚未登录或登录态无效，请在窗口内完成登录后重试".into(),
        ));
    }
    // 保存连接标记（账号名）为加密会话载体，使重启后状态可恢复（FR-002/005）。
    let marker = account.clone().unwrap_or_default();
    publish.session.save(platform, marker.as_bytes())?;
    publish
        .accounts
        .lock()
        .expect("accounts lock")
        .insert(platform, account);
    Ok(publish.connection(platform))
}

// ---- 发布/同步/历史命令（contracts/publish.md） ----

#[tauri::command]
pub async fn sync_article(
    app: AppHandle,
    state: State<'_, AppState>,
    publish: State<'_, PublishState>,
    request: SyncRequest,
) -> AppResult<Vec<SyncJob>> {
    let root = state.current_root()?;
    let article_dir = root
        .join(&request.article_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.clone());

    // 后端防御性登录态校验（FR-012 第二道闸）：全部未连接直接拒绝。
    ensure_connected_or_failed(&publish, &request)?;

    // eval 会阻塞等待页面回传，必须在 spawn_blocking 线程执行，避免阻塞主线程事件循环（否则死锁）。
    let app2 = app.clone();
    let req = request.clone();
    let root2 = root.clone();
    let jobs = tauri::async_runtime::spawn_blocking(move || {
        let bridge = TauriBridge::new(app2.clone());
        let loader = FsImageLoader {
            workspace_root: root2,
            article_dir,
        };
        run_batch(&bridge, &loader, &req, adapter_for, |job| {
            // 逐平台进度事件（FR-014）
            let _ = app2.emit("publish://sync-progress", job);
        })
    })
    .await
    .map_err(|e| AppError::Io(format!("sync join: {e}")))?;

    // 写同步历史（FR-018）；失败不影响同步成败结论。
    write_history(&state, &jobs);
    Ok(jobs)
}

#[tauri::command]
pub async fn retry_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    publish: State<'_, PublishState>,
    article_path: String,
    rendered_html: String,
    markdown: String,
    title: String,
    digest: Option<String>,
    cover: Option<String>,
    platform: PlatformId,
) -> AppResult<SyncJob> {
    let req = SyncRequest {
        article_path,
        rendered_html,
        markdown,
        title,
        digest,
        cover,
        platforms: vec![platform],
    };
    let mut jobs = sync_article(app, state, publish, req).await?;
    jobs.pop()
        .ok_or_else(|| AppError::Invalid("重试未产生任务".into()))
}

#[tauri::command]
pub fn get_sync_history(
    state: State<AppState>,
    article_path: String,
) -> AppResult<Vec<SyncRecord>> {
    let conn = state.db.lock().expect("db lock");
    history::list_by_article(&conn, &article_path)
}

// ---- 内部辅助 ----

/// 未连接/失效平台直接判 Failed(reason=Auth)（FR-012）。若全部需拦截则在此提前返回 Auth。
fn ensure_connected_or_failed(publish: &PublishState, request: &SyncRequest) -> AppResult<()> {
    let any_connected = request
        .platforms
        .iter()
        .any(|p| publish.session.exists(*p));
    if !any_connected {
        return Err(AppError::Auth("目标平台均未连接，请先登录".into()));
    }
    Ok(())
}

fn write_history(state: &AppState, jobs: &[SyncJob]) {
    let conn = state.db.lock().expect("db lock");
    for job in jobs {
        let url = job.draft_ref.as_ref().and_then(|d| d.url.as_deref());
        let synced = job.finished_at.as_deref().unwrap_or("");
        let _ = history::insert(
            &conn,
            &job.article_path,
            job.platform,
            job.status,
            job.failure_reason.as_deref(),
            url,
            synced,
        );
    }
}

// 防止未使用告警（adapter trait 在本模块通过 adapter_for 间接使用）。
#[allow(dead_code)]
fn _assert_adapter_object_safe(a: &dyn PublishAdapter) -> PlatformId {
    a.id()
}
