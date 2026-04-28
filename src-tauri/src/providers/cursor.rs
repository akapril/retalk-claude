use super::SessionProvider;
use crate::models::Session;
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
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

            // 尝试从 SQLite 读取聊天内容
            let user_messages = read_cursor_chats(&vscdb);
            let first_prompt = user_messages
                .first()
                .cloned()
                .unwrap_or_else(|| "Cursor workspace".to_string());
            let last_prompt = user_messages
                .last()
                .cloned()
                .unwrap_or_else(|| "Cursor workspace".to_string());
            let message_count = user_messages.len() as u32;

            sessions.push(Session {
                session_id: format!("cursor-{}", hash_name),
                provider: "cursor".to_string(),
                project_path,
                project_name,
                first_prompt,
                last_prompt,
                created_at: mtime,
                updated_at: mtime,
                message_count,
                user_messages,
                total_tokens: 0,
            });
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}

/// 解码 file:// URI 为本地路径（跨平台）
fn decode_file_uri(uri: &str) -> String {
    let path = uri
        .strip_prefix("file:///")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri);
    let decoded = percent_decode(path);
    // Windows 将 / 转为 \，Unix 保持 / 并补前缀
    if cfg!(windows) {
        decoded.replace("/", "\\")
    } else {
        format!("/{}", decoded)
    }
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

/// 从 state.vscdb 的 cursorDiskKV 表读取聊天内容
fn read_cursor_chats(vscdb_path: &Path) -> Vec<String> {
    let conn = match Connection::open_with_flags(
        vscdb_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut messages = Vec::new();

    // 尝试读取 chatdata（旧版存储格式）
    if let Ok(mut stmt) = conn.prepare(
        "SELECT value FROM cursorDiskKV WHERE key = 'workbench.panel.aichat.view.aichat.chatdata'",
    ) {
        if let Ok(Some(value)) = stmt.query_row([], |row| row.get::<_, Option<String>>(0)) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&value) {
                extract_user_messages_from_json(&data, &mut messages);
            }
        }
    }

    // 尝试读取 composerData（新版 Composer 格式）
    if messages.is_empty() {
        if let Ok(mut stmt) =
            conn.prepare("SELECT value FROM cursorDiskKV WHERE key = 'composer.composerData'")
        {
            if let Ok(Some(value)) = stmt.query_row([], |row| row.get::<_, Option<String>>(0)) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&value) {
                    extract_user_messages_from_json(&data, &mut messages);
                }
            }
        }
    }

    messages
}

/// 递归提取 JSON 中的用户消息
fn extract_user_messages_from_json(data: &serde_json::Value, messages: &mut Vec<String>) {
    match data {
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_user_messages_from_json(item, messages);
            }
        }
        serde_json::Value::Object(obj) => {
            // 匹配 role=user 格式的消息
            if obj.get("role").and_then(|r| r.as_str()) == Some("user") {
                if let Some(content) = obj.get("content").and_then(|c| c.as_str()) {
                    if !content.is_empty() {
                        messages.push(content.to_string());
                    }
                }
            }
            // 匹配 type=human 或 type=user 格式
            if let Some(t) = obj.get("type").and_then(|t| t.as_str()) {
                if t == "human" || t == "user" {
                    if let Some(content) = obj
                        .get("text")
                        .or_else(|| obj.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        if !content.is_empty() && !messages.contains(&content.to_string()) {
                            messages.push(content.to_string());
                        }
                    }
                }
            }
            // 递归搜索嵌套对象/数组
            for (_, v) in obj {
                if v.is_array() || v.is_object() {
                    extract_user_messages_from_json(v, messages);
                }
            }
        }
        _ => {}
    }
}
