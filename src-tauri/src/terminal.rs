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
        "gemini" => format!("gemini --resume {}", session_id),
        "opencode" => format!("opencode --session {}", session_id),
        "kilo" => format!("kilo --session {}", session_id),
        _ => String::new(),
    };
    if resume_cmd.is_empty() {
        format!("cd /d \"{}\"", project_path)
    } else {
        format!("cd /d \"{}\" && {}", project_path, resume_cmd)
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
        "gemini" => format!("gemini --resume {}", session_id),
        "opencode" => format!("opencode --session {}", session_id),
        "kilo" => format!("kilo --session {}", session_id),
        _ => String::new(),
    };

    let result = match terminal {
        TerminalKind::WindowsTerminal => {
            // wt.exe 对含中文/空格的路径需要用完整的命令行字符串
            // 使用 cmd /k 统一处理 cd 和恢复命令
            let full_cmd = build_resume_command(provider, project_path, session_id);
            // wt 的 commandline 参数用 -- 分隔
            Command::new("wt.exe")
                .args([
                    "new-tab",
                    "--title",
                    &format!("retalk: {}", project_name),
                    "cmd",
                    "/k",
                    &full_cmd,
                ])
                .spawn()
        }
        TerminalKind::PowerShell => {
            // PowerShell 用 Set-Location 处理含特殊字符的路径
            let ps_cmd = if tool_cmd.is_empty() {
                format!("Set-Location -LiteralPath '{}'", project_path)
            } else {
                format!("Set-Location -LiteralPath '{}'; {}", project_path, tool_cmd)
            };
            Command::new("pwsh.exe")
                .args(["-NoExit", "-Command", &ps_cmd])
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
