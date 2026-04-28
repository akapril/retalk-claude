# retalk

[English](README_EN.md) | 中文

快速、轻量的 AI 编码助手会话管理器，Spotlight 风格，常驻系统托盘。

一个快捷键，即可搜索、浏览和恢复你在 Claude Code、Codex、Gemini CLI、OpenCode、Kilo Code 中的所有对话。

## 为什么需要

同时使用多个 AI 编码工具、跨多个项目工作时，关闭终端后很难找到之前的对话。retalk 解决这个问题：

- 自动索引所有工具的会话数据
- 全文搜索（支持中文分词）
- 一键恢复任意会话到终端

## 功能

### 核心

- **Spotlight 弹窗** — `Ctrl+Shift+C` 弹出/隐藏，`Esc` 关闭
- **全文搜索** — Tantivy 引擎 + jieba 中文分词，亚毫秒级响应
- **多工具支持** — 5 个 provider，全部支持会话恢复
- **一键恢复** — 自动打开终端，切换到项目目录，执行恢复命令
- **系统托盘** — 常驻后台，左键点击或快捷键打开

### 支持的工具

| 工具 | 数据格式 | 恢复命令 |
|------|---------|---------|
| Claude Code | JSONL | `claude --resume <id>` |
| Codex CLI | JSONL | `codex resume <id>` |
| Gemini CLI | JSON | `gemini --resume <id>` |
| OpenCode | SQLite | `opencode --session <id>` |
| Kilo Code | SQLite | `kilo --session <id>` |

### 会话管理

- **双视图** — 按项目分组 / 时间线
- **时间分组** — 今天 / 昨天 / 本周 / 本月 / 更早
- **排序** — 按时间或按名称
- **工具筛选** — 按 provider 过滤，后端 Tantivy 查询
- **收藏置顶** — 星标重要会话，始终在列表顶部
- **标签系统** — 自定义标签，支持搜索。自动标签识别（bug修复、重构、新功能、测试、部署、文档）

### 效率工具

- **会话预览** — 方向键导航，底部面板显示最近 3 条消息
- **右键菜单** — 恢复 / 在 VS Code 中打开 / 在文件管理器中打开 / 复制路径 / 复制恢复命令 / 导出 Markdown / 对比工具
- **会话对比** — 同一个项目在不同工具中的会话并排对比
- **批量操作** — Ctrl+Click 多选，批量导出 Markdown
- **Git 集成** — 显示当前分支和未提交变更数
- **Token 估算** — 显示 Claude 和 Codex 会话的 token 用量和预估费用
- **导出** — 导出 Markdown 到剪贴板或文件（自动打开文件管理器定位）

### 设置

- **全局快捷键** — 按键捕获式设置，点击输入框后按下组合键自动录入
- **终端偏好** — 自动检测 / Windows Terminal / PowerShell / CMD
- **更新策略** — 文件监听、定时轮询（可配置间隔）、按需刷新，三种策略带性能统计
- **开机自启** — Windows 注册表自启动
- **重建索引** — 设置页一键重建，数据异常时无需手动删文件
- **最大结果数** — 可配置查询返回上限

### 使用统计

- 各工具会话数量
- 最活跃项目 Top 5（柱状图）
- 月度活跃度（柱状图）
- 频繁活动项目（本周 vs 上周活跃度对比）
- 长期未活动项目（30 天以上无会话）

### 性能

- **秒开** — 持久化 Tantivy 索引从磁盘加载，UI 立即可用
- **增量同步** — 后台线程对比索引与当前数据，只更新变化的部分
- **不阻塞** — 扫描、索引、Git 查询全部在后台线程，`try_lock` 防止 UI 冻结
- **搜索防抖** — 150ms 输入防抖，实时响应

## 技术栈

- **Rust** + **Tauri v2** — 原生 Windows 应用，约 8MB
- **Tantivy** — 全文搜索引擎（Rust 原生 Lucene 替代）
- **jieba-rs** — 中文分词
- **rusqlite** — 读取 OpenCode/Kilo 的 SQLite 数据
- **notify** — 文件系统监听，实时更新
- **原生 HTML/CSS/JS** — 无前端框架，最小开销

## 项目结构

```
src-tauri/src/
  main.rs          # 入口
  lib.rs           # Tauri 初始化、托盘、快捷键、窗口管理
  models.rs        # Session、Config、Stats 数据结构
  config.rs        # ~/.claude/retalk/config.toml 读写
  scanner.rs       # 聚合所有 provider
  indexer.rs       # Tantivy 索引管理、增量同步
  searcher.rs      # 全文搜索 + 过滤列表
  updater.rs       # 三种更新策略及性能统计
  terminal.rs      # 终端检测与会话恢复
  commands.rs      # Tauri IPC 命令（薄胶水层）
  providers/
    mod.rs         # SessionProvider trait + 注册
    claude.rs      # Claude Code JSONL 解析
    codex.rs       # Codex CLI JSONL 解析
    gemini.rs      # Gemini CLI JSON 解析
    opencode.rs    # OpenCode SQLite 读取（与 Kilo 共用）
    kilo.rs        # Kilo Code SQLite 读取
src/
  index.html       # Spotlight 弹窗布局
  style.css        # 暗色主题、毛玻璃效果、扁平 SVG 图标
  main.js          # 全部前端逻辑（原生 JS）
```

## 数据存储

```
~/.claude/retalk/
  config.toml      # 用户配置
  index/           # Tantivy 搜索索引（持久化）
  favorites.json   # 收藏的会话 ID
  tags.json        # 会话标签
```

## 构建

环境要求：Rust 1.77+、Node.js 18+、Windows 11（自带 WebView2）

```bash
# 安装 Tauri CLI
npm install -D @tauri-apps/cli@latest

# 开发模式
npx tauri dev

# 构建发布版
npx tauri build
```

## 配置

默认配置位于 `~/.claude/retalk/config.toml`：

```toml
[general]
hotkey = "Ctrl+Shift+C"

[terminal]
preferred = "auto"  # auto / wt / pwsh / cmd

[update]
watcher_enabled = true
poll_enabled = true
poll_interval_secs = 30
on_demand_enabled = true

[ui]
theme = "dark"
max_results = 1000
```

## 许可

MIT
