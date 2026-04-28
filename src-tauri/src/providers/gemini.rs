use super::SessionProvider;
use crate::models::Session;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
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
    /// content 可能是字符串或 [{"text": "..."}] 数组
    content: Option<serde_json::Value>,
}

/// projects.json 结构: { "projects": { "d:\\workspace\\foo": "foo", ... } }
#[derive(serde::Deserialize)]
struct ProjectsFile {
    projects: Option<HashMap<String, String>>,
}

impl SessionProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn is_available(&self) -> bool {
        gemini_dir().join("tmp").exists()
    }

    fn scan_all(&self) -> Vec<Session> {
        let base = gemini_dir();
        let tmp_dir = base.join("tmp");
        let mut sessions = Vec::new();

        // 加载 projects.json 映射: 项目名 -> 真实路径
        let name_to_path = load_project_map(&base);

        let sub_dirs = match fs::read_dir(&tmp_dir) {
            Ok(e) => e,
            Err(_) => return sessions,
        };

        for entry in sub_dirs.flatten() {
            let entry_path = entry.path();
            if !entry_path.is_dir() {
                continue;
            }

            let chats_dir = entry_path.join("chats");
            if !chats_dir.exists() {
                continue;
            }

            // 目录名可能是 hash 或项目名
            let dir_name = entry_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // 尝试从 projects.json 映射中找到真实路径
            let (project_path, project_name) = resolve_project(&dir_name, &name_to_path);

            let chat_files = match fs::read_dir(&chats_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for chat_entry in chat_files.flatten() {
                let path = chat_entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(session) =
                        parse_gemini_session(&path, &project_path, &project_name)
                    {
                        sessions.push(session);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}

/// 从 content 字段提取文本（兼容字符串和 [{"text":"..."}] 数组两种格式）
fn extract_content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            arr.iter()
                .filter_map(|item| item.get("text")?.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => String::new(),
    }
}

/// 加载 projects.json，构建 项目名/hash -> 真实路径 的双向映射
fn load_project_map(gemini_base: &Path) -> HashMap<String, String> {
    let projects_path = gemini_base.join("projects.json");
    let content = match fs::read_to_string(&projects_path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let data: ProjectsFile = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };

    let mut map = HashMap::new();
    if let Some(projects) = data.projects {
        for (real_path, short_name) in &projects {
            // 项目名 -> 真实路径
            map.insert(short_name.clone(), real_path.clone());
        }
    }
    map
}

/// 根据目录名（hash 或项目名）解析出项目路径和项目名
fn resolve_project(dir_name: &str, name_to_path: &HashMap<String, String>) -> (String, String) {
    // 先尝试作为项目名在映射中查找
    if let Some(real_path) = name_to_path.get(dir_name) {
        let project_name = Path::new(real_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        return (real_path.clone(), project_name);
    }

    // 可能是旧版 hash 格式 — 在映射的值（项目名）中找不到，用 hash 缩写
    let is_hash = dir_name.len() > 16 && dir_name.chars().all(|c| c.is_ascii_hexdigit());
    if is_hash {
        let short = &dir_name[..8];
        (
            format!("gemini:{}", dir_name),
            format!("gemini-{}", short),
        )
    } else {
        // 非 hash 也不在映射中 — 直接用目录名做项目名
        (dir_name.to_string(), dir_name.to_string())
    }
}

fn parse_gemini_session(
    path: &Path,
    project_path: &str,
    project_name: &str,
) -> Option<Session> {
    let raw = fs::read_to_string(path).ok()?;

    // Gemini 的 JSON 可能包含特殊字符导致解析失败，尝试容错
    let data: GeminiSession = match serde_json::from_str(&raw) {
        Ok(d) => d,
        Err(_) => {
            // 尝试清理无效 UTF-8 再解析
            let cleaned: String = raw.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t').collect();
            match serde_json::from_str(&cleaned) {
                Ok(d) => d,
                Err(_) => return None,
            }
        }
    };

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
                    let text = extract_content_text(content);
                    if !text.is_empty() {
                        user_messages.push(text);
                    }
                }
            }
        }
    }

    if user_messages.is_empty() {
        return None;
    }

    Some(Session {
        session_id,
        provider: "gemini".to_string(),
        project_path: project_path.to_string(),
        project_name: project_name.to_string(),
        first_prompt: user_messages.first().cloned().unwrap_or_default(),
        last_prompt: user_messages.last().cloned().unwrap_or_default(),
        created_at: start_time,
        updated_at: last_updated,
        message_count: user_messages.len() as u32,
        user_messages,
        total_tokens: 0,
    })
}
