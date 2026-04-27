use std::process::Command;

/// 支持的终端类型
#[derive(Debug, Clone)]
pub enum TerminalKind {
    WindowsTerminal,
    PowerShell,
    Cmd,
}

/// 根据用户偏好检测终端类型，"auto" 时自动探测系统可用终端
pub fn detect_terminal(preferred: &str) -> TerminalKind {
    match preferred {
        "wt" => TerminalKind::WindowsTerminal,
        "pwsh" => TerminalKind::PowerShell,
        "cmd" => TerminalKind::Cmd,
        _ => auto_detect(),
    }
}

/// 自动探测系统可用终端：优先 Windows Terminal > PowerShell > Cmd
fn auto_detect() -> TerminalKind {
    if Command::new("where")
        .arg("wt.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return TerminalKind::WindowsTerminal;
    }
    if Command::new("where")
        .arg("pwsh.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return TerminalKind::PowerShell;
    }
    TerminalKind::Cmd
}

/// 构建 claude --resume 命令字符串（用于展示或复制）
pub fn build_resume_command(project_path: &str, session_id: &str) -> String {
    format!(
        "cd \"{}\" && claude --resume {}",
        project_path, session_id
    )
}

/// 在指定终端中恢复 Claude Code 会话
pub fn resume_in_terminal(
    terminal: &TerminalKind,
    project_path: &str,
    session_id: &str,
) -> Result<(), String> {
    let project_name = std::path::Path::new(project_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let result = match terminal {
        TerminalKind::WindowsTerminal => Command::new("wt.exe")
            .args([
                "new-tab",
                "--title",
                &format!("retalk: {}", project_name),
                "-d",
                project_path,
                "cmd",
                "/k",
                "claude",
                "--resume",
                session_id,
            ])
            .spawn(),
        TerminalKind::PowerShell => Command::new("pwsh.exe")
            .args([
                "-NoExit",
                "-Command",
                &format!(
                    "Set-Location '{}'; claude --resume {}",
                    project_path, session_id
                ),
            ])
            .spawn(),
        TerminalKind::Cmd => Command::new("cmd.exe")
            .args([
                "/k",
                &format!(
                    "cd /d \"{}\" && claude --resume {}",
                    project_path, session_id
                ),
            ])
            .spawn(),
    };

    result
        .map(|_| ())
        .map_err(|e| format!("启动终端失败: {}", e))
}
