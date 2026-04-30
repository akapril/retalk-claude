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
    MacDefault,      // Terminal.app
    MacIterm,        // iTerm2
    MacGhostty,      // Ghostty
    MacAlacritty,    // Alacritty
    MacKitty,        // Kitty
    MacWezterm,      // WezTerm
    MacRio,          // Rio
    MacWarp,         // Warp
    // Linux
    LinuxTerminal(String),
    LinuxFallback,
    // 自定义命令模板 — {cmd} 和 {dir} 会被替换
    Custom(String),
}

/// 根据用户偏好和自定义命令检测终端类型
pub fn detect_terminal_with_custom(preferred: &str, custom_command: &str) -> TerminalKind {
    if preferred == "custom" && !custom_command.is_empty() {
        return TerminalKind::Custom(custom_command.to_string());
    }
    detect_terminal(preferred)
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
        "iterm" | "iTerm" | "iTerm2" => TerminalKind::MacIterm,
        "ghostty" | "Ghostty" => TerminalKind::MacGhostty,
        "alacritty" => TerminalKind::MacAlacritty,
        "kitty" => TerminalKind::MacKitty,
        "wezterm" | "WezTerm" => TerminalKind::MacWezterm,
        "rio" | "Rio" => TerminalKind::MacRio,
        "warp" | "Warp" => TerminalKind::MacWarp,
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

/// macOS: 按优先级检测已安装的终端
#[cfg(target_os = "macos")]
fn auto_detect() -> TerminalKind {
    let checks: Vec<(&str, TerminalKind)> = vec![
        ("/Applications/Ghostty.app", TerminalKind::MacGhostty),
        ("/Applications/iTerm.app", TerminalKind::MacIterm),
        ("/Applications/Warp.app", TerminalKind::MacWarp),
        ("/Applications/kitty.app", TerminalKind::MacKitty),
        ("/Applications/Alacritty.app", TerminalKind::MacAlacritty),
        ("/Applications/WezTerm.app", TerminalKind::MacWezterm),
        ("/Applications/Rio.app", TerminalKind::MacRio),
    ];
    for (path, kind) in checks {
        if std::path::Path::new(path).exists() {
            return kind;
        }
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
        TerminalKind::MacGhostty => {
            let shell_cmd = format!("{}; exec $SHELL", unix_cmd);
            Command::new("open")
                .args(["-na", "Ghostty", "--args", "-e", "/bin/zsh", "-c", &shell_cmd])
                .spawn()
        }
        TerminalKind::MacAlacritty => {
            let shell_cmd = format!("{}; exec zsh", unix_cmd);
            Command::new("alacritty")
                .args(["-e", "/bin/zsh", "-c", &shell_cmd])
                .spawn()
        }
        TerminalKind::MacKitty => {
            let shell_cmd = format!("{}; exec zsh", unix_cmd);
            Command::new("kitty")
                .args(["--hold", "zsh", "-c", &shell_cmd])
                .spawn()
        }
        TerminalKind::MacWezterm => {
            let shell_cmd = format!("{}; exec zsh", unix_cmd);
            Command::new("wezterm")
                .args(["start", "--cwd", project_path, "--", "zsh", "-c", &shell_cmd])
                .spawn()
        }
        TerminalKind::MacRio => {
            let shell_cmd = format!("{}; exec zsh", unix_cmd);
            Command::new("rio")
                .args(["-w", project_path, "-e", "zsh", "-c", &shell_cmd])
                .spawn()
        }
        TerminalKind::MacWarp => {
            // Warp 通过 URI scheme 打开，然后 osascript 输入命令
            let _ = Command::new("open")
                .args([&format!("warp://action/new_window?path={}", project_path)])
                .spawn();
            std::thread::sleep(std::time::Duration::from_millis(800));
            let script = format!(
                "tell application \"System Events\" to tell process \"Warp\"\n\
                    keystroke \"{}\"\n\
                    key code 36\n\
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

        // === 自定义终端 ===
        TerminalKind::Custom(template) => {
            // 模板中 {cmd} 替换为完整命令，{dir} 替换为项目目录
            let full_cmd = build_resume_command(provider, project_path, session_id);
            let resolved = template
                .replace("{cmd}", &full_cmd)
                .replace("{dir}", project_path);
            // 用 shell 执行解析后的命令
            if cfg!(windows) {
                Command::new("cmd.exe")
                    .args(["/c", &resolved])
                    .spawn()
            } else {
                Command::new("sh")
                    .args(["-c", &resolved])
                    .spawn()
            }
        }
    };

    result
        .map(|_| ())
        .map_err(|e| format!("启动终端失败: {}", e))
}
