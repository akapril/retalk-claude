mod commands;
mod config;
mod ecosystem;
mod indexer;
mod models;
mod providers;
mod scanner;
mod searcher;
mod terminal;
mod updater;

use commands::AppState;
use indexer::SessionIndex;
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

    // 初始化索引（尝试打开已有索引）
    let index = SessionIndex::new().expect("Tantivy 索引初始化失败");
    let has_existing_data = index.doc_count() > 0;
    let index = Arc::new(Mutex::new(index));
    let updater_instance = Arc::new(updater::Updater::new());

    // 加载收藏和标签
    let favorites = commands::load_favorites();
    let tags = commands::load_tags();
    let notes = commands::load_notes();

    // 如果有已有索引数据，立即标记就绪 + 初始化 mtime（防止 on_demand_refresh 触发全量扫描）
    let ready = Arc::new(std::sync::atomic::AtomicBool::new(has_existing_data));
    if has_existing_data {
        updater_instance.init_mtime_snapshot();
        eprintln!("[retalk] 已有索引数据，UI 立即可用");
    }

    let state = AppState {
        index: Arc::clone(&index),
        updater: Arc::clone(&updater_instance),
        config: Arc::new(Mutex::new(app_config.clone())),
        sessions: Arc::new(Mutex::new(Vec::new())),
        favorites: Arc::new(Mutex::new(favorites)),
        tags: Arc::new(Mutex::new(tags)),
        notes: Arc::new(Mutex::new(notes)),
        ready: Arc::clone(&ready),
    };

    // 后台线程：增量同步 + 启动更新策略
    let bg_index = Arc::clone(&index);
    let bg_updater = Arc::clone(&updater_instance);
    let bg_sessions = Arc::clone(&state.sessions);
    let bg_config = app_config.clone();
    let bg_ready = Arc::clone(&ready);
    std::thread::spawn(move || {
        eprintln!("[retalk] 后台扫描开始...");
        let sessions = scanner::scan_all_sessions();
        eprintln!("[retalk] 扫描完成，共 {} 条会话", sessions.len());

        {
            let idx = bg_index.lock();
            if has_existing_data {
                // 增量同步：只更新有变化的
                let _ = idx.incremental_sync(&sessions);
            } else {
                // 首次：全量建索引
                let _ = idx.rebuild(&sessions);
            }
        }

        *bg_sessions.lock() = sessions;
        bg_updater.init_mtime_snapshot();
        bg_ready.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("[retalk] 数据同步完成");

        // 启动后台更新策略
        bg_updater.start_watcher(Arc::clone(&bg_index), &bg_config);
        bg_updater.start_poll(Arc::clone(&bg_index), &bg_config);
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(|app| {
            // === 系统托盘 ===
            let show_i = MenuItem::with_id(app, "show", "显示/隐藏", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/icon.png"))
                .menu(&menu)
                .tooltip("retalk - Claude Code 会话管理")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_window(app),
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

            // === 窗口失焦自动隐藏 ===
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                    }
                });
            }

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
            commands::get_session_preview,
            commands::get_project_git_info,
            commands::toggle_favorite,
            commands::get_favorites,
            commands::open_in_vscode,
            commands::open_in_explorer,
            commands::set_tags,
            commands::get_all_tags,
            commands::export_session_markdown,
            commands::export_session_to_file,
            commands::get_desktop_path,
            commands::open_in_explorer_select,
            commands::is_ready,
            commands::get_provider_status,
            commands::batch_export_markdown,
            commands::auto_tag_sessions,
            commands::set_autostart,
            commands::get_autostart,
            commands::rebuild_index,
            commands::set_note,
            commands::get_all_notes,
            commands::get_ecosystem,
            commands::toggle_mcp_server,
            commands::plugin_toggle,
            commands::plugin_install,
            commands::plugin_uninstall,
            commands::plugin_update,
            commands::add_mcp_server_cmd,
            commands::remove_mcp_server,
            commands::codex_mcp_add,
            commands::codex_mcp_remove,
            commands::gemini_ext_install,
            commands::gemini_ext_toggle,
            commands::gemini_ext_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("启动 retalk 失败");
}
