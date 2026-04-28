use crate::models::Session;
use crate::providers;
use std::path::Path;

/// 扫描所有已安装的 AI 工具的会话数据
pub fn scan_all_sessions() -> Vec<Session> {
    let mut all_sessions = Vec::new();

    for provider in providers::all_providers() {
        let mut sessions = provider.scan_all();
        eprintln!("[retalk] provider '{}': {} sessions", provider.name(), sessions.len());
        all_sessions.append(&mut sessions);
    }

    // 按 updated_at 降序排序
    all_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    all_sessions
}

/// 扫描单个 Claude Code session 文件（用于 watcher 增量更新）
pub fn scan_single_session(path: &Path) -> Option<Session> {
    // 目前仅 Claude Code 的 watcher 使用此函数
    crate::providers::claude::scan_single_claude_session(path)
}
