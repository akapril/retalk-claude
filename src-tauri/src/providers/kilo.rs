use super::SessionProvider;
use crate::models::Session;
use crate::providers::opencode;
use std::path::PathBuf;

pub struct KiloProvider;

/// Kilo Code 数据库路径：~/.local/share/kilo/kilo.db
fn kilo_db_path() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".local")
        .join("share")
        .join("kilo")
        .join("kilo.db")
}

impl SessionProvider for KiloProvider {
    fn name(&self) -> &str {
        "kilo"
    }

    fn is_available(&self) -> bool {
        kilo_db_path().exists()
    }

    fn scan_all(&self) -> Vec<Session> {
        // 表结构与 OpenCode 相同，复用扫描逻辑
        let mut sessions = opencode::scan_sqlite_sessions(&kilo_db_path(), "kilo");
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }
}
