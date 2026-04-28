use super::SessionProvider;
use crate::models::Session;
use chrono::{TimeZone, Utc};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ContinueProvider;

fn continue_dir() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".continue")
}

#[derive(serde::Deserialize)]
struct ContinueSessionFile {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "workspaceDirectory")]
    workspace_directory: Option<String>,
    history: Option<Vec<ContinueHistoryEntry>>,
}

#[derive(serde::Deserialize)]
struct ContinueHistoryEntry {
    message: Option<ContinueMessage>,
}

#[derive(serde::Deserialize)]
struct ContinueMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

impl SessionProvider for ContinueProvider {
    fn name(&self) -> &str {
        "continue"
    }

    fn is_available(&self) -> bool {
        continue_dir().join("sessions").exists()
    }

    fn scan_all(&self) -> Vec<Session> {
        let sessions_dir = continue_dir().join("sessions");
        let mut sessions = Vec::new();

        // 读取所有 .json 文件（排除 sessions.json 索引文件）
        let entries = match fs::read_dir(&sessions_dir) {
            Ok(e) => e,
            Err(_) => return sessions,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) == Some("sessions.json") {
                continue;
            }
            if let Some(session) = parse_continue_session(&path) {
                sessions.push(session);
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}

fn parse_continue_session(path: &Path) -> Option<Session> {
    let content = fs::read_to_string(path).ok()?;
    let data: ContinueSessionFile = serde_json::from_str(&content).ok()?;

    let session_id = data.session_id.unwrap_or_default();
    if session_id.is_empty() {
        return None;
    }

    // 解码 workspace 路径: "file:///d%3A/workspace/foo" -> "D:\workspace\foo"
    let workspace = data.workspace_directory.unwrap_or_default();
    let project_path = decode_file_uri(&workspace);

    let project_name = Path::new(&project_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // 提取用户消息
    let mut user_messages = Vec::new();
    if let Some(history) = &data.history {
        for entry in history {
            if let Some(msg) = &entry.message {
                if msg.role.as_deref() == Some("user") {
                    if let Some(content) = &msg.content {
                        let text = match content {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Array(arr) => arr
                                .iter()
                                .filter_map(|item| {
                                    if item.get("type")?.as_str()? == "text" {
                                        item.get("text")?.as_str().map(|s| s.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n"),
                            _ => String::new(),
                        };
                        if !text.is_empty() {
                            user_messages.push(text);
                        }
                    }
                }
            }
        }
    }

    if user_messages.is_empty() {
        return None;
    }

    // 使用文件 mtime 作为时间戳
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| {
            let dur = t.duration_since(std::time::UNIX_EPOCH).ok()?;
            Utc.timestamp_opt(dur.as_secs() as i64, 0).single()
        })
        .unwrap_or_else(Utc::now);

    Some(Session {
        session_id,
        provider: "continue".to_string(),
        project_path,
        project_name,
        first_prompt: user_messages.first().cloned().unwrap_or_default(),
        last_prompt: user_messages.last().cloned().unwrap_or_default(),
        created_at: mtime,
        updated_at: mtime,
        message_count: user_messages.len() as u32,
        user_messages,
        total_tokens: 0, // Continue 会话文件中无 token 数据
    })
}

/// 解码 file:// URI 为本地路径（跨平台）
fn decode_file_uri(uri: &str) -> String {
    let path = uri
        .strip_prefix("file:///")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri);
    // URL 解码
    let decoded = percent_decode(path);
    // Windows 将 / 转为 \，Unix 保持 /
    if cfg!(windows) {
        decoded.replace("/", "\\")
    } else {
        // Unix: file:///home/user/... -> /home/user/...
        format!("/{}", decoded)
    }
}

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
