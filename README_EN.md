<p align="center">
  <img src="src-tauri/icons/icon.png" width="128" height="128" alt="retalk">
</p>

<h1 align="center">retalk</h1>

<p align="center">
  English | <a href="README.md">中文</a>
</p>

<p align="center">
  A fast, lightweight session manager for AI coding CLI tools<br>
  Spotlight-style popup — one hotkey to manage all your CLI conversations
</p>

<p align="center">
  <a href="https://github.com/akapril/retalk-claude/releases"><img src="https://img.shields.io/github/v/release/akapril/retalk-claude?style=flat-square" alt="Release"></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/github/license/akapril/retalk-claude?style=flat-square" alt="License">
</p>

---

## Why

When you work with multiple AI coding tools across many projects, finding and resuming the right conversation becomes painful. retalk solves this by:

- Indexing all your sessions from Claude Code, Codex, Gemini CLI, OpenCode, and Kilo Code
- Providing instant full-text search with Chinese language support
- Letting you resume any session with a double-click

## Supported Tools

| Provider | Data Format | Resume Command |
|----------|------------|----------------|
| Claude Code | JSONL | `claude --resume <id>` |
| Codex CLI | JSONL | `codex resume <id>` |
| Gemini CLI | JSON | `gemini --resume <id>` |
| OpenCode | SQLite | `opencode --session <id>` |
| Kilo Code | SQLite | `kilo --session <id>` |

## Features

### Core

- **Spotlight-style popup** — `Ctrl+Shift+C` to toggle, `Esc` to dismiss
- **Full-text search** — Tantivy engine with jieba Chinese segmentation, sub-millisecond queries
- **Multi-tool support** — 5 providers with session resume
- **One-click resume** — Double-click or Enter to open terminal and resume session
- **System tray** — Always running, left-click or hotkey to open
- **Cross-platform** — Windows / macOS / Linux

### Session Management

- **Dual view** — By project (grouped) or timeline (chronological)
- **Time groups** — Today / Yesterday / This Week / This Month / Earlier
- **Sorting** — By time or by name
- **Provider filter** — Filter sessions by tool (backend Tantivy queries, no truncation)
- **Favorites** — Star important sessions, always pinned to top
- **Tags** — Custom labels per session, searchable. Auto-tagging by keyword detection (bug fix, refactor, new feature, test, deploy, docs)
- **Session notes** — Stored independently, never modifies original conversation files

### Productivity

- **Session preview** — Arrow keys to navigate, bottom panel shows last 3 messages
- **Context menu** — Right-click for: resume, open in VS Code, open in Explorer, copy path, copy resume command, export Markdown, compare tools
- **Session compare** — Side-by-side view of sessions from different tools on the same project
- **Batch operations** — Ctrl+Click multi-select, batch export to Markdown
- **Git integration** — Shows current branch and uncommitted changes count per project
- **Token & cost estimates** — Displays token usage and estimated cost for Claude and Codex sessions
- **Export** — Export Markdown to clipboard or file (auto-opens file manager to locate)

### Ecosystem Panel

- **Plugin management** — View installed / available plugins, one-click install / enable / disable / update / uninstall (Claude Code + Gemini extensions)
- **Skills visualization** — Cross-tool display of all installed skills (including those inside plugins)
- **MCP server management** — View / add / enable / disable / remove (across Claude / Codex / Gemini / OpenCode / Kilo)
- **Tools overview** — Installation status, version, session count, MCP count, data directory for each CLI

### Settings

- **Global hotkey** — Press-to-capture key binding
- **Default open action** — Resume in terminal or open in VS Code
- **Terminal preference** — Auto-detect / Windows Terminal / PowerShell / CMD / macOS Terminal / iTerm2 / Linux terminals
- **Update strategies** — File watcher, polling (configurable interval), on-demand refresh, all with performance stats
- **Auto-start** — Windows registry / macOS LaunchAgents / Linux XDG autostart
- **Rebuild index** — One-click full re-index from settings

### Statistics

- Session count per provider
- Top 5 most active projects (bar chart)
- Monthly activity (bar chart)
- Hot projects (activity spike this week vs last week)
- Dormant projects (no activity in 30+ days)

### Performance

- **Instant startup** — Persisted Tantivy index opens from disk, UI available immediately
- **Incremental sync** — Background thread diffs current sessions against index, only updates changes
- **Non-blocking** — All heavy work (scanning, indexing, git queries) runs in background threads with `try_lock` to prevent UI freezing
- **Debounced search** — 150ms input debounce, real-time results

## Tech Stack

- **Rust** + **Tauri v2** — Cross-platform native app, ~8MB binary
- **Tantivy** — Full-text search engine (Rust-native Lucene alternative)
- **jieba-rs** — Chinese word segmentation
- **rusqlite** — SQLite reader for OpenCode/Kilo data
- **notify** — File system watcher for real-time updates
- **Vanilla HTML/CSS/JS** — No frontend framework, minimal overhead

## Project Structure

```
src-tauri/src/
  main.rs          # Entry point
  lib.rs           # Tauri setup, tray, hotkey, window management
  models.rs        # Session, Config, Stats data types
  config.rs        # ~/.claude/retalk/config.toml read/write
  scanner.rs       # Aggregates all providers
  indexer.rs       # Tantivy index management, incremental sync
  searcher.rs      # Full-text search + filtered listing
  updater.rs       # Three update strategies with stats
  terminal.rs      # Terminal detection and session resume (cross-platform)
  commands.rs      # Tauri IPC commands
  ecosystem.rs     # Ecosystem scanning (Skills/MCP/Plugins/Config)
  providers/
    mod.rs         # SessionProvider trait + registry
    claude.rs      # Claude Code JSONL parser
    codex.rs       # Codex CLI JSONL parser
    gemini.rs      # Gemini CLI JSON parser
    opencode.rs    # OpenCode SQLite reader (shared with Kilo)
    kilo.rs        # Kilo Code SQLite reader
src/
  index.html       # Spotlight popup layout
  style.css        # Dark theme, glassmorphism, flat SVG icons
  main.js          # All frontend logic (vanilla JS)
```

## Data Storage

```
~/.claude/retalk/
  config.toml      # User preferences
  index/           # Tantivy search index (persistent)
  favorites.json   # Starred session IDs
  tags.json        # Session tags
  notes.json       # Session notes
```

## Build

Requirements: Rust 1.77+, Node.js 18+

```bash
# Install dependencies
npm install

# Development
npx tauri dev

# Release build
npx tauri build
```

## Configuration

Default config at `~/.claude/retalk/config.toml`:

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

## License

MIT
