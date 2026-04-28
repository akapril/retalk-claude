<p align="center">
  <img src="src-tauri/icons/icon.png" width="128" height="128" alt="retalk">
</p>

<h1 align="center">retalk</h1>

<p align="center">
  <a href="README_EN.md">English</a> | 中文
</p>

<p align="center">
  快速、轻量的 AI 编码助手会话管理器<br>
  Spotlight 风格弹窗，一个快捷键管理所有 CLI 对话
</p>

<p align="center">
  <a href="https://github.com/akapril/retalk-claude/releases"><img src="https://img.shields.io/github/v/release/akapril/retalk-claude?style=flat-square" alt="Release"></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/github/license/akapril/retalk-claude?style=flat-square" alt="License">
</p>

---

## 为什么需要

同时使用多个 AI 编码工具、跨多个项目工作时，关闭终端后很难找到之前的对话。retalk 解决这个问题：

- 自动索引所有工具的会话数据
- 全文搜索（支持中文分词）
- 一键恢复任意会话到终端

## 支持的工具

| 工具 | 数据格式 | 恢复命令 |
|------|---------|---------|
| Claude Code | JSONL | `claude --resume <id>` |
| Codex CLI | JSONL | `codex resume <id>` |
| Gemini CLI | JSON | `gemini --resume <id>` |
| OpenCode | SQLite | `opencode --session <id>` |
| Kilo Code | SQLite | `kilo --session <id>` |

## 功能

### 核心

- **Spotlight 弹窗** — `Ctrl+Shift+C` 弹出/隐藏，`Esc` 关闭
- **全文搜索** — Tantivy 引擎 + jieba 中文分词，亚毫秒级响应
- **多工具支持** — 5 个 provider，全部支持会话恢复
- **一键恢复** — 双击或 Enter 自动打开终端恢复会话
- **系统托盘** — 常驻后台，左键点击或快捷键打开
- **跨平台** — Windows / macOS / Linux

### 会话管理

- **双视图** — 按项目分组 / 时间线
- **时间分组** — 今天 / 昨天 / 本周 / 本月 / 更早
- **排序** — 按时间或按名称
- **工具筛选** — 按 provider 过滤（后端 Tantivy 查询，不截断）
- **收藏置顶** — 星标重要会话，始终在列表顶部
- **标签系统** — 自定义标签，支持搜索。自动标签识别（bug修复、重构、新功能、测试、部署、文档）
- **会话备注** — 独立存储，不修改原始对话文件

### 效率工具

- **会话预览** — 方向键导航，底部面板显示最近 3 条消息
- **右键菜单** — 恢复 / 在 VS Code 中打开 / 在文件管理器中打开 / 复制路径 / 复制恢复命令 / 导出 Markdown / 对比工具
- **会话对比** — 同一个项目在不同工具中的会话并排对比
- **批量操作** — Ctrl+Click 多选，批量导出 Markdown
- **Git 集成** — 显示当前分支和未提交变更数
- **Token 估算** — 显示 Claude 和 Codex 会话的 token 用量和预估费用
- **导出** — 导出 Markdown 到剪贴板或文件（自动打开文件管理器定位）

### 生态面板

- **插件管理** — 查看已安装 / 可安装插件，一键安装 / 启用 / 禁用 / 更新 / 卸载（Claude Code + Gemini 扩展）
- **Skills 可视化** — 跨工具展示所有已安装 skills（含插件内的）
- **MCP 服务器管理** — 查看 / 添加 / 启用 / 禁用 / 移除（跨 Claude / Codex / Gemini / OpenCode / Kilo）
- **工具概览** — 各 CLI 安装状态、版本号、会话数、MCP 数、数据目录

### 设置

- **全局快捷键** — 按键捕获式设置
- **默认打开方式** — 终端恢复 或 VS Code 打开
- **终端偏好** — 自动检测 / Windows Terminal / PowerShell / CMD / macOS Terminal / iTerm2 / Linux 终端
- **更新策略** — 文件监听、定时轮询、按需刷新，三种策略带性能统计
- **开机自启** — Windows 注册表 / macOS LaunchAgents / Linux XDG autostart
- **重建索引** — 设置页一键重建

### 使用统计

- 各工具会话数量
- 最活跃项目 Top 5（柱状图）
- 月度活跃度（柱状图）
- 频繁活动项目（本周 vs 上周对比）
- 长期未活动项目（30 天以上无会话）

### 性能

- **秒开** — 持久化 Tantivy 索引从磁盘加载，UI 立即可用
- **增量同步** — 后台线程对比索引与当前数据，只更新变化部分
- **不阻塞** — 扫描、索引、Git 查询全部后台线程，`try_lock` 防止 UI 冻结
- **搜索防抖** — 150ms 输入防抖，实时响应

## 技术栈

- **Rust** + **Tauri v2** — 跨平台原生应用，约 8MB
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
  terminal.rs      # 终端检测与会话恢复（跨平台）
  commands.rs      # Tauri IPC 命令
  ecosystem.rs     # 生态扫描（Skills/MCP/插件/配置）
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
  notes.json       # 会话备注
```

## 构建

环境要求：Rust 1.77+、Node.js 18+

```bash
# 安装依赖
npm install

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
preferred = "auto"  # auto / wt / pwsh / cmd / terminal / iterm

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
