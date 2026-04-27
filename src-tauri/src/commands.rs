use crate::config;
use crate::indexer::SessionIndex;
use crate::models::{AppConfig, UpdateStats};
use crate::searcher::{self, SearchResult};
use crate::terminal;
use crate::updater::Updater;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::State;

/// 应用全局状态
pub struct AppState {
    pub index: Arc<Mutex<SessionIndex>>,
    pub updater: Arc<Updater>,
    pub config: Arc<Mutex<AppConfig>>,
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
    {
        let index = state.index.lock();
        state.updater.on_demand_refresh(&index, &config);
    }

    let index = state.index.lock();
    searcher::list_all(&index, config.ui.max_results)
}

/// 在终端中恢复 Claude Code 会话
#[tauri::command]
pub fn resume_session(
    state: State<AppState>,
    session_id: String,
    project_path: String,
) -> Result<(), String> {
    let config = state.config.lock();
    let term = terminal::detect_terminal(&config.terminal.preferred);
    terminal::resume_in_terminal(&term, &project_path, &session_id)
}

/// 构建 resume 命令字符串（用于复制到剪贴板）
#[tauri::command]
pub fn copy_command(
    session_id: String,
    project_path: String,
) -> String {
    terminal::build_resume_command(&project_path, &session_id)
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
