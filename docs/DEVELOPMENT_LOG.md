# retalk 开发日志

## 项目概述

**retalk** — 跨平台 AI 编码助手会话管理器，Spotlight 风格系统托盘应用。

- 著作权人：王玉强
- 技术栈：Rust + Tauri v2 + Tantivy + jieba-rs + vanilla JS
- 仓库：https://github.com/akapril/retalk-claude
- 开发周期：2026年4月27日 - 2026年5月7日
- 总 commit 数：60+

---

## 支持的 AI 编码工具（5个）

| 工具 | 数据格式 | 恢复命令 | 数据路径 |
|------|---------|---------|---------|
| Claude Code | JSONL | `claude --resume <id>` | `~/.claude/projects/` |
| Codex CLI | JSONL | `codex resume <id>` | `~/.codex/sessions/` |
| Gemini CLI | JSON | `gemini --resume <id>` | `~/.gemini/tmp/` |
| OpenCode | SQLite | `opencode --session <id>` | `~/.local/share/opencode/` |
| Kilo Code | SQLite | `kilo --session <id>` | `~/.local/share/kilo/` |

npm 包名（用于版本检查和安装）：
- Claude: `@anthropic-ai/claude-code`
- Codex: `@openai/codex`
- Gemini: `@google/gemini-cli`
- OpenCode: `opencode-ai`
- Kilo: 无 npm 包

---

## 功能清单

### 核心功能
- Spotlight 弹窗（全局快捷键 Ctrl+Shift+R / Cmd+Shift+R）
- 全文搜索（Tantivy + jieba 中文分词 + 特殊字符转义 + 路径子串匹配）
- 多工具会话统一管理（5 个 Provider）
- 一键恢复 / 新建会话
- 系统托盘常驻

### 会话管理
- 双视图（按项目分组 / 时间线）
- 时间分组（今天/昨天/本周/本月/更早）
- 排序（按时间/按名称）
- 工具筛选（后端 Tantivy 查询，不截断）
- 收藏置顶（排序后仍保持顶部）
- 标签系统（自定义 + 自动标签识别）
- 会话备注（独立存储，不修改原始文件）
- 会话预览（最近3条消息）
- Token/成本估算

### 效率工具
- 右键菜单（新建/恢复/VS Code/文件管理器/复制路径/复制命令/导出/对比）
- 会话对比（同项目不同工具并排）
- 批量操作（Ctrl+Click 多选，批量导出）
- Git 集成（分支名 + 未提交变更数）
- 导出 Markdown（剪贴板 + 保存到桌面并定位文件）
- 单击选中 / 双击打开
- 默认打开方式可配置（终端 / VS Code）

### 生态面板（4个Tab）
- 插件管理：Claude + Codex 插件（安装/卸载/启用/禁用/更新），Gemini 扩展，二级导航按工具+已安装/可安装
- Skills 可视化：按工具分组，扫描 plugins/cache 和独立插件目录
- MCP 服务器管理：按工具分组，添加/移除/启用/禁用，跨 Claude/Codex/Gemini/OpenCode/Kilo
- 工具概览：安装状态/版本号/会话数/MCP数/Skills数/数据路径/检查更新/一键安装

### 设置
- 快捷键（按键捕获式录入，保存后热更新生效）
- 默认打开方式（终端/VS Code）
- 终端偏好（Windows 4种 / macOS 8种 / Linux 4种 + 自定义命令模板+文件浏览）
- 默认工作目录（自动检测+手动设置）
- 数据更新策略（文件监听/定时轮询/按需刷新）
- 主题切换（深色/浅色）
- 开机自启（Windows注册表/macOS LaunchAgents/Linux XDG）
- 重建索引
- 自动标签
- 国际化（中英文双语，自动检测浏览器语言）

### 统计
- 各工具会话数量
- 最活跃项目 Top 5（柱状图）
- 月度活跃度（柱状图）
- 频繁活动项目（本周 vs 上周）
- 长期未活动项目（30天+）

---

## 架构设计

### 项目结构
```
src-tauri/src/
  main.rs              入口
  lib.rs               Tauri 初始化、托盘、快捷键、窗口管理
  models.rs            数据模型（Session, Config, Stats 等）
  config.rs            配置文件读写（~/.claude/retalk/config.toml）
  scanner.rs           多工具会话聚合扫描（薄代理层）
  indexer.rs           Tantivy 索引管理、增量同步
  searcher.rs          搜索查询（全文搜索 + provider 过滤 + 路径匹配）
  updater.rs           三种更新策略 + 性能统计
  terminal.rs          跨平台终端检测与会话恢复
  commands.rs          Tauri IPC 命令（全部 async）
  ecosystem.rs         生态扫描（Skills/MCP/插件/配置/概览）
  providers/
    mod.rs             SessionProvider trait + 注册
    claude.rs          Claude Code JSONL 解析（含 cwd 提取修复中文路径）
    codex.rs           Codex CLI JSONL 解析
    gemini.rs          Gemini CLI JSON 解析（兼容新旧目录结构 + content 字段格式）
    opencode.rs        OpenCode/Kilo SQLite 共用读取
    kilo.rs            Kilo Code SQLite 读取
src/
  index.html           Spotlight 弹窗布局
  style.css            暗色/亮色主题、扁平 SVG 图标、自定义下拉组件
  main.js              全部前端逻辑（vanilla JS、i18n）
```

### 数据存储
```
~/.claude/retalk/
  config.toml          用户配置
  index/               Tantivy 搜索索引（持久化）
  favorites.json       收藏的会话 ID
  tags.json            会话标签
  notes.json           会话备注
```

### 关键设计决策

1. **Provider 抽象**：SessionProvider trait，新增工具只需实现 trait
2. **增量索引**：启动时对比 session_id + updated_at 时间戳，只 upsert 变化的
3. **持久化索引**：二次启动直接从磁盘加载，UI 秒开
4. **后台扫描**：全量扫描在 spawn 线程，ready 标志位控制前端等待
5. **全异步 IPC**：所有外部命令用 async + spawn_blocking，try_lock 防锁竞争
6. **macOS 透明窗口**：macOSPrivateApi + cocoa NSWindow 背景清除
7. **macOS 状态栏图标**：Template Image（亮度→alpha 映射）
8. **Windows cmd**：npm.cmd / code.cmd / cd /d / CREATE_NO_WINDOW

---

## 已解决的关键问题

### 中文路径
- Claude Code 目录编码把中文替换为 `-`，decode 反推会出错
- 修复：从 session JSONL 的 `cwd` 字段直接获取真实路径

### Windows 跨盘符
- `cd "D:\..."` 在 C 盘 cmd 里不切盘
- 修复：改为 `cd /d "D:\..."`

### Provider 过滤截断
- 646 条 Claude 会话 + max_results=50 → Gemini 会话被截断
- 修复：provider 过滤从前端移到后端 Tantivy TermQuery

### 启动卡死
- 全量扫描阻塞主线程 → UI 无法打开
- 修复：扫描移到后台线程 + ready 标志 + waitForReady 轮询

### on_demand_refresh 重复扫描
- 首次 list_sessions 时 mtime 为 None → 误判有变化 → 再次全量扫描
- 修复：后台扫描完成后立即 init_mtime_snapshot

### Gemini 数据丢失
- 新版 Gemini CLI 用项目名目录（非 hash）+ content 字段从 string 改为 array
- 修复：兼容两种目录结构 + projects.json 映射 + content 字段类型兼容

### Codex token 不显示
- token 数据在 `payload.info.total_token_usage`，非根级
- 修复：从嵌套路径提取，用 `=` 赋值（累计值取最后一条）

### 终端闪烁
- git/reg/where 等后台命令在 Windows 弹 cmd 窗口
- 修复：CREATE_NO_WINDOW (0x08000000) 标志

---

## 跨平台适配

### 终端支持

**Windows（3种）**：Windows Terminal / PowerShell / CMD

**macOS（8种）**：Terminal.app / iTerm2 / Ghostty / Warp / Kitty / Alacritty / WezTerm / Rio

**Linux（4种）**：gnome-terminal / konsole / alacritty / kitty

**自定义**：命令模板 + 文件浏览选择（{cmd} 和 {dir} 占位符）

### 自启动
- Windows：注册表 `HKCU\...\Run`
- macOS：LaunchAgents plist
- Linux：XDG autostart desktop 文件

### 文件管理器
- Windows：`explorer` / `explorer /select,`
- macOS：`open` / `open -R`
- Linux：`xdg-open`

### 快捷键默认值
- Windows/Linux：Ctrl+Shift+R
- macOS：Cmd+Shift+R

---

## CI/CD

GitHub Actions 跨平台构建（`.github/workflows/release.yml`）：
- Windows x64：NSIS + MSI
- macOS ARM64：DMG
- macOS Intel：DMG
- Linux x64：DEB + AppImage

触发方式：push tag `v*` 或手动 workflow_dispatch

关键配置：
- `tauriScript: npx tauri`（不需要 cargo-tauri）
- 图标必须 8-bit RGBA PNG32
- `macOSPrivateApi: true` 启用透明窗口

---

## 软著申请

### 申请信息
- 软件全称：retalk AI编码助手会话管理软件
- 版本号：V1.0
- 著作权人：王玉强
- 开发完成日期：2026-04-28
- 首次发表日期：2026-04-28
- 首次发表地点：中国 山东 临沂

### 提交材料
- 程序鉴别材料：`docs/copyright/程序鉴别材料.pdf`（60页，前30+后30，每页50行）
- 文档鉴别材料：`docs/copyright/文档鉴别材料.pdf`（10页，全文提交，每页≥30行）
- 生成脚本在 commit 历史中可追溯
