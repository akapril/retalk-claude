mod config;
mod indexer;
mod models;
mod scanner;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("启动 retalk 失败");
}
