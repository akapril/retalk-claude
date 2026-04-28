use super::SessionProvider;
use crate::models::Session;
use chrono::{TimeZone, Utc};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct AiderProvider;

impl SessionProvider for AiderProvider {
    fn name(&self) -> &str {
        "aider"
    }

    fn is_available(&self) -> bool {
        // 检查 aider 是否已安装（跨平台）
        let which = if cfg!(windows) { "where" } else { "which" };
        std::process::Command::new(which)
            .arg("aider")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn scan_all(&self) -> Vec<Session> {
        let mut sessions = Vec::new();
        let mut project_dirs: HashSet<PathBuf> = HashSet::new();

        // 扫描常见工作区目录（跨平台）
        let workspace_names = &["workspace", "projects", "code", "dev", "src", "repos"];

        #[cfg(windows)]
        {
            // Windows: 扫描多个盘符
            for workspace_name in workspace_names {
                for drive in &["C:", "D:", "E:"] {
                    let ws = PathBuf::from(format!("{}\\{}", drive, workspace_name));
                    if ws.exists() {
                        scan_aider_workspace(&ws, &mut project_dirs);
                    }
                }
            }
        }

        #[cfg(not(windows))]
        {
            // macOS/Linux: 扫描 home 下的常见目录
            if let Some(home) = dirs::home_dir() {
                for workspace_name in workspace_names {
                    let ws = home.join(workspace_name);
                    if ws.exists() {
                        scan_aider_workspace(&ws, &mut project_dirs);
                    }
                }
            }
        }

        // 也检查 home 目录本身
        if let Some(home) = dirs::home_dir() {
            if home.join(".aider.chat.history.md").exists() {
                project_dirs.insert(home);
            }
        }

        for dir in project_dirs {
            let history_file = dir.join(".aider.chat.history.md");
            if let Some(session) = parse_aider_history(&history_file, &dir) {
                sessions.push(session);
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}

/// 扫描工作区目录下的 Aider 项目
fn scan_aider_workspace(ws: &Path, project_dirs: &mut HashSet<PathBuf>) {
    if let Ok(entries) = fs::read_dir(ws) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(".aider.chat.history.md").exists() {
                project_dirs.insert(path);
            }
        }
    }
}

/// 解析 Aider 的 .aider.chat.history.md 文件
fn parse_aider_history(path: &Path, project_dir: &Path) -> Option<Session> {
    let content = fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        return None;
    }

    // 提取用户消息（"#### " 开头的行为用户输入）
    let mut user_messages = Vec::new();
    let mut in_user_block = false;
    let mut current_msg = String::new();

    for line in content.lines() {
        if line.starts_with("#### ") {
            // 保存上一条消息
            if !current_msg.trim().is_empty() {
                user_messages.push(current_msg.trim().to_string());
            }
            current_msg = line.trim_start_matches("#### ").to_string();
            in_user_block = true;
        } else if in_user_block && line.starts_with("> ") {
            current_msg += " ";
            current_msg += line.trim_start_matches("> ");
        } else if in_user_block && !line.trim().is_empty() && !line.starts_with('>') {
            in_user_block = false;
        }
    }
    if !current_msg.trim().is_empty() {
        user_messages.push(current_msg.trim().to_string());
    }

    if user_messages.is_empty() {
        return None;
    }

    // 使用文件修改时间作为时间戳
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| {
            let dur = t.duration_since(std::time::UNIX_EPOCH).ok()?;
            Utc.timestamp_opt(dur.as_secs() as i64, 0).single()
        })
        .unwrap_or_else(Utc::now);

    // 用文件路径的哈希生成稳定的 session ID
    let session_id = format!("aider-{:x}", fnv_hash(path.to_string_lossy().as_bytes()));

    let project_name = project_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Some(Session {
        session_id,
        provider: "aider".to_string(),
        project_path: project_dir.to_string_lossy().to_string(),
        project_name,
        first_prompt: user_messages.first().cloned().unwrap_or_default(),
        last_prompt: user_messages.last().cloned().unwrap_or_default(),
        created_at: mtime,
        updated_at: mtime,
        message_count: user_messages.len() as u32,
        user_messages,
        total_tokens: 0, // Aider 历史文件中无 token 数据
    })
}

/// FNV-1a 哈希，用于生成稳定的 session ID（非加密用途）
fn fnv_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
