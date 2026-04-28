# retalk

A fast, Spotlight-style session manager for AI coding CLI tools on Windows.

Retalk lives in your system tray and lets you instantly search, browse, and resume conversations across multiple AI coding assistants with a single hotkey.

## Why

When you work with multiple AI coding tools across many projects, finding and resuming the right conversation becomes painful. Retalk solves this by:

- Indexing all your sessions from Claude Code, Codex, Gemini CLI, OpenCode, and Kilo Code
- Providing instant full-text search with Chinese language support
- Letting you resume any session in one keystroke

## Features

### Core

- **Spotlight-style popup** — `Ctrl+Shift+C` to toggle, `Esc` to dismiss
- **Full-text search** — Tantivy engine with jieba Chinese segmentation, sub-millisecond queries
- **Multi-tool support** — 5 providers with session resume
- **One-click resume** — Opens a new terminal, `cd` to project, runs the resume command
- **System tray** — Always running, left-click or hotkey to open

### Providers

| Provider | Data Format | Resume Command | Status |
|----------|------------|----------------|--------|
| Claude Code | JSONL | `claude --resume <id>` | Active |
| Codex CLI | JSONL | `codex resume <id>` | Active |
| Gemini CLI | JSON | `gemini --resume <id>` | Active |
| OpenCode | SQLite | `opencode --session <id>` | Active |
| Kilo Code | SQLite | `kilo --session <id>` | Active |

### Organization

- **Dual view** — By project (grouped) or timeline (chronological)
- **Time groups** — Today / Yesterday / This Week / This Month / Earlier
- **Sorting** — By time or by name
- **Provider filter** — Filter sessions by tool, powered by backend Tantivy queries
- **Favorites** — Star important sessions, always pinned to top
- **Tags** — Custom labels per session, searchable. Auto-tagging by keyword detection (bug fix, refactor, new feature, test, deploy, docs)

### Productivity

- **Session preview** — Arrow keys to navigate, bottom panel shows last 3 messages
- **Context menu** — Right-click for: resume, open in VS Code, open in Explorer, copy path, copy resume command, export Markdown, compare tools
- **Session compare** — Side-by-side view of sessions from different tools on the same project
- **Batch operations** — Ctrl+Click multi-select, batch export to Markdown
- **Git integration** — Shows current branch and uncommitted changes count per project
- **Token & cost estimates** — Displays token usage and estimated cost for Claude and Codex sessions

### Settings

- **Global hotkey** — Press-to-capture key binding
- **Terminal preference** — Auto-detect, Windows Terminal, PowerShell, or CMD
- **Update strategies** — File watcher, polling (configurable interval), on-demand refresh. All with performance stats
- **Auto-start** — Windows startup via registry
- **Rebuild index** — One-click full re-index from settings
- **Max results** — Configurable query limit

### Statistics

- Session count per provider
- Top 5 most active projects (bar chart)
- Monthly activity (bar chart)
- Hot projects (activity spike this week)
- Dormant projects (no activity in 30+ days)

### Performance

- **Instant startup** — Persisted Tantivy index opens from disk, UI available immediately
- **Incremental sync** — Background thread diffs current sessions against index, only updates changes
- **Non-blocking** — All heavy work (scanning, indexing, git queries) runs in background threads with `try_lock` to prevent UI freezing
- **Debounced search** — 150ms input debounce, real-time results

## Tech Stack

- **Rust** + **Tauri v2** — Native Windows app, ~8MB binary
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
  terminal.rs      # Terminal detection and session resume
  commands.rs      # Tauri IPC commands (thin glue layer)
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
```

## Build

Requirements: Rust 1.77+, Node.js 18+, Windows 11 (WebView2 included)

```bash
# Install Tauri CLI
npm install -D @tauri-apps/cli@latest

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

## License

MIT
