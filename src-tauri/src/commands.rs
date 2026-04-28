use crate::config;
use crate::indexer::SessionIndex;
use crate::models::{AppConfig, GitInfo, Session, TagsMap, UpdateStats};
use crate::searcher::{self, SearchResult};
use crate::terminal;
use crate::updater::Updater;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use tauri::State;

/// 应用全局状态
pub struct AppState {
    pub index: Arc<Mutex<SessionIndex>>,
    pub updater: Arc<Updater>,
    pub config: Arc<Mutex<AppConfig>>,
    /// 缓存的全量会话列表（供预览等功能查询）
    pub sessions: Arc<Mutex<Vec<Session>>>,
    /// 收藏的会话 ID 列表
    pub favorites: Arc<Mutex<Vec<String>>>,
    /// 会话标签：session_id -> [tag1, tag2, ...]
    pub tags: Arc<Mutex<TagsMap>>,
}

/// 全文搜索会话
#[tauri::command]
pub fn search(
    state: State<AppState>,
    query: String,
) -> Vec<SearchResult> {
    let index = state.index.lock();
    let max = state.config.lock().ui.max_results;
    searcher::search(&index, &query, max)
}

/// 列出所有会话（按更新时间降序），并按需刷新索引
#[tauri::command]
pub fn list_sessions(
    state: State<AppState>,
) -> Vec<SearchResult> {
    let config = state.config.lock().clone();

    // 按需刷新：先获取索引锁，刷新后释放
    let refreshed = {
        let index = state.index.lock();
        state.updater.on_demand_refresh(&index, &config)
    };

    // 如果发生了刷新，同步更新 sessions 缓存
    if refreshed {
        let fresh = crate::scanner::scan_all_sessions();
        *state.sessions.lock() = fresh;
    }

    let index = state.index.lock();
    searcher::list_all(&index, config.ui.max_results)
}

/// 在终端中恢复 AI 编码工具会话
#[tauri::command]
pub fn resume_session(
    state: State<AppState>,
    session_id: String,
    project_path: String,
    provider: String,
) -> Result<(), String> {
    let config = state.config.lock();
    let term = terminal::detect_terminal(&config.terminal.preferred);
    terminal::resume_in_terminal(&term, &provider, &project_path, &session_id)
}

/// 构建 resume 命令字符串（用于复制到剪贴板）
#[tauri::command]
pub fn copy_command(
    session_id: String,
    project_path: String,
    provider: String,
) -> String {
    terminal::build_resume_command(&provider, &project_path, &session_id)
}

/// 获取更新策略性能统计
#[tauri::command]
pub fn get_stats(state: State<AppState>) -> Vec<UpdateStats> {
    state.updater.get_stats()
}

/// 获取当前配置
#[tauri::command]
pub fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().clone()
}

/// 保存配置并更新内存中的配置状态
#[tauri::command]
pub fn save_config(
    state: State<AppState>,
    new_config: AppConfig,
) {
    config::save_config(&new_config);
    *state.config.lock() = new_config;
}

// ============================================================
// Feature 1: 会话预览 — 返回指定会话的最后 3 条用户消息
// ============================================================

#[tauri::command]
pub fn get_session_preview(
    state: State<AppState>,
    session_id: String,
) -> Vec<String> {
    let sessions = state.sessions.lock();
    sessions
        .iter()
        .find(|s| s.session_id == session_id)
        .map(|s| {
            let msgs = &s.user_messages;
            let start = if msgs.len() > 3 { msgs.len() - 3 } else { 0 };
            msgs[start..].to_vec()
        })
        .unwrap_or_default()
}

// ============================================================
// Feature 2: 项目 Git 信息
// ============================================================

#[tauri::command]
pub fn get_project_git_info(project_path: String) -> Option<GitInfo> {
    // 获取当前分支名
    let branch_output = Command::new("git")
        .args(["-C", &project_path, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !branch_output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();

    // 获取未提交变更数
    let status_output = Command::new("git")
        .args(["-C", &project_path, "status", "--porcelain"])
        .output()
        .ok()?;
    let dirty_count = String::from_utf8_lossy(&status_output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .count() as u32;

    Some(GitInfo {
        branch,
        dirty_count,
    })
}

// ============================================================
// Feature 3: 收藏/置顶
// ============================================================

/// 收藏文件路径
fn favorites_path() -> std::path::PathBuf {
    config::retalk_dir().join("favorites.json")
}

/// 从磁盘加载收藏列表
pub fn load_favorites() -> Vec<String> {
    let path = favorites_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// 保存收藏列表到磁盘
fn save_favorites(favs: &[String]) {
    let path = favorites_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(favs).unwrap_or_default());
}

#[tauri::command]
pub fn toggle_favorite(
    state: State<AppState>,
    session_id: String,
) -> bool {
    let mut favs = state.favorites.lock();
    let is_fav = if let Some(pos) = favs.iter().position(|id| id == &session_id) {
        favs.remove(pos);
        false
    } else {
        favs.push(session_id);
        true
    };
    save_favorites(&favs);
    is_fav
}

#[tauri::command]
pub fn get_favorites(state: State<AppState>) -> Vec<String> {
    state.favorites.lock().clone()
}

// ============================================================
// 会话导出 — 导出为 Markdown
// ============================================================

#[tauri::command]
pub fn export_session_markdown(
    state: State<AppState>,
    session_id: String,
) -> Result<String, String> {
    let sessions = state.sessions.lock();
    let session = sessions
        .iter()
        .find(|s| s.session_id == session_id)
        .ok_or("会话未找到")?;

    let mut md = format!("# {} - {}\n\n", session.project_name, session.provider);
    md += &format!("**项目路径:** {}\n", session.project_path);
    md += &format!("**会话ID:** {}\n", session.session_id);
    md += &format!("**消息数:** {}\n", session.message_count);
    if session.total_tokens > 0 {
        md += &format!("**Token 数:** {}\n", session.total_tokens);
    }
    md += "\n---\n\n";

    for (i, msg) in session.user_messages.iter().enumerate() {
        md += &format!("### 消息 {}\n\n{}\n\n", i + 1, msg);
    }

    Ok(md)
}

#[tauri::command]
pub fn export_session_to_file(
    state: State<AppState>,
    session_id: String,
    file_path: String,
) -> Result<(), String> {
    let md = export_session_markdown_inner(&state, &session_id)?;
    std::fs::write(&file_path, md).map_err(|e| format!("写入文件失败: {}", e))
}

/// 内部复用：生成 Markdown 文本
fn export_session_markdown_inner(
    state: &State<AppState>,
    session_id: &str,
) -> Result<String, String> {
    let sessions = state.sessions.lock();
    let session = sessions
        .iter()
        .find(|s| s.session_id == session_id)
        .ok_or("会话未找到")?;

    let mut md = format!("# {} - {}\n\n", session.project_name, session.provider);
    md += &format!("**项目路径:** {}\n", session.project_path);
    md += &format!("**会话ID:** {}\n", session.session_id);
    md += &format!("**消息数:** {}\n", session.message_count);
    if session.total_tokens > 0 {
        md += &format!("**Token 数:** {}\n", session.total_tokens);
    }
    md += "\n---\n\n";

    for (i, msg) in session.user_messages.iter().enumerate() {
        md += &format!("### 消息 {}\n\n{}\n\n", i + 1, msg);
    }

    Ok(md)
}

/// 获取用户桌面路径
#[tauri::command]
pub fn get_desktop_path() -> Result<String, String> {
    dirs::desktop_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "无法获取桌面路径".to_string())
}

// ============================================================
// Feature 4: 快捷操作 — 在 VS Code / 文件管理器中打开
// ============================================================

#[tauri::command]
pub fn open_in_vscode(project_path: String) -> Result<(), String> {
    Command::new("code")
        .arg(&project_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_in_explorer(project_path: String) -> Result<(), String> {
    Command::new("explorer")
        .arg(&project_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ============================================================
// Feature 6: 会话标签系统
// ============================================================

/// 标签文件路径
fn tags_path() -> std::path::PathBuf {
    config::retalk_dir().join("tags.json")
}

/// 从磁盘加载标签
pub fn load_tags() -> TagsMap {
    let path = tags_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

/// 保存标签到磁盘
fn save_tags(tags: &TagsMap) {
    let path = tags_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(tags).unwrap_or_default());
}

#[tauri::command]
pub fn set_tags(
    state: State<AppState>,
    session_id: String,
    tags: Vec<String>,
) {
    let mut all_tags = state.tags.lock();
    if tags.is_empty() {
        all_tags.remove(&session_id);
    } else {
        all_tags.insert(session_id, tags);
    }
    save_tags(&all_tags);
}

#[tauri::command]
pub fn get_all_tags(state: State<AppState>) -> TagsMap {
    state.tags.lock().clone()
}
