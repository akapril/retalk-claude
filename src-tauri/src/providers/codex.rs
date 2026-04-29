use super::SessionProvider;
use crate::models::Session;
use chrono::{DateTime, Utc};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub struct CodexProvider;

fn codex_dir() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".codex")
}

/// Codex JSONL 行
#[derive(serde::Deserialize)]
struct CodexEntry {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    entry_type: Option<String>,
    payload: Option<serde_json::Value>,
}

impl SessionProvider for CodexProvider {
    fn name(&self) -> &str {
        "codex"
    }

    fn is_available(&self) -> bool {
        codex_dir().join("sessions").exists()
    }

    fn scan_all(&self) -> Vec<Session> {
        let sessions_dir = codex_dir().join("sessions");
        let mut sessions = Vec::new();

        // 递归遍历 sessions/ 下的所有 .jsonl 文件
        visit_dir(&sessions_dir, &mut sessions);

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}

fn visit_dir(dir: &Path, sessions: &mut Vec<Session>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, sessions);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if let Some(session) = scan_single_codex_session(&path) {
                sessions.push(session);
            }
        }
    }
}

/// 扫描单个 Codex session JSONL 文件（供 watcher 增量更新使用）
pub fn scan_single_codex_session(path: &Path) -> Option<Session> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = String::new();
    let mut cwd = String::new();
    let mut user_messages = Vec::new();
    let mut first_timestamp: Option<DateTime<Utc>> = None;
    let mut last_timestamp: Option<DateTime<Utc>> = None;
    let mut total_tokens: u64 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let entry: CodexEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // 解析时间戳
        if let Some(ts_str) = &entry.timestamp {
            if let Ok(ts) = ts_str.parse::<DateTime<Utc>>() {
                if first_timestamp.is_none() {
                    first_timestamp = Some(ts);
                }
                last_timestamp = Some(ts);
            }
        }

        match entry.entry_type.as_deref() {
            Some("session_meta") => {
                if let Some(payload) = &entry.payload {
                    if let Some(id) = payload.get("id").and_then(|v| v.as_str()) {
                        session_id = id.to_string();
                    }
                    if let Some(c) = payload.get("cwd").and_then(|v| v.as_str()) {
                        cwd = c.to_string();
                    }
                }
            }
            Some("event_msg") => {
                if let Some(payload) = &entry.payload {
                    // 提取 token 统计
                    if payload.get("type").and_then(|v| v.as_str()) == Some("token_count") {
                        let input = payload.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let output = payload.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        total_tokens += input + output;
                    }
                    // 提取用户消息
                    if payload.get("type").and_then(|v| v.as_str()) == Some("user_message") {
                        if let Some(msg) = payload.get("message").and_then(|v| v.as_str()) {
                            if !msg.is_empty() {
                                user_messages.push(msg.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if user_messages.is_empty() || session_id.is_empty() {
        return None;
    }

    let project_name = Path::new(&cwd)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Some(Session {
        session_id,
        provider: "codex".to_string(),
        project_path: cwd,
        project_name,
        first_prompt: user_messages.first().cloned().unwrap_or_default(),
        last_prompt: user_messages.last().cloned().unwrap_or_default(),
        created_at: first_timestamp.unwrap_or_else(Utc::now),
        updated_at: last_timestamp.unwrap_or_else(Utc::now),
        message_count: user_messages.len() as u32,
        user_messages,
        total_tokens,
    })
}
