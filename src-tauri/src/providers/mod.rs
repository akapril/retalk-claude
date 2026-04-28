pub mod claude;
pub mod codex;
pub mod gemini;
pub mod opencode;

use crate::models::Session;

/// 所有 AI 编码工具的会话扫描 trait
pub trait SessionProvider {
    /// provider 名称
    fn name(&self) -> &str;
    /// 检测本机是否安装了该工具
    fn is_available(&self) -> bool;
    /// 扫描全部会话
    fn scan_all(&self) -> Vec<Session>;
}

/// 获取所有已安装的 provider
pub fn all_providers() -> Vec<Box<dyn SessionProvider>> {
    let providers: Vec<Box<dyn SessionProvider>> = vec![
        Box::new(claude::ClaudeProvider),
        Box::new(codex::CodexProvider),
        Box::new(gemini::GeminiProvider),
        Box::new(opencode::OpenCodeProvider),
    ];
    providers.into_iter().filter(|p| p.is_available()).collect()
}
