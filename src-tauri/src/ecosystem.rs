use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct EcosystemData {
    pub skills: Vec<SkillInfo>,
    pub mcp_servers: Vec<McpServerInfo>,
    pub overview: Vec<ToolOverview>,
    pub plugins: Vec<PluginInfo>,
    pub available_plugins: Vec<AvailablePlugin>,
    pub extensions: Vec<ExtensionInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolOverview {
    pub name: String,
    pub installed: bool,
    pub version: String,
    pub data_dir: String,
    pub session_count: u32,
    pub mcp_count: u32,
    pub skill_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtensionInfo {
    pub tool: String,      // "gemini"
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub source: String,    // 安装来源（git URL 或本地路径）
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailablePlugin {
    pub name: String,
    pub marketplace: String,
    pub description: String,
    /// 完整标识符: name@marketplace，用于安装命令
    pub full_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub marketplace: String,
    pub version: String,
    pub description: String,
    pub scope: String,
    pub installed_at: String,
    pub install_path: String,
    pub has_skills: bool,
    pub has_mcp: bool,
    pub skill_count: u32,
    pub enabled: bool,
    /// 完整标识符: name@marketplace
    pub full_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillInfo {
    pub tool: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerInfo {
    pub tool: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub source: String,
}

/// 扫描所有 AI 编码工具的生态信息
pub fn scan_ecosystem() -> EcosystemData {
    let mut skills = Vec::new();
    let mut mcp_servers = Vec::new();

    // Claude Code
    scan_claude_skills(&mut skills);
    scan_claude_mcp(&mut mcp_servers);

    // Codex CLI
    scan_codex_mcp(&mut mcp_servers);

    // Gemini CLI
    scan_gemini_mcp(&mut mcp_servers);

    // OpenCode
    scan_opencode_mcp(&mut mcp_servers);

    // Kilo Code
    scan_kilo_mcp(&mut mcp_servers);

    // Claude Code 插件
    let plugins = scan_claude_plugins();
    let available_plugins = scan_available_plugins(&plugins);

    // Gemini 扩展
    let extensions = scan_gemini_extensions();

    // 工具概览
    let overview = build_overview(&skills, &mcp_servers, &plugins, &extensions);

    EcosystemData {
        skills,
        mcp_servers,
        overview,
        plugins,
        available_plugins,
        extensions,
    }
}

/// 构建各工具概览信息
fn build_overview(
    skills: &[SkillInfo],
    mcp_servers: &[McpServerInfo],
    _plugins: &[PluginInfo],
    _extensions: &[ExtensionInfo],
) -> Vec<ToolOverview> {
    use std::process::Command;
    let home = dirs::home_dir().unwrap_or_default();

    let tools = vec![
        ("Claude Code", "claude", home.join(".claude")),
        ("Codex CLI", "codex", home.join(".codex")),
        ("Gemini CLI", "gemini", home.join(".gemini")),
        ("OpenCode", "opencode", home.join(".local").join("share").join("opencode")),
        ("Kilo Code", "kilo", home.join(".local").join("share").join("kilo")),
    ];

    tools.iter().map(|(display_name, cmd, data_dir)| {
        // 检测是否安装并获取版本
        let (installed, version) = get_tool_version(cmd);

        // 统计数量
        let mcp_count = mcp_servers.iter().filter(|s| s.tool == *cmd).count() as u32;
        let skill_count = if *cmd == "claude" {
            skills.len() as u32
        } else {
            0
        };

        // 统计会话数（检查数据目录大小/文件数）
        let session_count = if *cmd == "claude" {
            count_claude_sessions(&home)
        } else if data_dir.exists() {
            count_files_in_dir(data_dir, "jsonl") + count_files_in_dir(data_dir, "json")
        } else {
            0
        };

        ToolOverview {
            name: display_name.to_string(),
            installed,
            version,
            data_dir: data_dir.to_string_lossy().to_string(),
            session_count,
            mcp_count,
            skill_count,
        }
    }).collect()
}

fn get_tool_version(cmd: &str) -> (bool, String) {
    #[cfg(windows)]
    fn make_cmd(program: &str) -> std::process::Command {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(program);
        cmd.creation_flags(0x08000000);
        cmd
    }
    #[cfg(not(windows))]
    fn make_cmd(program: &str) -> std::process::Command {
        std::process::Command::new(program)
    }

    match make_cmd(cmd).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let ver = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            (true, ver)
        }
        _ => (false, String::new()),
    }
}

fn count_claude_sessions(home: &std::path::Path) -> u32 {
    let projects_dir = home.join(".claude").join("projects");
    if !projects_dir.exists() { return 0; }
    let mut count = 0u32;
    if let Ok(dirs) = std::fs::read_dir(&projects_dir) {
        for entry in dirs.flatten() {
            if entry.path().is_dir() {
                if let Ok(files) = std::fs::read_dir(entry.path()) {
                    count += files.flatten()
                        .filter(|f| f.path().extension().and_then(|e| e.to_str()) == Some("jsonl"))
                        .count() as u32;
                }
            }
        }
    }
    count
}

fn count_files_in_dir(dir: &std::path::Path, ext: &str) -> u32 {
    if !dir.exists() { return 0; }
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten()
            .filter(|f| f.path().extension().and_then(|e| e.to_str()) == Some(ext))
            .count() as u32)
        .unwrap_or(0)
}

// === Claude Code ===

fn scan_claude_skills(skills: &mut Vec<SkillInfo>) {
    let home = dirs::home_dir().unwrap_or_default();
    let claude_dir = home.join(".claude");

    // 全局 skills: ~/.claude/skills/
    scan_skill_dir(&claude_dir.join("skills"), "claude", "global", skills);

    // 兼容旧版 commands 目录
    scan_skill_dir(&claude_dir.join("commands"), "claude", "global", skills);

    // 插件内的 skills: ~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/skills/
    let plugins_cache = claude_dir.join("plugins").join("cache");
    if plugins_cache.exists() {
        scan_plugins_dir(&plugins_cache, skills);
    }

    // 独立安装的插件: ~/.claude/plugins/<plugin-name>/skills/
    let plugins_dir = claude_dir.join("plugins");
    if plugins_dir.exists() {
        if let Ok(entries) = fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                // 跳过非目录和 cache 等已知子目录
                if !path.is_dir() { continue; }
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                if name == "cache" || name == "marketplaces" || name == "data" { continue; }
                // 检查是否有 skills 子目录
                let skills_dir = path.join("skills");
                if skills_dir.exists() {
                    scan_skill_dir(&skills_dir, "claude", &format!("plugin:{}", name), skills);
                }
            }
        }
    }
}

/// 递归扫描 plugins/cache 目录，查找所有 skills
fn scan_plugins_dir(cache_dir: &PathBuf, skills: &mut Vec<SkillInfo>) {
    // 结构: cache/<marketplace>/<plugin>/<version>/skills/<skill-name>/SKILL.md
    let marketplaces = match fs::read_dir(cache_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for mp_entry in marketplaces.flatten() {
        let mp_path = mp_entry.path();
        if !mp_path.is_dir() { continue; }
        let plugins = match fs::read_dir(&mp_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for plugin_entry in plugins.flatten() {
            let plugin_path = plugin_entry.path();
            if !plugin_path.is_dir() { continue; }
            let plugin_name = plugin_path.file_name().unwrap_or_default().to_string_lossy().to_string();
            // 找最新版本目录（按名称排序取最后一个）
            let mut versions: Vec<_> = fs::read_dir(&plugin_path)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().is_dir())
                .collect();
            versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
            if let Some(version_entry) = versions.first() {
                let skills_dir = version_entry.path().join("skills");
                if skills_dir.exists() {
                    scan_skill_dir(&skills_dir, "claude", &format!("plugin:{}", plugin_name), skills);
                }
            }
        }
    }
}

fn scan_skill_dir(dir: &PathBuf, tool: &str, scope: &str, skills: &mut Vec<SkillInfo>) {
    if !dir.exists() {
        return;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // 检查目录内的 SKILL.md
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let desc = extract_skill_description(&skill_file);
                    skills.push(SkillInfo {
                        tool: tool.to_string(),
                        name,
                        description: desc,
                        path: skill_file.to_string_lossy().to_string(),
                        scope: scope.to_string(),
                    });
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let desc = extract_skill_description(&path);
                skills.push(SkillInfo {
                    tool: tool.to_string(),
                    name,
                    description: desc,
                    path: path.to_string_lossy().to_string(),
                    scope: scope.to_string(),
                });
            }
        }
    }
}

/// 从 Skill 文件中提取描述（YAML frontmatter 或首段文本）
fn extract_skill_description(path: &PathBuf) -> String {
    let content = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().take(10).collect();

    let mut in_frontmatter = false;
    for line in &lines {
        if *line == "---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }
        if in_frontmatter {
            if let Some(desc) = line.strip_prefix("description:") {
                return desc.trim().trim_matches('"').to_string();
            }
        }
        // 无 frontmatter 时，取第一行非空、非标题的文本
        if !in_frontmatter && !line.is_empty() && !line.starts_with('#') && !line.starts_with("---")
        {
            return line.to_string();
        }
    }
    String::new()
}

fn scan_claude_mcp(servers: &mut Vec<McpServerInfo>) {
    let home = dirs::home_dir().unwrap_or_default();

    // 1. 全局配置: ~/.claude/settings.json
    let settings_path = home.join(".claude").join("settings.json");
    if let Some(data) = read_json_file(&settings_path) {
        if let Some(mcp) = data.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, config) in mcp {
                servers.push(parse_mcp_entry("claude", name, config, &settings_path));
            }
        }
    }

    // 2. 插件内的 .mcp.json: ~/.claude/plugins/cache/<mp>/<plugin>/<version>/.mcp.json
    let plugins_cache = home.join(".claude").join("plugins").join("cache");
    if plugins_cache.exists() {
        scan_plugin_mcp_files(&plugins_cache, servers);
    }

    // 3. 独立插件: ~/.claude/plugins/<plugin>/.mcp.json
    let plugins_dir = home.join(".claude").join("plugins");
    if plugins_dir.exists() {
        if let Ok(entries) = fs::read_dir(&plugins_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                if name == "cache" || name == "marketplaces" || name == "data" { continue; }
                let mcp_file = path.join(".mcp.json");
                if mcp_file.exists() {
                    if let Some(data) = read_json_file(&mcp_file) {
                        if let Some(mcp) = data.get("mcpServers").and_then(|v| v.as_object()) {
                            for (sname, config) in mcp {
                                servers.push(parse_mcp_entry("claude", sname, config, &mcp_file));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 扫描 plugins/cache 下所有插件的 .mcp.json
fn scan_plugin_mcp_files(cache_dir: &PathBuf, servers: &mut Vec<McpServerInfo>) {
    let marketplaces = match fs::read_dir(cache_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for mp_entry in marketplaces.flatten() {
        let mp_path = mp_entry.path();
        if !mp_path.is_dir() { continue; }
        let plugins = match fs::read_dir(&mp_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for plugin_entry in plugins.flatten() {
            let plugin_path = plugin_entry.path();
            if !plugin_path.is_dir() { continue; }
            // 找最新版本
            let mut versions: Vec<_> = fs::read_dir(&plugin_path)
                .into_iter().flatten().flatten()
                .filter(|e| e.path().is_dir())
                .collect();
            versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
            if let Some(version_entry) = versions.first() {
                let mcp_file = version_entry.path().join(".mcp.json");
                if mcp_file.exists() {
                    if let Some(data) = read_json_file(&mcp_file) {
                        if let Some(mcp) = data.get("mcpServers").and_then(|v| v.as_object()) {
                            for (name, config) in mcp {
                                servers.push(parse_mcp_entry("claude", name, config, &mcp_file));
                            }
                        }
                    }
                }
            }
        }
    }
}

// === Codex CLI ===

fn scan_codex_mcp(servers: &mut Vec<McpServerInfo>) {
    let home = dirs::home_dir().unwrap_or_default();
    let config_path = home.join(".codex").join("config.toml");
    if !config_path.exists() {
        return;
    }

    let content = fs::read_to_string(&config_path).unwrap_or_default();
    if let Ok(toml_val) = content.parse::<toml::Value>() {
        if let Some(mcp) = toml_val.get("mcp_servers").and_then(|v| v.as_table()) {
            for (name, config) in mcp {
                let command = config
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args: Vec<String> = config
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| a.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let enabled = config
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                servers.push(McpServerInfo {
                    tool: "codex".to_string(),
                    name: name.clone(),
                    command,
                    args,
                    enabled,
                    source: config_path.to_string_lossy().to_string(),
                });
            }
        }
    }
}

// === Gemini CLI ===

fn scan_gemini_mcp(servers: &mut Vec<McpServerInfo>) {
    let home = dirs::home_dir().unwrap_or_default();
    let settings_path = home.join(".gemini").join("settings.json");
    if let Some(data) = read_json_file(&settings_path) {
        if let Some(mcp) = data.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, config) in mcp {
                servers.push(parse_mcp_entry("gemini", name, config, &settings_path));
            }
        }
    }
}


// === OpenCode ===

fn scan_opencode_mcp(servers: &mut Vec<McpServerInfo>) {
    let home = dirs::home_dir().unwrap_or_default();
    for path in &[
        home.join(".config")
            .join("opencode")
            .join("opencode.json"),
        home.join(".config")
            .join("opencode")
            .join("opencode.jsonc"),
    ] {
        if let Some(data) = read_json_file(path) {
            if let Some(mcp) = data.get("mcp").and_then(|v| v.as_object()) {
                for (name, config) in mcp {
                    servers.push(parse_mcp_entry("opencode", name, config, path));
                }
            }
        }
    }
}

// === Kilo Code ===

fn scan_kilo_mcp(servers: &mut Vec<McpServerInfo>) {
    let home = dirs::home_dir().unwrap_or_default();
    for path in &[
        home.join(".config").join("kilo").join("kilo.jsonc"),
        home.join(".config").join("kilo").join("kilo.json"),
    ] {
        if let Some(data) = read_json_file(path) {
            if let Some(mcp) = data.get("mcp").and_then(|v| v.as_object()) {
                for (name, config) in mcp {
                    servers.push(parse_mcp_entry("kilo", name, config, path));
                }
            }
        }
    }
}

// === 辅助函数 ===

/// 读取 JSON/JSONC 文件并解析
fn read_json_file(path: &PathBuf) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    let cleaned = strip_jsonc_comments(&content);
    serde_json::from_str(&cleaned).ok()
}

/// 去除 JSONC 注释（// 行注释 和 /* */ 块注释）
fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            result.push(c);
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    result.push(next);
                    chars.next();
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            result.push(c);
        } else if c == '/' {
            if let Some(&next) = chars.peek() {
                if next == '/' {
                    // 行注释 — 跳到行末
                    while let Some(nc) = chars.next() {
                        if nc == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                } else if next == '*' {
                    // 块注释 — 跳到 */
                    chars.next();
                    while let Some(nc) = chars.next() {
                        if nc == '*' {
                            if let Some(&'/') = chars.peek() {
                                chars.next();
                                break;
                            }
                        }
                    }
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// 从 JSON 配置解析 MCP 服务器条目
fn parse_mcp_entry(
    tool: &str,
    name: &str,
    config: &serde_json::Value,
    source: &PathBuf,
) -> McpServerInfo {
    let command = config
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let args: Vec<String> = config
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    // disabled 字段为 true 表示已禁用
    let disabled = config
        .get("disabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let enabled = !disabled;

    McpServerInfo {
        tool: tool.to_string(),
        name: name.to_string(),
        command,
        args,
        enabled,
        source: source.to_string_lossy().to_string(),
    }
}

// === MCP 服务器启禁切换 ===

/// 切换 Claude Code settings.json 中 MCP 服务器的启用状态
/// 切换 MCP 服务器启禁状态（通过修改其所在的配置文件）
pub fn toggle_mcp_in_file(source_path: &str, server_name: &str, enabled: bool) -> Result<(), String> {
    let path = std::path::Path::new(source_path);
    let content = fs::read_to_string(path).map_err(|e| format!("读取失败: {}", e))?;
    let mut data: serde_json::Value = serde_json::from_str(&content).map_err(|e| format!("解析失败: {}", e))?;

    if let Some(servers) = data.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        if let Some(server) = servers.get_mut(server_name) {
            if let Some(obj) = server.as_object_mut() {
                if enabled {
                    obj.remove("disabled");
                } else {
                    obj.insert("disabled".to_string(), serde_json::Value::Bool(true));
                }
            }
        }
    }

    let output = serde_json::to_string_pretty(&data).map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(path, output).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

// ============================================================
// Claude Code 插件管理
// ============================================================

/// 扫描已安装的 Claude Code 插件
fn scan_claude_plugins() -> Vec<PluginInfo> {
    let home = dirs::home_dir().unwrap_or_default();
    let plugins_file = home.join(".claude").join("plugins").join("installed_plugins.json");

    let content = match fs::read_to_string(&plugins_file) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let data: serde_json::Value = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    // 读取 enabled 状态: ~/.claude/settings.json -> enabledPlugins
    let settings_path = home.join(".claude").join("settings.json");
    let enabled_map: std::collections::HashMap<String, bool> = fs::read_to_string(&settings_path)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|d| d.get("enabledPlugins").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let mut plugins = Vec::new();

    if let Some(plugin_map) = data.get("plugins").and_then(|v| v.as_object()) {
        for (key, installs) in plugin_map {
            let parts: Vec<&str> = key.splitn(2, '@').collect();
            let plugin_name = parts.first().unwrap_or(&"").to_string();
            let marketplace = parts.get(1).unwrap_or(&"").to_string();

            if let Some(install_arr) = installs.as_array() {
                for inst in install_arr {
                    let version = inst.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let scope = inst.get("scope").and_then(|v| v.as_str()).unwrap_or("user").to_string();
                    let installed_at = inst.get("installedAt").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let install_path = inst.get("installPath").and_then(|v| v.as_str()).unwrap_or("").to_string();

                    // 读取 plugin.json 获取描述
                    let description = if !install_path.is_empty() {
                        let plugin_json = std::path::Path::new(&install_path).join(".claude-plugin").join("plugin.json");
                        fs::read_to_string(&plugin_json)
                            .ok()
                            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                            .and_then(|d| d.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    // 检查是否有 skills 和 mcp
                    let has_skills = if !install_path.is_empty() {
                        std::path::Path::new(&install_path).join("skills").exists()
                            || std::path::Path::new(&install_path).join(".claude-plugin").join("skills").exists()
                    } else {
                        false
                    };

                    let has_mcp = if !install_path.is_empty() {
                        std::path::Path::new(&install_path).join(".mcp.json").exists()
                            || std::path::Path::new(&install_path).join(".claude-plugin").join(".mcp.json").exists()
                    } else {
                        false
                    };

                    // 统计 skill 数量
                    let skill_count = if has_skills {
                        let skills_dir = std::path::Path::new(&install_path).join("skills");
                        if skills_dir.exists() {
                            fs::read_dir(&skills_dir)
                                .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count() as u32)
                                .unwrap_or(0)
                        } else {
                            0
                        }
                    } else {
                        0
                    };

                    let full_id = key.clone();
                    let enabled = enabled_map.get(&full_id).copied().unwrap_or(true);

                    plugins.push(PluginInfo {
                        name: plugin_name.clone(),
                        marketplace: marketplace.clone(),
                        version,
                        description,
                        scope,
                        installed_at: installed_at.chars().take(10).collect(),
                        install_path,
                        has_skills,
                        has_mcp,
                        skill_count,
                        enabled,
                        full_id,
                    });
                }
            }
        }
    }

    // 按安装时间降序
    plugins.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));
    plugins
}

/// 扫描 marketplace 目录中尚未安装的可用插件
fn scan_available_plugins(installed: &[PluginInfo]) -> Vec<AvailablePlugin> {
    let home = dirs::home_dir().unwrap_or_default();
    let marketplaces_dir = home.join(".claude").join("plugins").join("marketplaces");
    if !marketplaces_dir.exists() {
        return Vec::new();
    }

    // 已安装插件的 full_id 集合，用于过滤
    let installed_ids: std::collections::HashSet<String> = installed.iter().map(|p| p.full_id.clone()).collect();

    let mut available = Vec::new();

    let mp_entries = match fs::read_dir(&marketplaces_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    for mp_entry in mp_entries.flatten() {
        let mp_path = mp_entry.path();
        if !mp_path.is_dir() { continue; }
        let mp_name = mp_path.file_name().unwrap_or_default().to_string_lossy().to_string();

        // 扫描 plugins/ 和 external_plugins/ 子目录
        for sub_dir in &["plugins", "external_plugins"] {
            let dir = mp_path.join(sub_dir);
            if !dir.exists() { continue; }

            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let plugin_path = entry.path();
                if !plugin_path.is_dir() { continue; }
                let plugin_name = plugin_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let full_id = format!("{}@{}", plugin_name, mp_name);

                // 跳过已安装的
                if installed_ids.contains(&full_id) { continue; }

                // 尝试读取 plugin.json 获取描述
                let description = [
                    plugin_path.join(".claude-plugin").join("plugin.json"),
                    plugin_path.join("plugin.json"),
                ].iter()
                    .find_map(|p| {
                        fs::read_to_string(p).ok()
                            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                            .and_then(|d| d.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    })
                    .unwrap_or_default();

                available.push(AvailablePlugin {
                    name: plugin_name,
                    marketplace: mp_name.clone(),
                    description,
                    full_id,
                });
            }
        }
    }

    // 按名称排序
    available.sort_by(|a, b| a.name.cmp(&b.name));
    available
}

/// 从配置文件中移除指定 MCP 服务器（支持 JSON 格式的 mcpServers / mcp 字段）
pub fn remove_mcp_from_file(source_path: &str, server_name: &str) -> Result<(), String> {
    let path = std::path::Path::new(source_path);
    let content = fs::read_to_string(path).map_err(|e| format!("读取失败: {}", e))?;
    let cleaned = strip_jsonc_comments(&content);
    let mut data: serde_json::Value = serde_json::from_str(&cleaned).map_err(|e| format!("解析失败: {}", e))?;

    // 尝试从 mcpServers 字段移除（Claude / Gemini 格式）
    let mut removed = false;
    if let Some(servers) = data.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        if servers.remove(server_name).is_some() {
            removed = true;
        }
    }

    // 尝试从 mcp 字段移除（OpenCode / Kilo 格式）
    if !removed {
        if let Some(servers) = data.get_mut("mcp").and_then(|v| v.as_object_mut()) {
            if servers.remove(server_name).is_some() {
                removed = true;
            }
        }
    }

    if !removed {
        return Err(format!("未找到 MCP 服务器: {}", server_name));
    }

    let output = serde_json::to_string_pretty(&data).map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(path, output).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

// ============================================================
// Gemini 扩展扫描
// ============================================================

/// 扫描 Gemini CLI 已安装扩展
fn scan_gemini_extensions() -> Vec<ExtensionInfo> {
    let home = dirs::home_dir().unwrap_or_default();
    let extensions_dir = home.join(".gemini").join("extensions");
    if !extensions_dir.exists() {
        return Vec::new();
    }

    let mut extensions = Vec::new();
    if let Ok(entries) = fs::read_dir(&extensions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

            // 尝试读取扩展元信息（package.json 或 extension.json）
            let description = [
                path.join("package.json"),
                path.join("extension.json"),
            ].iter()
                .find_map(|p| {
                    fs::read_to_string(p).ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|d| d.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()))
                })
                .unwrap_or_default();

            // 尝试判断是否有 source 信息
            let source = [
                path.join("package.json"),
            ].iter()
                .find_map(|p| {
                    fs::read_to_string(p).ok()
                        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                        .and_then(|d| {
                            d.get("repository")
                                .and_then(|v| v.as_str().map(|s| s.to_string())
                                    .or_else(|| v.get("url").and_then(|u| u.as_str()).map(|s| s.to_string())))
                        })
                })
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            extensions.push(ExtensionInfo {
                tool: "gemini".to_string(),
                name,
                description,
                enabled: true, // 存在即启用
                source,
            });
        }
    }

    extensions.sort_by(|a, b| a.name.cmp(&b.name));
    extensions
}

/// 向 Claude Code settings.json 添加 MCP 服务器配置
pub fn add_mcp_server(name: &str, command: &str, args: &[String]) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("无法获取 home 目录")?;
    let path = home.join(".claude").join("settings.json");
    let content = fs::read_to_string(&path).unwrap_or_else(|_| "{}".to_string());
    let mut data: serde_json::Value = serde_json::from_str(&content).map_err(|e| format!("解析 settings.json 失败: {}", e))?;

    // 确保 mcpServers 字段存在
    if data.get("mcpServers").is_none() {
        data.as_object_mut().unwrap().insert("mcpServers".to_string(), serde_json::json!({}));
    }

    let servers = data.get_mut("mcpServers").unwrap().as_object_mut().unwrap();
    servers.insert(name.to_string(), serde_json::json!({
        "command": command,
        "args": args
    }));

    let output = serde_json::to_string_pretty(&data).map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&path, output).map_err(|e| format!("写入 settings.json 失败: {}", e))?;
    Ok(())
}
