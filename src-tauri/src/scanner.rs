use crate::config::claude_dir;
use crate::models::{HistoryEntry, Session, SessionEntry};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// 扫描全部会话数据
pub fn scan_all_sessions() -> Vec<Session> {
    let claude = claude_dir();
    let history_path = claude.join("history.jsonl");
    let projects_dir = claude.join("projects");

    // 第一步：从 history.jsonl 建立 project_path -> session_id 映射
    let history_map = parse_history(&history_path);

    // 第二步：从 projects/ 目录下的 session 文件提取完整数据
    let mut sessions = Vec::new();
    if projects_dir.exists() {
        for project_entry in fs::read_dir(&projects_dir).into_iter().flatten() {
            let project_entry = match project_entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let project_dir = project_entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            // 从 history_map 找到此目录对应的原始路径
            let dir_name = project_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let original_path = history_map
                .get(&dir_name)
                .cloned()
                .unwrap_or_else(|| decode_project_dir(&dir_name));

            // 扫描此项目下的所有 session jsonl 文件
            for file_entry in fs::read_dir(&project_dir).into_iter().flatten() {
                let file_entry = match file_entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let file_path = file_entry.path();
                if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let session_id = file_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if let Some(session) =
                    parse_session_file(&file_path, &session_id, &original_path)
                {
                    sessions.push(session);
                }
            }
        }
    }

    // 按 updated_at 降序排序
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
}

/// 扫描单个 session 文件
pub fn scan_single_session(path: &Path) -> Option<Session> {
    let session_id = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let project_dir = path.parent()?;
    let dir_name = project_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let history_map = parse_history(&claude_dir().join("history.jsonl"));
    let original_path = history_map
        .get(&dir_name)
        .cloned()
        .unwrap_or_else(|| decode_project_dir(&dir_name));

    parse_session_file(path, &session_id, &original_path)
}

/// 解析 history.jsonl，建立 编码目录名 -> 原始路径 的映射
fn parse_history(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return map,
    };
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
            if let Some(project) = entry.project {
                let encoded = encode_project_path(&project);
                map.insert(encoded, project);
            }
        }
    }
    map
}

/// 解析单个 session JSONL 文件
fn parse_session_file(
    path: &Path,
    session_id: &str,
    project_path: &str,
) -> Option<Session> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut user_messages = Vec::new();
    let mut first_timestamp: Option<DateTime<Utc>> = None;
    let mut last_timestamp: Option<DateTime<Utc>> = None;
    let mut message_count: u32 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let entry: SessionEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.entry_type.as_deref() != Some("user") {
            continue;
        }

        // 提取时间戳
        if let Some(ts_str) = &entry.timestamp {
            if let Ok(ts) = ts_str.parse::<DateTime<Utc>>() {
                if first_timestamp.is_none() {
                    first_timestamp = Some(ts);
                }
                last_timestamp = Some(ts);
            }
        }

        // 提取用户消息文本
        if let Some(msg) = &entry.message {
            if msg.role.as_deref() == Some("user") {
                if let Some(content) = &msg.content {
                    let text = extract_text_content(content);
                    if !text.is_empty() {
                        user_messages.push(text);
                        message_count += 1;
                    }
                }
            }
        }
    }

    if user_messages.is_empty() {
        return None;
    }

    let project_name = Path::new(project_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Some(Session {
        session_id: session_id.to_string(),
        project_path: project_path.to_string(),
        project_name,
        first_prompt: user_messages.first().cloned().unwrap_or_default(),
        last_prompt: user_messages.last().cloned().unwrap_or_default(),
        created_at: first_timestamp.unwrap_or_else(Utc::now),
        updated_at: last_timestamp.unwrap_or_else(Utc::now),
        message_count,
        user_messages,
    })
}

/// 从 message content 中提取纯文本
fn extract_text_content(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            arr.iter()
                .filter_map(|item| {
                    if item.get("type")?.as_str()? == "text" {
                        item.get("text")?.as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => String::new(),
    }
}

/// 将原始路径编码为 projects/ 下的目录名
/// "D:\workspace\geo2" -> "D--workspace-geo2"
fn encode_project_path(path: &str) -> String {
    path.replace(":\\", "--").replace("\\", "-").replace("/", "-")
}

/// 尝试从编码目录名反推原始路径（兜底方案）
fn decode_project_dir(encoded: &str) -> String {
    if let Some(pos) = encoded.find("--") {
        let drive = &encoded[..pos];
        let rest = &encoded[pos + 2..];
        format!("{}:\\{}", drive, rest.replace("-", "\\"))
    } else {
        encoded.to_string()
    }
}
