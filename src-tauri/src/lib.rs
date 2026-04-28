mod commands;
mod config;
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

    // 初始化索引
    let index = SessionIndex::new().expect("Tantivy 索引初始化失败");

    // 全量扫描并建立索引
    let sessions = scanner::scan_all_sessions();
    let _ = index.rebuild(&sessions);

    let index = Arc::new(Mutex::new(index));
    let updater_instance = Arc::new(updater::Updater::new());

    // 启动后台更新策略
    updater_instance.start_watcher(Arc::clone(&index), &app_config);
    updater_instance.start_poll(Arc::clone(&index), &app_config);

    // 加载收藏和标签
    let favorites = commands::load_favorites();
    let tags = commands::load_tags();

    let state = AppState {
        index,
        updater: updater_instance,
        config: Arc::new(Mutex::new(app_config)),
        sessions: Arc::new(Mutex::new(sessions)),
        favorites: Arc::new(Mutex::new(favorites)),
        tags: Arc::new(Mutex::new(tags)),
    };

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
                .icon(app.default_window_icon().unwrap().clone())
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
        ])
        .run(tauri::generate_context!())
        .expect("启动 retalk 失败");
}
