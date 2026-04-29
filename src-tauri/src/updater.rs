use crate::config::claude_dir;
use crate::indexer::SessionIndex;
use crate::models::{AppConfig, UpdateStats};
use crate::scanner;
use notify_debouncer_mini::new_debouncer;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// 三策略更新管理器：文件监听、定时轮询、按需刷新
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

    /// 初始化 mtime 快照，防止首次 on_demand_refresh 误判为"有变化"
    pub fn init_mtime_snapshot(&self) {
        let home = dirs::home_dir().unwrap_or_default();
        let check_paths = vec![
            claude_dir().join("history.jsonl"),
            home.join(".codex").join("history.jsonl"),
            home.join(".gemini").join("projects.json"),
            home.join(".local").join("share").join("opencode").join("opencode.db"),
            home.join(".local").join("share").join("kilo").join("kilo.db"),
        ];
        let mtime = check_paths.iter()
            .filter_map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
            .max();
        *self.last_history_mtime.lock() = mtime;
    }

    /// 策略 1：文件系统监听（debouncer 去抖动后触发索引更新）
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

            // new_debouncer 接受 DebounceEventHandler，Sender<DebounceEventResult> 实现了该 trait
            let mut debouncer = match new_debouncer(Duration::from_millis(500), tx) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("文件监听器启动失败: {}", e);
                    return;
                }
            };

            let home = dirs::home_dir().unwrap_or_default();

            // 监听所有工具的数据目录
            // Claude Code
            let _ = debouncer.watcher().watch(&claude, notify::RecursiveMode::NonRecursive);
            let projects_path = claude.join("projects");
            if projects_path.exists() {
                let _ = debouncer.watcher().watch(&projects_path, notify::RecursiveMode::Recursive);
            }
            // Codex CLI
            let codex_sessions = home.join(".codex").join("sessions");
            if codex_sessions.exists() {
                let _ = debouncer.watcher().watch(&codex_sessions, notify::RecursiveMode::Recursive);
            }
            // Gemini CLI
            let gemini_tmp = home.join(".gemini").join("tmp");
            if gemini_tmp.exists() {
                let _ = debouncer.watcher().watch(&gemini_tmp, notify::RecursiveMode::Recursive);
            }
            // OpenCode
            let opencode_dir = home.join(".local").join("share").join("opencode");
            if opencode_dir.exists() {
                let _ = debouncer.watcher().watch(&opencode_dir, notify::RecursiveMode::NonRecursive);
            }
            // Kilo Code
            let kilo_dir = home.join(".local").join("share").join("kilo");
            if kilo_dir.exists() {
                let _ = debouncer.watcher().watch(&kilo_dir, notify::RecursiveMode::NonRecursive);
            }

            loop {
                match rx.recv() {
                    // DebounceEventResult = Result<Vec<DebouncedEvent>, Error>
                    Ok(Ok(events)) => {
                        let start = Instant::now();
                        let mut files_count = 0u64;

                        let mut needs_full_rescan = false;
                        for event in &events {
                            let path = &event.path;
                            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                            let path_str = path.to_string_lossy();
                            let is_claude_path = path_str.contains(".claude");
                            let is_codex_path = path_str.contains(".codex");
                            let is_gemini_path = path_str.contains(".gemini");

                            if is_claude_path && ext == "jsonl" {
                                if path.file_name().and_then(|n| n.to_str()) == Some("history.jsonl") {
                                    needs_full_rescan = true;
                                } else if let Some(session) = scanner::scan_single_session(path) {
                                    let _ = index.lock().upsert_session(&session);
                                    files_count += 1;
                                }
                            } else if is_codex_path && ext == "jsonl" {
                                // Codex: 增量扫描单个 session 文件
                                if let Some(session) = scanner::scan_single_session(path) {
                                    let _ = index.lock().upsert_session(&session);
                                    files_count += 1;
                                }
                            } else if is_gemini_path && ext == "json" {
                                // Gemini: 增量扫描单个 session 文件
                                if let Some(session) = scanner::scan_single_session(path) {
                                    let _ = index.lock().upsert_session(&session);
                                    files_count += 1;
                                }
                            } else if ext == "db" {
                                // SQLite (OpenCode/Kilo): 全量重扫
                                needs_full_rescan = true;
                            }
                        }
                        if needs_full_rescan {
                            let sessions = scanner::scan_all_sessions();
                            let _ = index.lock().rebuild(&sessions);
                            files_count += sessions.len() as u64;
                        }

                        let elapsed = start.elapsed().as_millis() as u64;
                        stats.lock()[0].record(elapsed, files_count);
                    }
                    Ok(Err(e)) => eprintln!("监听错误: {:?}", e),
                    Err(_) => break, // 发送端关闭，退出循环
                }
            }
        });
    }

    /// 策略 2：定时轮询（检测 history.jsonl 的 mtime 变化后触发全量重建）
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

    /// 策略 3：按需扫描（弹窗打开时调用，检测 mtime 变化后同步重建）
    pub fn on_demand_refresh(
        &self,
        index: &SessionIndex,
        config: &AppConfig,
    ) -> bool {
        if !config.update.on_demand_enabled {
            return false;
        }

        let start = Instant::now();
        let home = dirs::home_dir().unwrap_or_default();

        // 检查所有工具的关键文件 mtime（取最新的）
        let check_paths = vec![
            claude_dir().join("history.jsonl"),
            home.join(".codex").join("history.jsonl"),
            home.join(".gemini").join("projects.json"),
            home.join(".local").join("share").join("opencode").join("opencode.db"),
            home.join(".local").join("share").join("kilo").join("kilo.db"),
        ];
        let current_mtime = check_paths.iter()
            .filter_map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
            .max();

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
