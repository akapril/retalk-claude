use super::SessionProvider;
use crate::models::Session;
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub struct OpenCodeProvider;

/// 获取 OpenCode 数据库路径
fn opencode_db_path() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db")
}

impl SessionProvider for OpenCodeProvider {
    fn name(&self) -> &str {
        "opencode"
    }

    fn is_available(&self) -> bool {
        opencode_db_path().exists()
    }

    fn scan_all(&self) -> Vec<Session> {
        scan_sqlite_sessions(&opencode_db_path(), "opencode")
    }
}

/// 通用 SQLite 会话扫描（OpenCode / Kilo Code 共用表结构）
pub fn scan_sqlite_sessions(db_path: &Path, provider_name: &str) -> Vec<Session> {
    let conn = match Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();

    let mut stmt = match conn.prepare(
        "SELECT s.id, s.title, s.directory, s.time_created, s.time_updated, p.worktree
         FROM session s
         LEFT JOIN project p ON s.project_id = p.id
         ORDER BY s.time_updated DESC",
    ) {
        Ok(s) => s,
        Err(_) => return sessions,
    };

    let session_rows: Vec<(String, String, String, i64, i64, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1).unwrap_or_default(),
                row.get::<_, String>(2).unwrap_or_default(),
                row.get::<_, i64>(3).unwrap_or(0),
                row.get::<_, i64>(4).unwrap_or(0),
                row.get::<_, String>(5).unwrap_or_default(),
            ))
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    for (session_id, title, directory, time_created, time_updated, worktree) in session_rows {
        let user_messages = get_user_messages(&conn, &session_id);

        if user_messages.is_empty() {
            continue;
        }

        let project_path = if !directory.is_empty() && directory != "/" {
            directory
        } else if !worktree.is_empty() && worktree != "/" {
            worktree
        } else {
            continue;
        };

        let project_name = std::path::Path::new(&project_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let created_at = Utc
            .timestamp_opt(time_created / 1000, 0)
            .single()
            .unwrap_or_else(Utc::now);
        let updated_at = Utc
            .timestamp_opt(time_updated / 1000, 0)
            .single()
            .unwrap_or_else(Utc::now);

        sessions.push(Session {
            session_id,
            provider: provider_name.to_string(),
            project_path,
            project_name,
            first_prompt: user_messages
                .first()
                .cloned()
                .unwrap_or_else(|| title.clone()),
            last_prompt: user_messages.last().cloned().unwrap_or_else(|| title),
            created_at,
            updated_at,
            message_count: user_messages.len() as u32,
            user_messages,
            total_tokens: 0,
        });
    }

    sessions
}

/// 从 message + part 表中提取用户消息文本
fn get_user_messages(conn: &Connection, session_id: &str) -> Vec<String> {
    let mut msg_stmt = match conn.prepare(
        "SELECT id FROM message WHERE session_id = ?1 AND data LIKE '%\"role\":\"user\"%' ORDER BY time_created ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let msg_ids: Vec<String> = msg_stmt
        .query_map([session_id], |row| row.get::<_, String>(0))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    let mut part_stmt = match conn.prepare(
        "SELECT data FROM part WHERE message_id = ?1 AND data LIKE '%\"type\":\"text\"%'",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut messages = Vec::new();
    for msg_id in &msg_ids {
        let parts: Vec<String> = part_stmt
            .query_map([msg_id], |row| row.get::<_, String>(0))
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        let mut text_parts = Vec::new();
        for part_data in &parts {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(part_data) {
                if let Some(text) = val.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        text_parts.push(text.to_string());
                    }
                }
            }
        }
        if !text_parts.is_empty() {
            messages.push(text_parts.join("\n"));
        }
    }

    messages
}
