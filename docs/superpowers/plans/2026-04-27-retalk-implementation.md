# Retalk Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Windows system tray app that indexes Claude Code session history and provides a Spotlight-style popup for searching and resuming conversations.

**Architecture:** Tauri v2 app with Rust backend (scanner, tantivy indexer, searcher, updater, terminal launcher) exposed via IPC commands to a vanilla HTML/CSS/JS frontend. Core logic lives in a reusable lib, `commands.rs` is thin glue.

**Tech Stack:** Rust 1.89, Tauri v2, Tantivy, jieba-rs, notify, vanilla HTML/CSS/JS

---

## File Map

```
retalk-claude/
├── src-tauri/
│   ├── Cargo.toml                # 依赖声明
│   ├── tauri.conf.json           # Tauri v2 配置
│   ├── capabilities/
│   │   └── default.json          # 权限声明
│   ├── icons/
│   │   └── icon.ico              # 托盘图标
│   ├── build.rs                  # Tauri 构建脚本
│   └── src/
│       ├── main.rs               # 入口
│       ├── lib.rs                # 核心库导出 + Tauri setup
│       ├── models.rs             # Session, Config, UpdateStats 数据结构
│       ├── config.rs             # 配置读写 (~/.claude/retalk/config.toml)
│       ├── scanner.rs            # JSONL 解析，会话数据提取
│       ├── indexer.rs            # Tantivy 索引管理（建索引、增量更新）
│       ├── searcher.rs           # 搜索查询（全文搜索 + 列表）
│       ├── updater.rs            # 三种更新策略（watcher/poll/on_demand）
│       ├── terminal.rs           # 终端检测与启动
│       └── commands.rs           # Tauri IPC 命令
├── src/                          # 前端
│   ├── index.html
│   ├── style.css
│   └── main.js
└── docs/
```

---

### Task 1: 项目脚手架与依赖配置

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `src/index.html`
- Create: `package.json`

- [ ] **Step 1: 安装 Tauri CLI**

```bash
cargo install tauri-cli --version "^2.0.0" --locked
```

Expected: `Installed package `tauri-cli v2.x.x``

- [ ] **Step 2: 创建 package.json**

```json
{
  "name": "retalk",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "tauri": "cargo tauri"
  }
}
```

- [ ] **Step 3: 创建 src-tauri/Cargo.toml**

```toml
[package]
name = "retalk"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-global-shortcut = "2"
tauri-plugin-clipboard-manager = "2"
tauri-plugin-shell = "2"
tantivy = "0.22"
jieba-rs = "0.7"
notify = "7"
notify-debouncer-mini = "0.5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
chrono = { version = "0.4", features = ["serde"] }
dirs = "6"
parking_lot = "0.12"

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

- [ ] **Step 4: 创建 src-tauri/build.rs**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 5: 创建 src-tauri/tauri.conf.json**

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-config-schema/schema.json",
  "productName": "retalk",
  "version": "0.1.0",
  "identifier": "com.retalk.app",
  "build": {
    "frontendDist": "../src"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "label": "main",
        "title": "retalk",
        "width": 600,
        "height": 500,
        "resizable": false,
        "decorations": false,
        "transparent": true,
        "visible": false,
        "center": true,
        "alwaysOnTop": true,
        "skipTaskbar": true
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": [
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 6: 创建 src-tauri/capabilities/default.json**

```json
{
  "identifier": "main-capability",
  "description": "Main window permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:default",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-set-focus",
    "core:window:allow-is-visible",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister",
    "global-shortcut:allow-is-registered",
    "clipboard-manager:allow-write-text",
    "shell:allow-execute",
    "shell:allow-open",
    "shell:allow-spawn"
  ]
}
```

- [ ] **Step 7: 创建最小 src-tauri/src/main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    retalk_lib::run();
}
```

- [ ] **Step 8: 创建最小 src-tauri/src/lib.rs**

```rust
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("启动 retalk 失败");
}
```

- [ ] **Step 9: 创建最小前端 src/index.html**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>retalk</title>
</head>
<body>
  <h1>retalk</h1>
</body>
</html>
```

- [ ] **Step 10: 创建托盘图标**

使用 ImageMagick 生成一个简单的占位图标：

```bash
magick -size 256x256 xc:"#4A90D9" -fill white -font Arial -pointsize 120 -gravity center -annotate 0 "R" src-tauri/icons/icon.ico
```

如果没有 ImageMagick，手动放置任意 .ico 文件到 `src-tauri/icons/icon.ico`。

- [ ] **Step 11: 验证构建**

```bash
cd D:/workspace/retalk-claude && cargo tauri dev
```

Expected: 窗口弹出显示 "retalk"，无编译错误。

- [ ] **Step 12: 提交**

```bash
git add src-tauri/ src/ package.json
git commit -m "feat: 项目脚手架，Tauri v2 最小可运行骨架"
```

---

### Task 2: 数据模型 (models.rs)

**Files:**
- Create: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 models.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Claude Code 会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub project_path: String,
    pub project_name: String,
    pub first_prompt: String,
    pub last_prompt: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: u32,
    pub user_messages: Vec<String>,
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
                max_results: 50,
            },
        }
    }
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
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

在 `src-tauri/src/lib.rs` 顶部添加：

```rust
mod models;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

Expected: 编译通过，无错误。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/models.rs src-tauri/src/lib.rs
git commit -m "feat: 数据模型定义（Session, Config, UpdateStats）"
```

---

### Task 3: 配置模块 (config.rs)

**Files:**
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 config.rs**

```rust
use crate::models::AppConfig;
use std::fs;
use std::path::PathBuf;

/// 获取 retalk 数据目录：~/.claude/retalk/
pub fn retalk_dir() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".claude")
        .join("retalk")
}

/// 获取 Claude Code 数据目录：~/.claude/
pub fn claude_dir() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".claude")
}

/// 配置文件路径
fn config_path() -> PathBuf {
    retalk_dir().join("config.toml")
}

/// 加载配置，不存在则创建默认配置
pub fn load_config() -> AppConfig {
    let path = config_path();
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        toml::from_str(&content).unwrap_or_default()
    } else {
        let config = AppConfig::default();
        save_config(&config);
        config
    }
}

/// 保存配置到文件
pub fn save_config(config: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = toml::to_string_pretty(config).expect("序列化配置失败");
    let _ = fs::write(&path, content);
}
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

```rust
mod config;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat: 配置模块，支持加载/保存 config.toml"
```

---

### Task 4: 扫描器模块 (scanner.rs)

**Files:**
- Create: `src-tauri/src/scanner.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 scanner.rs**

```rust
use crate::config::claude_dir;
use crate::models::{HistoryEntry, MessageContent, Session, SessionEntry};
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

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
    // 简单反推：第一个 -- 还原为 :\，其余 - 还原为 \
    // 注意：项目名含 - 时不准确，所以优先用 history_map
    if let Some(pos) = encoded.find("--") {
        let drive = &encoded[..pos];
        let rest = &encoded[pos + 2..];
        format!("{}:\\{}", drive, rest.replace("-", "\\"))
    } else {
        encoded.to_string()
    }
}
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

```rust
mod scanner;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/scanner.rs src-tauri/src/lib.rs
git commit -m "feat: 扫描器模块，解析 history.jsonl 和 session 文件"
```

---

### Task 5: 索引模块 (indexer.rs)

**Files:**
- Create: `src-tauri/src/indexer.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 indexer.rs**

```rust
use crate::config::retalk_dir;
use crate::models::Session;
use jieba_rs::Jieba;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::tokenizer::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};

/// Tantivy 索引管理器
pub struct SessionIndex {
    index: Index,
    reader: IndexReader,
    schema: Schema,
    jieba: Arc<Jieba>,
}

/// jieba 分词器适配 tantivy
#[derive(Clone)]
struct JiebaTokenizer {
    jieba: Arc<Jieba>,
}

impl Tokenizer for JiebaTokenizer {
    type TokenStream<'a> = JiebaTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let words = self.jieba.cut(text, true);
        let mut tokens = Vec::new();
        let mut offset = 0;
        for word in words {
            let word = word.trim();
            if word.is_empty() {
                offset += word.len();
                continue;
            }
            tokens.push(Token {
                offset_from: offset,
                offset_to: offset + word.len(),
                position: tokens.len(),
                text: word.to_lowercase(),
                position_length: 1,
            });
            offset += word.len();
        }
        JiebaTokenStream {
            tokens,
            index: 0,
        }
    }
}

struct JiebaTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for JiebaTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

impl SessionIndex {
    /// 创建或打开索引
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let index_dir = retalk_dir().join("index");
        std::fs::create_dir_all(&index_dir)?;

        let jieba = Arc::new(Jieba::new());

        // 定义 schema
        let mut schema_builder = Schema::builder();
        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        let text_indexed_only = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("jieba")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            );

        schema_builder.add_text_field("session_id", STRING | STORED);
        schema_builder.add_text_field("project_path", STRING | STORED);
        schema_builder.add_text_field("project_name", text_options.clone());
        schema_builder.add_text_field("first_prompt", text_options.clone());
        schema_builder.add_text_field("last_prompt", text_options.clone());
        schema_builder.add_text_field("content", text_indexed_only);
        schema_builder.add_date_field("updated_at", INDEXED | STORED | FAST);
        schema_builder.add_u64_field("message_count", STORED);

        let schema = schema_builder.build();

        let dir = MmapDirectory::open(&index_dir)?;
        let index = Index::open_or_create(dir, schema.clone())?;

        // 注册 jieba 分词器
        let tokenizer = JiebaTokenizer {
            jieba: Arc::clone(&jieba),
        };
        index
            .tokenizers()
            .register("jieba", tokenizer);

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            reader,
            schema,
            jieba,
        })
    }

    /// 全量重建索引
    pub fn rebuild(&self, sessions: &[Session]) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        writer.delete_all_documents()?;

        for session in sessions {
            self.add_session_to_writer(&mut writer, session)?;
        }

        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// 增量更新单个 session
    pub fn upsert_session(&self, session: &Session) -> Result<(), Box<dyn std::error::Error>> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        // 先删除旧文档
        let session_id_field = self.schema.get_field("session_id").unwrap();
        let term = tantivy::Term::from_field_text(session_id_field, &session.session_id);
        writer.delete_term(term);

        self.add_session_to_writer(&mut writer, session)?;
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    fn add_session_to_writer(
        &self,
        writer: &mut IndexWriter,
        session: &Session,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session_id = self.schema.get_field("session_id").unwrap();
        let project_path = self.schema.get_field("project_path").unwrap();
        let project_name = self.schema.get_field("project_name").unwrap();
        let first_prompt = self.schema.get_field("first_prompt").unwrap();
        let last_prompt = self.schema.get_field("last_prompt").unwrap();
        let content = self.schema.get_field("content").unwrap();
        let updated_at = self.schema.get_field("updated_at").unwrap();
        let message_count = self.schema.get_field("message_count").unwrap();

        let all_content = session.user_messages.join("\n");
        let date_val = tantivy::DateTime::from_timestamp_micros(
            session.updated_at.timestamp_micros(),
        );

        writer.add_document(doc!(
            session_id => session.session_id.as_str(),
            project_path => session.project_path.as_str(),
            project_name => session.project_name.as_str(),
            first_prompt => session.first_prompt.as_str(),
            last_prompt => session.last_prompt.as_str(),
            content => all_content.as_str(),
            updated_at => date_val,
            message_count => session.message_count as u64,
        ))?;

        Ok(())
    }

    /// 获取 index 和 reader 的引用（供 searcher 使用）
    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn reader(&self) -> &IndexReader {
        &self.reader
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }
}
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

```rust
mod indexer;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

Expected: 编译通过。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/indexer.rs src-tauri/src/lib.rs
git commit -m "feat: Tantivy 索引模块，支持 jieba 中文分词"
```

---

### Task 6: 搜索模块 (searcher.rs)

**Files:**
- Create: `src-tauri/src/searcher.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 searcher.rs**

```rust
use crate::indexer::SessionIndex;
use crate::models::Session;
use chrono::{DateTime, Utc};
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, QueryParser};
use tantivy::Order;

/// 搜索结果条目（从索引还原的轻量 Session）
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub session_id: String,
    pub project_path: String,
    pub project_name: String,
    pub first_prompt: String,
    pub last_prompt: String,
    pub updated_at: String,
    pub message_count: u64,
    pub score: f32,
}

/// 全文搜索
pub fn search(
    index: &SessionIndex,
    query_str: &str,
    max_results: usize,
) -> Vec<SearchResult> {
    let searcher = index.reader().searcher();
    let schema = index.schema();

    let project_name = schema.get_field("project_name").unwrap();
    let first_prompt = schema.get_field("first_prompt").unwrap();
    let last_prompt = schema.get_field("last_prompt").unwrap();
    let content = schema.get_field("content").unwrap();

    let query_parser = QueryParser::for_index(
        index.index(),
        vec![project_name, first_prompt, last_prompt, content],
    );

    let query = match query_parser.parse_query(query_str) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };

    let top_docs = match searcher.search(&query, &TopDocs::with_limit(max_results)) {
        Ok(docs) => docs,
        Err(_) => return Vec::new(),
    };

    extract_results(&searcher, schema, &top_docs)
}

/// 列出所有会话（按时间排序）
pub fn list_all(
    index: &SessionIndex,
    max_results: usize,
) -> Vec<SearchResult> {
    let searcher = index.reader().searcher();
    let schema = index.schema();
    let updated_at_field = schema.get_field("updated_at").unwrap();

    let collector = TopDocs::with_limit(max_results)
        .order_by_fast_field::<tantivy::DateTime>(updated_at_field, Order::Desc);

    let top_docs = match searcher.search(&AllQuery, &collector) {
        Ok(docs) => docs,
        Err(_) => return Vec::new(),
    };

    // AllQuery + order_by 返回 (tantivy::DateTime, DocAddress)
    let results: Vec<(f32, tantivy::DocAddress)> = top_docs
        .into_iter()
        .map(|(_date, addr)| (0.0f32, addr))
        .collect();

    extract_results(&searcher, schema, &results)
}

fn extract_results(
    searcher: &tantivy::Searcher,
    schema: &tantivy::schema::Schema,
    docs: &[(f32, tantivy::DocAddress)],
) -> Vec<SearchResult> {
    let session_id_field = schema.get_field("session_id").unwrap();
    let project_path_field = schema.get_field("project_path").unwrap();
    let project_name_field = schema.get_field("project_name").unwrap();
    let first_prompt_field = schema.get_field("first_prompt").unwrap();
    let last_prompt_field = schema.get_field("last_prompt").unwrap();
    let updated_at_field = schema.get_field("updated_at").unwrap();
    let message_count_field = schema.get_field("message_count").unwrap();

    let mut results = Vec::new();
    for (score, doc_addr) in docs {
        let doc = match searcher.doc(*doc_addr) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let get_text = |field| -> String {
            doc.get_first(field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let updated_str = doc
            .get_first(updated_at_field)
            .and_then(|v| v.as_datetime())
            .map(|dt| {
                let ts = dt.into_timestamp_micros();
                DateTime::from_timestamp_micros(ts)
                    .unwrap_or_default()
                    .format("%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_default();

        let msg_count = doc
            .get_first(message_count_field)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        results.push(SearchResult {
            session_id: get_text(session_id_field),
            project_path: get_text(project_path_field),
            project_name: get_text(project_name_field),
            first_prompt: get_text(first_prompt_field),
            last_prompt: get_text(last_prompt_field),
            updated_at: updated_str,
            message_count: msg_count,
            score: *score,
        });
    }
    results
}
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

```rust
mod searcher;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/searcher.rs src-tauri/src/lib.rs
git commit -m "feat: 搜索模块，支持全文搜索和列表查询"
```

---

### Task 7: 更新策略模块 (updater.rs)

**Files:**
- Create: `src-tauri/src/updater.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 updater.rs**

```rust
use crate::config::claude_dir;
use crate::indexer::SessionIndex;
use crate::models::{AppConfig, UpdateStats};
use crate::scanner;
use notify::{Event, RecursiveMode, Watcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub struct Updater {
    stats: Arc<Mutex<Vec<UpdateStats>>>,
    last_history_mtime: Arc<Mutex<Option<SystemTime>>>,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(vec![
                UpdateStats::new("watcher"),
                UpdateStats::new("poll"),
                UpdateStats::new("on_demand"),
            ])),
            last_history_mtime: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_stats(&self) -> Vec<UpdateStats> {
        self.stats.lock().clone()
    }

    /// 策略 1：文件系统监听
    pub fn start_watcher(
        &self,
        index: Arc<Mutex<SessionIndex>>,
        config: &AppConfig,
    ) {
        if !config.update.watcher_enabled {
            return;
        }

        let stats = Arc::clone(&self.stats);
        let claude = claude_dir();

        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut debouncer = match new_debouncer(Duration::from_millis(500), tx) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("文件监听器启动失败: {}", e);
                    return;
                }
            };

            let history_path = claude.join("history.jsonl");
            let projects_path = claude.join("projects");

            // 监听 history.jsonl 所在目录和 projects 目录
            let _ = debouncer.watcher().watch(&claude, RecursiveMode::NonRecursive);
            if projects_path.exists() {
                let _ = debouncer.watcher().watch(&projects_path, RecursiveMode::Recursive);
            }

            loop {
                match rx.recv() {
                    Ok(Ok(events)) => {
                        let start = Instant::now();
                        let mut files_count = 0u64;

                        for event in &events {
                            let path = &event.path;
                            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                                // 增量更新：扫描变更的 session 文件
                                if path.file_name().and_then(|n| n.to_str())
                                    == Some("history.jsonl")
                                {
                                    // history 变更 -> 全量重扫
                                    let sessions = scanner::scan_all_sessions();
                                    if let Ok(idx) = index.lock().rebuild(&sessions) {
                                        // 已重建
                                    }
                                    files_count += 1;
                                } else if let Some(session) =
                                    scanner::scan_single_session(path)
                                {
                                    let _ = index.lock().upsert_session(&session);
                                    files_count += 1;
                                }
                            }
                        }

                        let elapsed = start.elapsed().as_millis() as u64;
                        stats.lock()[0].record(elapsed, files_count);
                    }
                    Ok(Err(e)) => eprintln!("监听错误: {:?}", e),
                    Err(_) => break,
                }
            }
        });
    }

    /// 策略 2：定时轮询
    pub fn start_poll(
        &self,
        index: Arc<Mutex<SessionIndex>>,
        config: &AppConfig,
    ) {
        if !config.update.poll_enabled {
            return;
        }

        let interval = Duration::from_secs(config.update.poll_interval_secs);
        let stats = Arc::clone(&self.stats);
        let last_mtime = Arc::clone(&self.last_history_mtime);

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);

                let start = Instant::now();
                let history_path = claude_dir().join("history.jsonl");

                // 检查 mtime 是否变化
                let current_mtime = std::fs::metadata(&history_path)
                    .and_then(|m| m.modified())
                    .ok();

                let should_update = {
                    let mut last = last_mtime.lock();
                    if *last != current_mtime {
                        *last = current_mtime;
                        true
                    } else {
                        false
                    }
                };

                if should_update {
                    let sessions = scanner::scan_all_sessions();
                    let _ = index.lock().rebuild(&sessions);
                    let elapsed = start.elapsed().as_millis() as u64;
                    stats.lock()[1].record(elapsed, sessions.len() as u64);
                }
            }
        });
    }

    /// 策略 3：按需扫描（弹窗打开时调用）
    pub fn on_demand_refresh(
        &self,
        index: &SessionIndex,
        config: &AppConfig,
    ) -> bool {
        if !config.update.on_demand_enabled {
            return false;
        }

        let start = Instant::now();
        let history_path = claude_dir().join("history.jsonl");

        let current_mtime = std::fs::metadata(&history_path)
            .and_then(|m| m.modified())
            .ok();

        let should_update = {
            let mut last = self.last_history_mtime.lock();
            if *last != current_mtime {
                *last = current_mtime;
                true
            } else {
                false
            }
        };

        if should_update {
            let sessions = scanner::scan_all_sessions();
            let _ = index.rebuild(&sessions);
            let elapsed = start.elapsed().as_millis() as u64;
            self.stats.lock()[2].record(elapsed, sessions.len() as u64);
            true
        } else {
            false
        }
    }
}
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

```rust
mod updater;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/updater.rs src-tauri/src/lib.rs
git commit -m "feat: 三种更新策略（watcher/poll/on_demand）+ 性能统计"
```

---

### Task 8: 终端启动模块 (terminal.rs)

**Files:**
- Create: `src-tauri/src/terminal.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 terminal.rs**

```rust
use std::process::Command;

/// 终端类型
#[derive(Debug, Clone)]
pub enum TerminalKind {
    WindowsTerminal,
    PowerShell,
    Cmd,
}

/// 检测可用终端
pub fn detect_terminal(preferred: &str) -> TerminalKind {
    match preferred {
        "wt" => TerminalKind::WindowsTerminal,
        "pwsh" => TerminalKind::PowerShell,
        "cmd" => TerminalKind::Cmd,
        _ => auto_detect(),
    }
}

fn auto_detect() -> TerminalKind {
    // 检查 wt.exe 是否在 PATH 中
    if Command::new("where")
        .arg("wt.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return TerminalKind::WindowsTerminal;
    }

    // 检查 pwsh.exe
    if Command::new("where")
        .arg("pwsh.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return TerminalKind::PowerShell;
    }

    TerminalKind::Cmd
}

/// 构建恢复命令字符串
pub fn build_resume_command(project_path: &str, session_id: &str) -> String {
    format!(
        "cd \"{}\" && claude --resume {}",
        project_path, session_id
    )
}

/// 在新终端中恢复会话
pub fn resume_in_terminal(
    terminal: &TerminalKind,
    project_path: &str,
    session_id: &str,
) -> Result<(), String> {
    let project_name = std::path::Path::new(project_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let result = match terminal {
        TerminalKind::WindowsTerminal => Command::new("wt.exe")
            .args([
                "new-tab",
                "--title",
                &format!("retalk: {}", project_name),
                "-d",
                project_path,
                "cmd",
                "/k",
                "claude",
                "--resume",
                session_id,
            ])
            .spawn(),

        TerminalKind::PowerShell => Command::new("pwsh.exe")
            .args([
                "-NoExit",
                "-Command",
                &format!(
                    "Set-Location '{}'; claude --resume {}",
                    project_path, session_id
                ),
            ])
            .spawn(),

        TerminalKind::Cmd => Command::new("cmd.exe")
            .args([
                "/k",
                &format!(
                    "cd /d \"{}\" && claude --resume {}",
                    project_path, session_id
                ),
            ])
            .spawn(),
    };

    result
        .map(|_| ())
        .map_err(|e| format!("启动终端失败: {}", e))
}
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

```rust
mod terminal;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/terminal.rs src-tauri/src/lib.rs
git commit -m "feat: 终端检测与启动模块，支持 WT/PowerShell/Cmd"
```

---

### Task 9: IPC 命令层 (commands.rs)

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 commands.rs**

```rust
use crate::config;
use crate::indexer::SessionIndex;
use crate::models::{AppConfig, UpdateStats};
use crate::searcher::{self, SearchResult};
use crate::terminal;
use crate::updater::Updater;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::State;

/// 应用全局状态
pub struct AppState {
    pub index: Arc<Mutex<SessionIndex>>,
    pub updater: Arc<Updater>,
    pub config: Arc<Mutex<AppConfig>>,
}

#[tauri::command]
pub fn search(
    state: State<AppState>,
    query: String,
) -> Vec<SearchResult> {
    let index = state.index.lock();
    let max = state.config.lock().ui.max_results;
    searcher::search(&index, &query, max)
}

#[tauri::command]
pub fn list_sessions(
    state: State<AppState>,
) -> Vec<SearchResult> {
    let config = state.config.lock().clone();

    // 按需刷新
    {
        let index = state.index.lock();
        state.updater.on_demand_refresh(&index, &config);
    }

    let index = state.index.lock();
    searcher::list_all(&index, config.ui.max_results)
}

#[tauri::command]
pub fn resume_session(
    state: State<AppState>,
    session_id: String,
    project_path: String,
) -> Result<(), String> {
    let config = state.config.lock();
    let term = terminal::detect_terminal(&config.terminal.preferred);
    terminal::resume_in_terminal(&term, &project_path, &session_id)
}

#[tauri::command]
pub fn copy_command(
    session_id: String,
    project_path: String,
) -> String {
    terminal::build_resume_command(&project_path, &session_id)
}

#[tauri::command]
pub fn get_stats(state: State<AppState>) -> Vec<UpdateStats> {
    state.updater.get_stats()
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().clone()
}

#[tauri::command]
pub fn save_config(
    state: State<AppState>,
    new_config: AppConfig,
) {
    config::save_config(&new_config);
    *state.config.lock() = new_config;
}
```

- [ ] **Step 2: 在 lib.rs 中声明模块**

```rust
mod commands;
```

- [ ] **Step 3: 验证编译**

```bash
cd D:/workspace/retalk-claude/src-tauri && cargo check
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: Tauri IPC 命令层，连接前后端"
```

---

### Task 10: Tauri 主入口集成 (lib.rs 完整版)

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: 编写完整的 lib.rs**

```rust
mod commands;
mod config;
mod indexer;
mod models;
mod scanner;
mod searcher;
mod terminal;
mod updater;

use commands::AppState;
use indexer::SessionIndex;
use models::AppConfig;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// 切换窗口显隐
fn toggle_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

pub fn run() {
    // 加载配置
    let app_config = config::load_config();

    // 初始化索引
    let index = SessionIndex::new().expect("Tantivy 索引初始化失败");

    // 全量扫描并建立索引
    let sessions = scanner::scan_all_sessions();
    let _ = index.rebuild(&sessions);

    let index = Arc::new(Mutex::new(index));
    let updater = Arc::new(updater::Updater::new());

    // 启动后台更新策略
    updater.start_watcher(Arc::clone(&index), &app_config);
    updater.start_poll(Arc::clone(&index), &app_config);

    let state = AppState {
        index,
        updater,
        config: Arc::new(Mutex::new(app_config)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(|app| {
            // === 系统托盘 ===
            let show_i = MenuItem::with_id(app, "show", "显示/隐藏", true, None::<&str>)?;
            let stats_i = MenuItem::with_id(app, "stats", "性能统计", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &stats_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("retalk - Claude Code 会话管理")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_window(app),
                    "stats" => {
                        // TODO: 后续可弹出统计面板
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // === 全局快捷键 Ctrl+Shift+C ===
            let hotkey = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::KeyC,
            );

            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |app, shortcut, event| {
                        if shortcut == &hotkey && event.state() == ShortcutState::Pressed {
                            toggle_window(app);
                        }
                    })
                    .build(),
            )?;

            app.global_shortcut().register(hotkey)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::list_sessions,
            commands::resume_session,
            commands::copy_command,
            commands::get_stats,
            commands::get_config,
            commands::save_config,
        ])
        .run(tauri::generate_context!())
        .expect("启动 retalk 失败");
}
```

- [ ] **Step 2: 确认 main.rs 内容**

`src-tauri/src/main.rs` 保持不变：

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    retalk_lib::run();
}
```

注意：Cargo.toml 中的 `[lib]` 需要设置 name：

在 `src-tauri/Cargo.toml` 的 `[package]` 之后添加：

```toml
[lib]
name = "retalk_lib"
crate-type = ["lib"]
```

- [ ] **Step 3: 验证完整编译**

```bash
cd D:/workspace/retalk-claude && cargo tauri dev
```

Expected: 应用启动，系统托盘出现图标，Ctrl+Shift+C 弹出空白窗口。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/lib.rs src-tauri/src/main.rs src-tauri/Cargo.toml
git commit -m "feat: Tauri 主入口集成，托盘 + 快捷键 + IPC 注册"
```

---

### Task 11: 前端 — Spotlight 弹窗 UI

**Files:**
- Modify: `src/index.html`
- Create: `src/style.css`
- Create: `src/main.js`

- [ ] **Step 1: 编写 src/index.html**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>retalk</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <div id="app">
    <div class="search-bar">
      <input
        type="text"
        id="search-input"
        placeholder="搜索会话..."
        autocomplete="off"
        spellcheck="false"
      />
      <select id="view-mode">
        <option value="timeline">时间线</option>
        <option value="project">按项目</option>
      </select>
    </div>
    <div id="session-list" class="session-list"></div>
    <div class="status-bar">
      <span>↑↓ 导航</span>
      <span>Enter 恢复</span>
      <span>Ctrl+C 复制命令</span>
      <span>Esc 关闭</span>
    </div>
  </div>
  <script src="main.js"></script>
</body>
</html>
```

- [ ] **Step 2: 编写 src/style.css**

```css
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

:root {
  --bg: rgba(30, 30, 30, 0.95);
  --bg-item: rgba(255, 255, 255, 0.05);
  --bg-item-hover: rgba(255, 255, 255, 0.1);
  --bg-item-selected: rgba(74, 144, 217, 0.3);
  --text: #e0e0e0;
  --text-dim: #888;
  --text-highlight: #4A90D9;
  --border: rgba(255, 255, 255, 0.1);
  --radius: 12px;
}

html, body {
  background: transparent;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 14px;
  color: var(--text);
  overflow: hidden;
  user-select: none;
}

#app {
  background: var(--bg);
  border-radius: var(--radius);
  border: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  height: 100vh;
  backdrop-filter: blur(20px);
}

/* 搜索栏 */
.search-bar {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  gap: 8px;
}

#search-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: var(--text);
  font-size: 16px;
  font-family: inherit;
}

#search-input::placeholder {
  color: var(--text-dim);
}

#view-mode {
  background: var(--bg-item);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text);
  padding: 4px 8px;
  font-size: 12px;
  cursor: pointer;
  outline: none;
}

/* 会话列表 */
.session-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.session-list::-webkit-scrollbar {
  width: 4px;
}

.session-list::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 2px;
}

.session-item {
  padding: 10px 16px;
  cursor: pointer;
  border-bottom: 1px solid rgba(255, 255, 255, 0.03);
}

.session-item:hover {
  background: var(--bg-item-hover);
}

.session-item.selected {
  background: var(--bg-item-selected);
}

.session-item .header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}

.session-item .project-name {
  font-weight: 600;
  color: var(--text-highlight);
  font-size: 13px;
}

.session-item .time {
  color: var(--text-dim);
  font-size: 12px;
}

.session-item .prompt {
  color: var(--text-dim);
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.session-item .prompt mark {
  background: rgba(74, 144, 217, 0.4);
  color: var(--text);
  border-radius: 2px;
  padding: 0 1px;
}

/* 项目分组标题 */
.group-header {
  padding: 8px 16px 4px;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-dim);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

/* 状态栏 */
.status-bar {
  display: flex;
  justify-content: center;
  gap: 16px;
  padding: 8px;
  border-top: 1px solid var(--border);
  font-size: 11px;
  color: var(--text-dim);
}

/* 空状态 */
.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-dim);
  font-size: 14px;
}
```

- [ ] **Step 3: 编写 src/main.js**

```javascript
const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

let sessions = [];
let selectedIndex = 0;
let currentQuery = "";
let viewMode = "timeline"; // "timeline" | "project"

const searchInput = document.getElementById("search-input");
const sessionList = document.getElementById("session-list");
const viewModeSelect = document.getElementById("view-mode");
const appWindow = getCurrentWindow();

// 初始化：加载会话列表
async function init() {
  await loadSessions();
  searchInput.focus();
}

// 加载会话列表
async function loadSessions() {
  try {
    if (currentQuery.trim()) {
      sessions = await invoke("search", { query: currentQuery });
    } else {
      sessions = await invoke("list_sessions");
    }
    selectedIndex = 0;
    render();
  } catch (e) {
    console.error("加载会话失败:", e);
  }
}

// 渲染会话列表
function render() {
  sessionList.innerHTML = "";

  if (sessions.length === 0) {
    sessionList.innerHTML = '<div class="empty-state">没有找到会话</div>';
    return;
  }

  if (viewMode === "project") {
    renderGrouped();
  } else {
    renderTimeline();
  }
}

function renderTimeline() {
  let globalIdx = 0;
  sessions.forEach((s) => {
    sessionList.appendChild(createSessionItem(s, globalIdx));
    globalIdx++;
  });
}

function renderGrouped() {
  // 按 project_name 分组
  const groups = {};
  sessions.forEach((s) => {
    const key = s.project_name || "未知项目";
    if (!groups[key]) groups[key] = [];
    groups[key].push(s);
  });

  let globalIdx = 0;
  Object.entries(groups).forEach(([name, items]) => {
    const header = document.createElement("div");
    header.className = "group-header";
    header.textContent = `${name} (${items.length})`;
    sessionList.appendChild(header);

    items.forEach((s) => {
      sessionList.appendChild(createSessionItem(s, globalIdx));
      globalIdx++;
    });
  });
}

function createSessionItem(session, index) {
  const item = document.createElement("div");
  item.className = "session-item" + (index === selectedIndex ? " selected" : "");
  item.dataset.index = index;

  const promptText = session.last_prompt || session.first_prompt || "";
  const displayPrompt = highlightMatch(
    truncate(promptText, 80),
    currentQuery
  );

  item.innerHTML = `
    <div class="header">
      <span class="project-name">${escapeHtml(session.project_name)}</span>
      <span class="time">${escapeHtml(session.updated_at)}</span>
    </div>
    <div class="prompt">${displayPrompt}</div>
  `;

  item.addEventListener("click", () => {
    selectedIndex = index;
    render();
    resumeSession(session);
  });

  return item;
}

// 搜索输入防抖
let searchTimer = null;
searchInput.addEventListener("input", () => {
  currentQuery = searchInput.value;
  clearTimeout(searchTimer);
  searchTimer = setTimeout(loadSessions, 150);
});

// 视图模式切换
viewModeSelect.addEventListener("change", () => {
  viewMode = viewModeSelect.value;
  render();
});

// 键盘导航
document.addEventListener("keydown", async (e) => {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (selectedIndex < sessions.length - 1) {
      selectedIndex++;
      render();
      scrollToSelected();
    }
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    if (selectedIndex > 0) {
      selectedIndex--;
      render();
      scrollToSelected();
    }
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (sessions[selectedIndex]) {
      await resumeSession(sessions[selectedIndex]);
    }
  } else if (e.key === "c" && e.ctrlKey) {
    e.preventDefault();
    if (sessions[selectedIndex]) {
      await copyCommand(sessions[selectedIndex]);
    }
  } else if (e.key === "Escape") {
    await appWindow.hide();
  }
});

function scrollToSelected() {
  const selected = sessionList.querySelector(".selected");
  if (selected) {
    selected.scrollIntoView({ block: "nearest" });
  }
}

// 恢复会话
async function resumeSession(session) {
  try {
    await invoke("resume_session", {
      sessionId: session.session_id,
      projectPath: session.project_path,
    });
    await appWindow.hide();
  } catch (e) {
    console.error("恢复会话失败:", e);
  }
}

// 复制命令
async function copyCommand(session) {
  try {
    const cmd = await invoke("copy_command", {
      sessionId: session.session_id,
      projectPath: session.project_path,
    });
    // 使用 Tauri 剪贴板 API
    await window.__TAURI__.clipboardManager.writeText(cmd);
  } catch (e) {
    console.error("复制失败:", e);
  }
}

// 工具函数
function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str || "";
  return div.innerHTML;
}

function truncate(str, max) {
  if (!str) return "";
  return str.length > max ? str.slice(0, max) + "..." : str;
}

function highlightMatch(text, query) {
  if (!query || !query.trim()) return escapeHtml(text);
  const escaped = escapeHtml(text);
  const queryEscaped = escapeHtml(query.trim());
  const regex = new RegExp(`(${queryEscaped.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, "gi");
  return escaped.replace(regex, "<mark>$1</mark>");
}

// 窗口可见时刷新数据
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    searchInput.focus();
    loadSessions();
  }
});

init();
```

- [ ] **Step 4: 验证完整功能**

```bash
cd D:/workspace/retalk-claude && cargo tauri dev
```

Expected:
1. 系统托盘出现图标
2. Ctrl+Shift+C 弹出 Spotlight 风格窗口
3. 会话列表显示本机 Claude Code 历史
4. 搜索框可实时过滤
5. Enter 可恢复会话
6. Esc 隐藏窗口

- [ ] **Step 5: 提交**

```bash
git add src/
git commit -m "feat: Spotlight 风格前端 UI，搜索/导航/恢复/复制"
```

---

### Task 12: 窗口失焦隐藏 + 前端剪贴板修正

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/main.js`

- [ ] **Step 1: 在 lib.rs 的 setup 中添加窗口失焦隐藏**

在 `app.global_shortcut().register(hotkey)?;` 之后，`Ok(())` 之前添加：

```rust
            // === 窗口失焦自动隐藏 ===
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                    }
                });
            }
```

- [ ] **Step 2: 修正 main.js 中剪贴板调用路径**

Tauri v2 的剪贴板插件前端 API 路径可能不同。将 `copyCommand` 函数中的剪贴板调用改为：

```javascript
async function copyCommand(session) {
  try {
    const cmd = await invoke("copy_command", {
      sessionId: session.session_id,
      projectPath: session.project_path,
    });
    // 尝试 Tauri 剪贴板，降级到浏览器 API
    if (window.__TAURI__?.clipboardManager?.writeText) {
      await window.__TAURI__.clipboardManager.writeText(cmd);
    } else if (navigator.clipboard) {
      await navigator.clipboard.writeText(cmd);
    }
  } catch (e) {
    console.error("复制失败:", e);
  }
}
```

- [ ] **Step 3: 验证失焦隐藏**

```bash
cd D:/workspace/retalk-claude && cargo tauri dev
```

Expected: 弹出窗口后点击桌面其他地方，窗口自动隐藏。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/lib.rs src/main.js
git commit -m "fix: 窗口失焦自动隐藏 + 剪贴板兼容降级"
```

---

### Task 13: 构建与打包验证

**Files:**
- No new files

- [ ] **Step 1: 执行 release 构建**

```bash
cd D:/workspace/retalk-claude && cargo tauri build
```

Expected: 在 `src-tauri/target/release/` 下生成 `retalk.exe`，在 `src-tauri/target/release/bundle/nsis/` 下生成安装包。

- [ ] **Step 2: 运行 release 版本**

直接运行生成的 exe，验证：
1. 系统托盘图标正常
2. Ctrl+Shift+C 弹窗正常
3. 会话列表加载正常
4. 搜索、恢复、复制功能正常

- [ ] **Step 3: 提交最终状态**

```bash
git add -A
git commit -m "chore: 构建验证通过"
```
