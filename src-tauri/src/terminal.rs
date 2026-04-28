use std::process::Command;

/// 创建隐藏窗口的 Command（Windows 上不闪 cmd 窗口）
#[cfg(windows)]
fn silent_command(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new(program);
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    cmd
}

#[cfg(not(windows))]
fn silent_command(program: &str) -> Command {
    Command::new(program)
}

/// 支持的终端类型（跨平台）
#[derive(Debug, Clone)]
#[allow(dead_code)] // 各平台仅使用自身的变体
pub enum TerminalKind {
    // Windows
    WindowsTerminal,
    PowerShell,
    Cmd,
    // macOS
    MacDefault,
    MacIterm,
    // Linux
    LinuxTerminal(String), // gnome-terminal, konsole, alacritty, kitty 等
    LinuxFallback,         // xterm
}

/// 根据用户偏好检测终端类型，"auto" 时自动探测系统可用终端
pub fn detect_terminal(preferred: &str) -> TerminalKind {
    match preferred {
        // Windows
        "wt" => TerminalKind::WindowsTerminal,
        "pwsh" => TerminalKind::PowerShell,
        "cmd" => TerminalKind::Cmd,
        // macOS
        "terminal" | "Terminal" => TerminalKind::MacDefault,
        "iterm" | "iTerm" => TerminalKind::MacIterm,
        // Linux — 用户可直接指定终端名称
        #[cfg(target_os = "linux")]
        name if !name.is_empty() && name != "auto" => TerminalKind::LinuxTerminal(name.to_string()),
        _ => auto_detect(),
    }
}

/// Windows: 优先 Windows Terminal > PowerShell > Cmd
#[cfg(windows)]
fn auto_detect() -> TerminalKind {
    if silent_command("where")
        .arg("wt.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return TerminalKind::WindowsTerminal;
    }
    if silent_command("where")
        .arg("pwsh.exe")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return TerminalKind::PowerShell;
    }
    TerminalKind::Cmd
}

/// macOS: 默认使用 Terminal.app，可检测 iTerm2
#[cfg(target_os = "macos")]
fn auto_detect() -> TerminalKind {
    // 检测 iTerm2 是否安装
    if std::path::Path::new("/Applications/iTerm.app").exists() {
        return TerminalKind::MacIterm;
    }
    TerminalKind::MacDefault
}

/// Linux: 探测常见终端仿真器
#[cfg(target_os = "linux")]
fn auto_detect() -> TerminalKind {
    for term in &[
        "gnome-terminal",
        "konsole",
        "alacritty",
        "kitty",
        "xfce4-terminal",
        "xterm",
    ] {
        if silent_command("which")
            .arg(term)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return TerminalKind::LinuxTerminal(term.to_string());
        }
    }
    TerminalKind::LinuxFallback
}

/// 兜底：其他 Unix 类平台回退到 xterm
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn auto_detect() -> TerminalKind {
    TerminalKind::LinuxFallback
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

    // Windows 使用 cd /d 支持跨盘符切换，Unix 使用 cd
    let cd_cmd = if cfg!(windows) {
        format!("cd /d \"{}\"", project_path)
    } else {
        format!("cd \"{}\"", project_path)
    };

    if resume_cmd.is_empty() {
        cd_cmd
    } else {
        format!("{} && {}", cd_cmd, resume_cmd)
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

    // Unix 系统的 cd + 恢复命令
    let unix_cmd = if tool_cmd.is_empty() {
        format!("cd \"{}\"", project_path)
    } else {
        format!("cd \"{}\" && {}", project_path, tool_cmd)
    };

    let result = match terminal {
        // === Windows ===
        TerminalKind::WindowsTerminal => {
            let full_cmd = build_resume_command(provider, project_path, session_id);
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
            let ps_cmd = if tool_cmd.is_empty() {
                format!("Set-Location -LiteralPath '{}'", project_path)
            } else {
                format!("Set-Location -LiteralPath '{}'; {}", project_path, tool_cmd)
            };
            // Windows 使用 pwsh.exe，macOS/Linux 使用 pwsh
            let pwsh = if cfg!(windows) { "pwsh.exe" } else { "pwsh" };
            Command::new(pwsh)
                .args(["-NoExit", "-Command", &ps_cmd])
                .spawn()
        }
        TerminalKind::Cmd => {
            let full_cmd = build_resume_command(provider, project_path, session_id);
            Command::new("cmd.exe")
                .args(["/k", &full_cmd])
                .spawn()
        }

        // === macOS ===
        TerminalKind::MacDefault => {
            // 使用 osascript 在 Terminal.app 中打开新标签页并执行命令
            let script = format!(
                "tell application \"Terminal\"\n\
                    activate\n\
                    do script \"{}\"\n\
                end tell",
                unix_cmd.replace("\"", "\\\"")
            );
            Command::new("osascript")
                .args(["-e", &script])
                .spawn()
        }
        TerminalKind::MacIterm => {
            // 使用 osascript 在 iTerm2 中打开新标签页并执行命令
            let script = format!(
                "tell application \"iTerm\"\n\
                    activate\n\
                    tell current window\n\
                        create tab with default profile\n\
                        tell current session\n\
                            write text \"{}\"\n\
                        end tell\n\
                    end tell\n\
                end tell",
                unix_cmd.replace("\"", "\\\"")
            );
            Command::new("osascript")
                .args(["-e", &script])
                .spawn()
        }

        // === Linux ===
        TerminalKind::LinuxTerminal(term) => {
            // 不同终端的参数格式略有不同
            let shell_cmd = format!("{}; exec $SHELL", unix_cmd);
            match term.as_str() {
                "gnome-terminal" => Command::new(term)
                    .args(["--", "bash", "-c", &shell_cmd])
                    .spawn(),
                "konsole" => Command::new(term)
                    .args(["-e", "bash", "-c", &shell_cmd])
                    .spawn(),
                "xfce4-terminal" => Command::new(term)
                    .args(["-e", &format!("bash -c '{}'", shell_cmd)])
                    .spawn(),
                "alacritty" => Command::new(term)
                    .args(["-e", "bash", "-c", &shell_cmd])
                    .spawn(),
                "kitty" => Command::new(term)
                    .args(["bash", "-c", &shell_cmd])
                    .spawn(),
                _ => Command::new(term)
                    .args(["-e", "bash", "-c", &shell_cmd])
                    .spawn(),
            }
        }
        TerminalKind::LinuxFallback => {
            let shell_cmd = format!("{}; exec $SHELL", unix_cmd);
            Command::new("xterm")
                .args(["-e", "bash", "-c", &shell_cmd])
                .spawn()
        }
    };

    result
        .map(|_| ())
        .map_err(|e| format!("启动终端失败: {}", e))
}
