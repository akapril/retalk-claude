use super::SessionProvider;
use crate::models::Session;
use chrono::{TimeZone, Utc};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CursorProvider;

/// 获取 Cursor 的 workspaceStorage 目录
fn cursor_workspace_dir() -> Option<PathBuf> {
    // %APPDATA%\Cursor\User\workspaceStorage
    dirs::config_dir().map(|c| c.join("Cursor").join("User").join("workspaceStorage"))
}

impl SessionProvider for CursorProvider {
    fn name(&self) -> &str {
        "cursor"
    }

    fn is_available(&self) -> bool {
        cursor_workspace_dir().map(|d| d.exists()).unwrap_or(false)
    }

    fn scan_all(&self) -> Vec<Session> {
        let ws_dir = match cursor_workspace_dir() {
            Some(d) if d.exists() => d,
            _ => return Vec::new(),
        };

        let mut sessions = Vec::new();
        let entries = match fs::read_dir(&ws_dir) {
            Ok(e) => e,
            Err(_) => return sessions,
        };

        for entry in entries.flatten() {
            let hash_dir = entry.path();
            if !hash_dir.is_dir() {
                continue;
            }

            let workspace_json = hash_dir.join("workspace.json");
            if !workspace_json.exists() {
                continue;
            }

            // 读取 workspace.json 获取项目路径
            let content = match fs::read_to_string(&workspace_json) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let data: serde_json::Value = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(_) => continue,
            };

            // workspace.json 中 "folder" 字段包含 file:// URI
            let folder_uri = match data.get("folder").and_then(|f| f.as_str()) {
                Some(f) => f,
                None => continue,
            };

            let project_path = decode_file_uri(folder_uri);
            if project_path.is_empty() {
                continue;
            }

            // 检查 state.vscdb 是否存在（表明有实际使用）
            let vscdb = hash_dir.join("state.vscdb");
            if !vscdb.exists() {
                continue;
            }

            let mtime = fs::metadata(&vscdb)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| {
                    let dur = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                    Utc.timestamp_opt(dur.as_secs() as i64, 0).single()
                })
                .unwrap_or_else(Utc::now);

            let hash_name = hash_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let project_name = Path::new(&project_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            sessions.push(Session {
                session_id: format!("cursor-{}", hash_name),
                provider: "cursor".to_string(),
                project_path,
                project_name,
                first_prompt: "Cursor workspace".to_string(),
                last_prompt: "Cursor workspace".to_string(),
                created_at: mtime,
                updated_at: mtime,
                message_count: 0,
                user_messages: Vec::new(),
                total_tokens: 0, // Cursor 需要 SQLite 读取，暂不支持
            });
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}

/// 解码 file:// URI 为 Windows 路径
fn decode_file_uri(uri: &str) -> String {
    let path = uri
        .strip_prefix("file:///")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri);
    let decoded = percent_decode(path);
    decoded.replace("/", "\\")
}

/// URL 百分号解码
fn percent_decode(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else {
            result.push(c);
        }
    }
    result
}
