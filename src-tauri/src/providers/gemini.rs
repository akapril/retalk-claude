use super::SessionProvider;
use crate::models::Session;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

pub struct GeminiProvider;

fn gemini_dir() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".gemini")
}

#[derive(serde::Deserialize)]
struct GeminiSession {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "startTime")]
    start_time: Option<String>,
    #[serde(rename = "lastUpdated")]
    last_updated: Option<String>,
    messages: Option<Vec<GeminiMessage>>,
}

#[derive(serde::Deserialize)]
struct GeminiMessage {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    content: Option<String>,
}

impl SessionProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn is_available(&self) -> bool {
        gemini_dir().join("tmp").exists()
    }

    fn scan_all(&self) -> Vec<Session> {
        let tmp_dir = gemini_dir().join("tmp");
        let mut sessions = Vec::new();

        // 遍历 tmp/<hash>/chats/ 目录
        let hash_dirs = match fs::read_dir(&tmp_dir) {
            Ok(e) => e,
            Err(_) => return sessions,
        };

        for hash_entry in hash_dirs.flatten() {
            let chats_dir = hash_entry.path().join("chats");
            if !chats_dir.exists() {
                continue;
            }

            // 用 hash 目录名做项目标识
            let hash_name = hash_entry
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let chat_files = match fs::read_dir(&chats_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for chat_entry in chat_files.flatten() {
                let path = chat_entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(session) = parse_gemini_session(&path, &hash_name) {
                        sessions.push(session);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}

fn parse_gemini_session(path: &Path, project_hash: &str) -> Option<Session> {
    let content = fs::read_to_string(path).ok()?;
    let data: GeminiSession = serde_json::from_str(&content).ok()?;

    let session_id = data.session_id.unwrap_or_default();
    if session_id.is_empty() {
        return None;
    }

    let start_time = data
        .start_time
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .unwrap_or_else(Utc::now);
    let last_updated = data
        .last_updated
        .and_then(|s| s.parse::<DateTime<Utc>>().ok())
        .unwrap_or(start_time);

    let mut user_messages = Vec::new();
    if let Some(messages) = &data.messages {
        for msg in messages {
            if msg.msg_type.as_deref() == Some("user") {
                if let Some(content) = &msg.content {
                    if !content.is_empty() {
                        user_messages.push(content.clone());
                    }
                }
            }
        }
    }

    if user_messages.is_empty() {
        return None;
    }

    // Gemini 用 project hash 做项目标识，缩短显示
    let short_hash = &project_hash[..8.min(project_hash.len())];

    Some(Session {
        session_id,
        provider: "gemini".to_string(),
        project_path: format!("gemini:{}", project_hash),
        project_name: format!("gemini-{}", short_hash),
        first_prompt: user_messages.first().cloned().unwrap_or_default(),
        last_prompt: user_messages.last().cloned().unwrap_or_default(),
        created_at: start_time,
        updated_at: last_updated,
        message_count: user_messages.len() as u32,
        user_messages,
        total_tokens: 0, // Gemini 会话文件中无 token 数据
    })
}
