use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AI 编码工具会话（支持多 provider）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub provider: String, // "claude" / "codex" / "gemini" / "continue"
    pub project_path: String,
    pub project_name: String,
    pub first_prompt: String,
    pub last_prompt: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: u32,
    pub user_messages: Vec<String>,
    pub total_tokens: u64, // 总 token 数（从会话文件中提取）
}

/// history.jsonl 中的单行记录
#[derive(Debug, Deserialize)]
pub struct HistoryEntry {
    pub display: Option<String>,
    pub timestamp: Option<u64>,
    pub project: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

/// 会话 JSONL 中的单行记录
#[derive(Debug, Deserialize)]
pub struct SessionEntry {
    #[serde(rename = "type")]
    pub entry_type: Option<String>,
    pub message: Option<MessageContent>,
    pub timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageContent {
    pub role: Option<String>,
    pub content: Option<serde_json::Value>,
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub terminal: TerminalConfig,
    pub update: UpdateConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub hotkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub preferred: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub watcher_enabled: bool,
    pub poll_enabled: bool,
    pub poll_interval_secs: u64,
    pub on_demand_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub max_results: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                hotkey: "Ctrl+Shift+C".to_string(),
            },
            terminal: TerminalConfig {
                preferred: "auto".to_string(),
            },
            update: UpdateConfig {
                watcher_enabled: true,
                poll_enabled: true,
                poll_interval_secs: 30,
                on_demand_enabled: true,
            },
            ui: UiConfig {
                theme: "dark".to_string(),
                max_results: 1000,
            },
        }
    }
}

/// Git 仓库信息
#[derive(Debug, Clone, Serialize)]
pub struct GitInfo {
    pub branch: String,
    pub dirty_count: u32,
}

/// 会话标签存储：session_id -> [tag1, tag2, ...]
pub type TagsMap = HashMap<String, Vec<String>>;

/// Provider 可用状态信息（Feature 1: 空状态引导）
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub available: bool,
}

/// 更新策略性能统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStats {
    pub strategy: String,
    pub trigger_count: u64,
    pub total_time_ms: u64,
    pub avg_time_ms: f64,
    pub last_trigger: Option<DateTime<Utc>>,
    pub files_processed: u64,
}

impl UpdateStats {
    pub fn new(strategy: &str) -> Self {
        Self {
            strategy: strategy.to_string(),
            trigger_count: 0,
            total_time_ms: 0,
            avg_time_ms: 0.0,
            last_trigger: None,
            files_processed: 0,
        }
    }

    pub fn record(&mut self, duration_ms: u64, files: u64) {
        self.trigger_count += 1;
        self.total_time_ms += duration_ms;
        self.files_processed += files;
        self.avg_time_ms = self.total_time_ms as f64 / self.trigger_count as f64;
        self.last_trigger = Some(Utc::now());
    }
}
