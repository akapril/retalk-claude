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

/// 根据 provider 构建恢复命令字符串（用于展示或复制）
pub fn build_resume_command(provider: &str, project_path: &str, session_id: &str) -> String {
    let resume_cmd = match provider {
        "claude" => format!("claude --resume {}", session_id),
        "codex" => format!("codex resume {}", session_id),
        _ => String::new(),
    };
    if resume_cmd.is_empty() {
        format!("cd \"{}\"", project_path)
    } else {
        format!("cd \"{}\" && {}", project_path, resume_cmd)
    }
}

/// 在指定终端中恢复 AI 编码工具会话
pub fn resume_in_terminal(
    terminal: &TerminalKind,
    provider: &str,
    project_path: &str,
    session_id: &str,
) -> Result<(), String> {
    let project_name = std::path::Path::new(project_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    // 根据 provider 生成工具恢复命令（不含 cd）
    let tool_cmd = match provider {
        "claude" => format!("claude --resume {}", session_id),
        "codex" => format!("codex resume {}", session_id),
        _ => String::new(),
    };

    let result = match terminal {
        TerminalKind::WindowsTerminal => {
            // wt 使用 -d 设置工作目录，cmd /k 执行工具命令
            let cmd_arg = if tool_cmd.is_empty() {
                // 无恢复命令时只打开目录
                String::new()
            } else {
                tool_cmd.clone()
            };
            Command::new("wt.exe")
                .args([
                    "new-tab",
                    "--title",
                    &format!("retalk: {}", project_name),
                    "-d",
                    project_path,
                    "cmd",
                    "/k",
                    &cmd_arg,
                ])
                .spawn()
        }
        TerminalKind::PowerShell => {
            let full_cmd = build_resume_command(provider, project_path, session_id);
            Command::new("pwsh.exe")
                .args(["-NoExit", "-Command", &full_cmd])
                .spawn()
        }
        TerminalKind::Cmd => {
            let full_cmd = build_resume_command(provider, project_path, session_id);
            Command::new("cmd.exe")
                .args(["/k", &full_cmd])
                .spawn()
        }
    };

    result
        .map(|_| ())
        .map_err(|e| format!("启动终端失败: {}", e))
}
