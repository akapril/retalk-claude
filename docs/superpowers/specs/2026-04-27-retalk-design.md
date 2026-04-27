# Retalk — Claude Code 会话管理器

## 概述

Retalk 是一个 Windows 系统托盘应用，用于管理 Claude Code 的会话历史。解决多项目多会话场景下，关闭终端后无法快速找到和恢复历史对话的痛点。

**技术栈**：Rust + Tauri v2 + Tantivy + 纯 HTML/CSS/JS 前端

## 核心功能

1. **系统托盘常驻** — 左键弹出/隐藏窗口，右键菜单（设置、性能统计、退出）
2. **全局快捷键** — 默认 `Ctrl+Shift+C`，切换弹窗显隐
3. **Spotlight 风格弹窗** — 居中搜索框 + 下拉会话列表
4. **全文搜索** — Tantivy + jieba 中文分词，搜索所有会话内容
5. **双视图模式** — 时间线（按最后活跃时间排序）/ 按项目分组，用户可切换
6. **一键恢复** — Enter 打开新终端执行 `cd <project> && claude --resume <session_id>`
7. **复制命令** — Ctrl+C 将恢复命令复制到剪贴板

## 架构

```
┌─────────────────────────────────────────┐
│              retalk (Tauri v2)           │
│                                         │
│  ┌─────────┐   ┌──────────────────────┐ │
│  │ 系统托盘 │   │   Spotlight 弹窗     │ │
│  │ (TrayIcon)│   │   (WebView Window)  │ │
│  └────┬─────┘   └──────────┬──────────┘ │
│       │                    │             │
│  ┌────┴────────────────────┴──────────┐ │
│  │         Rust 后端核心               │ │
│  │                                     │ │
│  │  ┌───────────┐  ┌───────────────┐  │ │
│  │  │ 数据扫描器 │  │ Tantivy 索引  │  │ │
│  │  │ (Scanner)  │  │ (全文搜索)    │  │ │
│  │  └─────┬─────┘  └───────┬───────┘  │ │
│  │        │                │           │ │
│  │  ┌─────┴────────────────┴────────┐  │ │
│  │  │    文件监听 / 定时轮询 / 按需   │  │ │
│  │  │    (三种更新策略)              │  │ │
│  │  └───────────────────────────────┘  │ │
│  └─────────────────────────────────────┘ │
│                                         │
│  数据源: ~/.claude/history.jsonl         │
│         ~/.claude/projects/**/*.jsonl    │
└─────────────────────────────────────────┘
```

**前后端解耦**：Rust 核心逻辑（scanner、indexer、searcher）作为独立 lib 模块，`commands.rs` 只是薄薄一层胶水暴露给 Tauri IPC。前端可随时替换为 React/Vue/Svelte，或将后端改为 HTTP API 独立部署。

## 数据模型

### 会话结构

```rust
struct Session {
    session_id: String,          // UUID
    project_path: String,        // 如 "D:\workspace\geo2"
    project_name: String,        // 取路径最后一段
    first_prompt: String,        // 用户第一条消息（会话标题）
    last_prompt: String,         // 用户最后一条消息
    created_at: DateTime,        // 第一条消息时间
    updated_at: DateTime,        // 最后一条消息时间
    message_count: u32,          // 总消息数
    user_messages: Vec<String>,  // 所有用户消息（用于全文索引）
}
```

### 数据源解析

1. **快速路径** — 扫描 `history.jsonl`，提取 project + sessionId + timestamp + display 映射，建立会话列表骨架
2. **补全路径** — 按需读取 `projects/{编码路径}/{session_id}.jsonl`，提取完整消息内容
3. **路径编码还原** — 不做简单字符串替换（项目名可能含 `-`），而是用 `history.jsonl` 中的 `project` 字段（原始路径）与 `projects/` 目录名建立映射表

### Tantivy 索引字段

| 字段 | 类型 | 用途 |
|------|------|------|
| session_id | STRING (stored) | 唯一标识 |
| project_path | STRING (stored) | 分组筛选 |
| project_name | TEXT (indexed) | 按项目名搜索 |
| first_prompt | TEXT (stored + indexed) | 显示标题 + 搜索 |
| content | TEXT (indexed) | 全部用户消息，全文搜索 |
| updated_at | DATE (stored + indexed) | 排序 |

**索引存储位置**：`~/.claude/retalk/index/`

## UI 交互

### 弹窗布局

```
╔══════════════════════════════════════════╗
║  🔍 搜索会话...            [时间线 ▼]   ║  搜索框 + 视图切换
╠══════════════════════════════════════════╣
║                                          ║
║  geo2                        4月16日     ║  项目名 + 时间
║  开始一下                                ║  最后一条 prompt
║  ─────────────────────────────────────── ║
║  exam-alert                  4月27日     ║
║  这些考试都需要                           ║
║                                          ║
╠══════════════════════════════════════════╣
║  ↑↓ 导航  Enter 恢复  Ctrl+C 复制命令   ║  底部快捷键提示
╚══════════════════════════════════════════╝
```

### 交互行为

| 操作 | 行为 |
|------|------|
| `Ctrl+Shift+C` | 弹出/隐藏窗口 |
| 托盘左键单击 | 弹出/隐藏窗口 |
| 托盘右键 | 菜单：设置、性能统计、退出 |
| 输入文字 | 实时搜索，过滤列表 |
| `↑` / `↓` | 上下选择会话 |
| `Enter` | 打开新终端，cd + claude --resume |
| `Ctrl+C` | 复制恢复命令到剪贴板 |
| `Esc` / 失焦 | 隐藏窗口 |
| 右上角下拉 | 切换视图：时间线 / 按项目分组 |

### 窗口特性

- 无边框、圆角、半透明背景（毛玻璃，如系统支持）
- 固定宽度 ~600px，高度自适应（最大 ~500px，超出滚动）
- 屏幕居中偏上
- 弹出时自动聚焦搜索框
- 搜索结果高亮匹配关键词

## 三种更新策略

### 策略 1：文件系统监听（主力）

- `notify` crate 监听 `~/.claude/` 目录
- 监听 `history.jsonl` 修改和 `projects/` 下 `.jsonl` 新增/修改
- 防抖 500ms，合并多次变更

### 策略 2：定时轮询（兜底）

- 默认 30 秒一次，可配置
- 对比文件 mtime，仅处理变更文件
- 文件监听失败时的降级方案

### 策略 3：按需扫描（弹窗时）

- 每次弹窗显示时检查 `history.jsonl` 的 mtime
- 有变化则增量更新
- 确保用户看到最新数据

**三种策略默认全部开启**，互不冲突。通过性能统计面板查看各策略的触发次数、耗时等指标，后续根据数据决定保留/剔除。

### 性能监控

```rust
struct UpdateStats {
    strategy: String,       // "watcher" / "poll" / "on_demand"
    trigger_count: u64,     // 触发次数
    total_time_ms: u64,     // 累计耗时
    avg_time_ms: f64,       // 平均耗时
    last_trigger: DateTime, // 最近一次触发
    files_processed: u64,   // 处理文件数
}
```

## 终端集成

### 恢复命令

```bash
cd "D:\workspace\geo2" && claude --resume 164832d7-8ce0-4e7b-900c-175f78e6062f
```

### 终端检测优先级

1. Windows Terminal (`wt.exe`) — Win11 默认
2. PowerShell (`pwsh.exe` / `powershell.exe`)
3. `cmd.exe` — 兜底

用户可在配置中手动指定。

### Windows Terminal 调用

```
wt.exe new-tab --title "retalk: geo2" -d "D:\workspace\geo2" cmd /k claude --resume <session_id>
```

## IPC 接口（前后端契约）

```
invoke("search", { query, mode })        → Vec<Session>
invoke("list_sessions", { sort, group }) → Vec<Session>
invoke("resume_session", { session_id }) → ()
invoke("copy_command", { session_id })   → ()
invoke("get_stats")                      → UpdateStats
invoke("get_config")                     → Config
invoke("save_config", { config })        → ()
```

## 配置文件

存储位置：`~/.claude/retalk/config.toml`

```toml
[general]
hotkey = "Ctrl+Shift+C"

[terminal]
preferred = "auto"  # "auto" / "wt" / "pwsh" / "cmd"

[update]
watcher_enabled = true
poll_enabled = true
poll_interval_secs = 30
on_demand_enabled = true

[ui]
theme = "dark"
max_results = 50
```

## 项目结构

```
retalk-claude/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs          # 入口，初始化 Tauri
│   │   ├── lib.rs           # 核心库导出
│   │   ├── tray.rs          # 托盘图标与菜单
│   │   ├── hotkey.rs        # 全局快捷键注册
│   │   ├── scanner.rs       # JSONL 解析，会话数据提取
│   │   ├── indexer.rs       # Tantivy 索引管理
│   │   ├── searcher.rs      # 搜索查询接口
│   │   ├── updater.rs       # 三种更新策略
│   │   ├── terminal.rs      # 终端检测与启动
│   │   ├── config.rs        # 配置读写
│   │   └── commands.rs      # Tauri IPC 命令
│   └── icons/               # 托盘图标资源
├── src/                      # 前端 (纯 HTML+CSS+JS)
│   ├── index.html
│   ├── style.css
│   └── main.js
└── docs/
    └── superpowers/specs/
```

### 关键依赖

| crate | 用途 |
|-------|------|
| `tauri` v2 | 窗口、托盘、快捷键、IPC |
| `tantivy` | 全文搜索索引 |
| `jieba-rs` | 中文分词 |
| `notify` | 文件系统监听 |
| `serde` / `serde_json` | JSONL 解析 |
| `toml` | 配置文件 |
| `chrono` | 时间处理 |
