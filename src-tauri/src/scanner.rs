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

/// 根据文件路径判断所属工具，增量扫描单个会话文件
pub fn scan_single_session(path: &Path) -> Option<Session> {
    let path_str = path.to_string_lossy();

    if path_str.contains(".claude") {
        crate::providers::claude::scan_single_claude_session(path)
    } else if path_str.contains(".codex") {
        crate::providers::codex::scan_single_codex_session(path)
    } else if path_str.contains(".gemini") {
        crate::providers::gemini::scan_single_gemini_session(path)
    } else {
        None
    }
}
