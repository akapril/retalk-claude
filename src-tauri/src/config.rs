use crate::models::AppConfig;
use std::fs;
use std::path::PathBuf;

/// 获取 retalk 数据目录：~/.claude/retalk/
pub fn retalk_dir() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".claude")
        .join("retalk")
}

/// 获取 Claude Code 数据目录：~/.claude/
pub fn claude_dir() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".claude")
}

/// 配置文件路径
fn config_path() -> PathBuf {
    retalk_dir().join("config.toml")
}

/// 加载配置，不存在则创建默认配置
pub fn load_config() -> AppConfig {
    let path = config_path();
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        toml::from_str(&content).unwrap_or_default()
    } else {
        let config = AppConfig::default();
        save_config(&config);
        config
    }
}

/// 保存配置到文件
pub fn save_config(config: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = toml::to_string_pretty(config).expect("序列化配置失败");
    let _ = fs::write(&path, content);
}
