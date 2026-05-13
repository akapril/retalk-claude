use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// 时间线消息：用于会话回放视图
#[derive(Debug, Clone, Serialize)]
pub struct TimelineMessage {
    pub role: String,              // "user" / "assistant" / "tool" / "system"
    pub content: String,           // 消息内容（截断到 500 字符）
    pub timestamp: String,         // 可读时间（HH:MM:SS）
    pub token_count: u64,          // 该消息的 token 数
    pub tool_name: Option<String>, // 工具调用名称（仅 role="tool" 时有值）
}

/// 读取会话的完整时间线（根据 provider 分发到对应解析器）
pub fn read_timeline(provider: &str, session_id: &str) -> Vec<TimelineMessage> {
    let mut messages = match provider {
        "claude" => read_claude_timeline(session_id),
        "codex" => read_codex_timeline(session_id),
        "gemini" => read_gemini_timeline(session_id),
        "opencode" | "kilo" => read_sqlite_timeline(provider, session_id),
        _ => Vec::new(),
    };
    // 限制最大消息数，防止超大 IPC 响应
    if messages.len() > 500 {
        messages.truncate(500);
    }
    messages
}

// ============================================================
// Claude 时间线解析 — JSONL 格式
// ============================================================

fn read_claude_timeline(session_id: &str) -> Vec<TimelineMessage> {
    let home = dirs::home_dir().unwrap_or_default();
    let projects_dir = home.join(".claude").join("projects");

    // 查找会话文件
    let path = match find_session_file(&projects_dir, session_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let raw: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = raw
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        match entry_type {
            "user" => {
                // 用户消息：message.content 可能是字符串或 [{type:"text",text:"..."}] 数组
                if let Some(msg) = raw.get("message") {
                    let content = extract_content_text(msg.get("content"));
                    if !content.is_empty() {
                        messages.push(TimelineMessage {
                            role: "user".to_string(),
                            content: truncate(&content, 500),
                            timestamp: format_timestamp(&timestamp),
                            token_count: 0,
                            tool_name: None,
                        });
                    }
                }
            }
            "assistant" => {
                // 助手消息：提取 text 块和 tool_use 块
                if let Some(msg) = raw.get("message") {
                    let mut tokens: u64 = 0;
                    if let Some(usage) = msg.get("usage") {
                        tokens = usage
                            .get("input_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0)
                            + usage
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                    }

                    if let Some(content_arr) = msg.get("content").and_then(|c| c.as_array()) {
                        for block in content_arr {
                            let block_type =
                                block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            match block_type {
                                "text" => {
                                    let text =
                                        block.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                    if !text.is_empty() {
                                        messages.push(TimelineMessage {
                                            role: "assistant".to_string(),
                                            content: truncate(text, 500),
                                            timestamp: format_timestamp(&timestamp),
                                            token_count: tokens,
                                            tool_name: None,
                                        });
                                        // token 计数只附加到第一个 text 块
                                        tokens = 0;
                                    }
                                }
                                "tool_use" => {
                                    let name = block
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let input_str = block
                                        .get("input")
                                        .map(|v| serde_json::to_string(v).unwrap_or_default())
                                        .unwrap_or_default();
                                    messages.push(TimelineMessage {
                                        role: "tool".to_string(),
                                        content: truncate(&input_str, 300),
                                        timestamp: format_timestamp(&timestamp),
                                        token_count: 0,
                                        tool_name: Some(name.to_string()),
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {} // 跳过 system、summary、file-history-snapshot 等
        }
    }

    messages
}

// ============================================================
// Codex 时间线解析 — JSONL 格式
// ============================================================

fn read_codex_timeline(session_id: &str) -> Vec<TimelineMessage> {
    let home = dirs::home_dir().unwrap_or_default();
    let sessions_dir = home.join(".codex").join("sessions");

    let path = match find_session_file_recursive(&sessions_dir, session_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let raw: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = raw
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let payload = raw.get("payload");

        if entry_type == "event_msg" {
            if let Some(p) = payload {
                let ptype = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match ptype {
                    "user_message" => {
                        let text = p.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        if !text.is_empty() {
                            messages.push(TimelineMessage {
                                role: "user".to_string(),
                                content: truncate(text, 500),
                                timestamp: format_timestamp(&timestamp),
                                token_count: 0,
                                tool_name: None,
                            });
                        }
                    }
                    "agent_message" => {
                        let text = p.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        if !text.is_empty() {
                            messages.push(TimelineMessage {
                                role: "assistant".to_string(),
                                content: truncate(text, 500),
                                timestamp: format_timestamp(&timestamp),
                                token_count: 0,
                                tool_name: None,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    messages
}

// ============================================================
// Gemini 时间线解析 — JSON 格式
// ============================================================

fn read_gemini_timeline(session_id: &str) -> Vec<TimelineMessage> {
    let home = dirs::home_dir().unwrap_or_default();
    let tmp_dir = home.join(".gemini").join("tmp");

    let path = match find_gemini_session(&tmp_dir, session_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let raw = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    // 清洗控制字符（Gemini JSON 可能含有非法字符）
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect();
    let data: serde_json::Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut messages = Vec::new();
    if let Some(msg_arr) = data.get("messages").and_then(|v| v.as_array()) {
        for msg in msg_arr {
            let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let timestamp = msg
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = extract_gemini_content(msg.get("content"));

            if content.is_empty() {
                continue;
            }

            let role = match msg_type {
                "user" => "user",
                "gemini" | "model" => "assistant",
                _ => "system",
            };

            messages.push(TimelineMessage {
                role: role.to_string(),
                content: truncate(&content, 500),
                timestamp: format_timestamp(&timestamp),
                token_count: 0,
                tool_name: None,
            });
        }
    }

    messages
}

// ============================================================
// SQLite 时间线解析 — OpenCode / Kilo
// ============================================================

fn read_sqlite_timeline(provider: &str, session_id: &str) -> Vec<TimelineMessage> {
    let home = dirs::home_dir().unwrap_or_default();
    let db_path = match provider {
        "opencode" => home
            .join(".local")
            .join("share")
            .join("opencode")
            .join("opencode.db"),
        "kilo" => home
            .join(".local")
            .join("share")
            .join("kilo")
            .join("kilo.db"),
        _ => return Vec::new(),
    };

    let conn = match rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut stmt = match conn.prepare(
        "SELECT data, time_created FROM message WHERE session_id = ?1 ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows: Vec<(String, i64)> = stmt
        .query_map([session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .ok()
        .map(|r| r.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    let mut messages = Vec::new();
    for (data_str, ts) in rows {
        let data: serde_json::Value = match serde_json::from_str(&data_str) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let role = data
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("system");

        // 从 summary.title 提取内容摘要
        let content = data
            .get("summary")
            .and_then(|v| v.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if !content.is_empty() {
            let dt = chrono::DateTime::from_timestamp(ts / 1000, 0)
                .map(|d| d.format("%H:%M:%S").to_string())
                .unwrap_or_default();
            messages.push(TimelineMessage {
                role: role.to_string(),
                content: truncate(&content, 500),
                timestamp: dt,
                token_count: 0,
                tool_name: None,
            });
        }
    }

    messages
}

// ============================================================
// 辅助函数
// ============================================================

/// 在 projects 目录下查找 {session_id}.jsonl 文件（一级子目录）
fn find_session_file(projects_dir: &Path, session_id: &str) -> Option<std::path::PathBuf> {
    let target = format!("{}.jsonl", session_id);
    if !projects_dir.exists() {
        return None;
    }
    for entry in fs::read_dir(projects_dir).ok()?.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let file = dir.join(&target);
        if file.exists() {
            return Some(file);
        }
    }
    None
}

/// 递归查找包含 session_id 的 .jsonl 文件
fn find_session_file_recursive(dir: &Path, session_id: &str) -> Option<std::path::PathBuf> {
    if !dir.exists() {
        return None;
    }
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_session_file_recursive(&path, session_id) {
                return Some(found);
            }
        } else if path.to_string_lossy().contains(session_id)
            && path.extension().and_then(|e| e.to_str()) == Some("jsonl")
        {
            return Some(path);
        }
    }
    None
}

/// 查找 Gemini 会话文件（~/.gemini/tmp/*/chats/ 下匹配 session_id 的 .json）
fn find_gemini_session(tmp_dir: &Path, session_id: &str) -> Option<std::path::PathBuf> {
    if !tmp_dir.exists() {
        return None;
    }
    for entry in fs::read_dir(tmp_dir).ok()?.flatten() {
        let chats = entry.path().join("chats");
        if !chats.exists() {
            continue;
        }
        for f in fs::read_dir(&chats).ok()?.flatten() {
            let fname = f.file_name().to_string_lossy().to_string();
            if fname.contains(session_id) && fname.ends_with(".json") {
                return Some(f.path());
            }
        }
    }
    None
}

/// 提取 Claude 消息内容（兼容字符串和数组格式）
fn extract_content_text(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
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
    }
}

/// 提取 Gemini 消息内容（兼容字符串和 [{text:"..."}] 数组）
fn extract_gemini_content(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| item.get("text")?.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// 截断字符串到指定字符数（超出部分用 "..." 替代）
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        s.chars().take(max).collect::<String>() + "..."
    } else {
        s.to_string()
    }
}

/// 格式化时间戳为 HH:MM:SS（支持 ISO 8601 解析）
fn format_timestamp(ts: &str) -> String {
    if let Ok(dt) = ts.parse::<chrono::DateTime<chrono::Utc>>() {
        dt.format("%H:%M:%S").to_string()
    } else {
        ts.to_string()
    }
}
