use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct EcosystemData {
    pub skills: Vec<SkillInfo>,
    pub mcp_servers: Vec<McpServerInfo>,
    pub configs: Vec<ToolConfig>,
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

#[derive(Debug, Clone, Serialize)]
pub struct ToolConfig {
    pub tool: String,
    pub key: String,
    pub value: String,
}

/// 扫描所有 AI 编码工具的生态信息
pub fn scan_ecosystem() -> EcosystemData {
    let mut skills = Vec::new();
    let mut mcp_servers = Vec::new();
    let mut configs = Vec::new();

    // Claude Code
    scan_claude_skills(&mut skills);
    scan_claude_mcp(&mut mcp_servers);
    scan_claude_config(&mut configs);

    // Codex CLI
    scan_codex_mcp(&mut mcp_servers);
    scan_codex_config(&mut configs);

    // Gemini CLI
    scan_gemini_mcp(&mut mcp_servers);
    scan_gemini_config(&mut configs);

    // OpenCode
    scan_opencode_mcp(&mut mcp_servers);

    // Kilo Code
    scan_kilo_mcp(&mut mcp_servers);

    EcosystemData {
        skills,
        mcp_servers,
        configs,
    }
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

    // 全局配置: ~/.claude/settings.json
    let settings_path = home.join(".claude").join("settings.json");
    if let Some(data) = read_json_file(&settings_path) {
        if let Some(mcp) = data.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, config) in mcp {
                servers.push(parse_mcp_entry("claude", name, config, &settings_path));
            }
        }
    }
}

fn scan_claude_config(configs: &mut Vec<ToolConfig>) {
    let home = dirs::home_dir().unwrap_or_default();
    let settings_path = home.join(".claude").join("settings.json");
    if let Some(data) = read_json_file(&settings_path) {
        if let Some(model) = data.get("model").and_then(|v| v.as_str()) {
            configs.push(ToolConfig {
                tool: "claude".into(),
                key: "model".into(),
                value: model.into(),
            });
        }
        if let Some(pm) = data
            .get("permissions")
            .and_then(|v| v.get("mode"))
            .and_then(|v| v.as_str())
        {
            configs.push(ToolConfig {
                tool: "claude".into(),
                key: "permission_mode".into(),
                value: pm.into(),
            });
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

fn scan_codex_config(configs: &mut Vec<ToolConfig>) {
    let home = dirs::home_dir().unwrap_or_default();
    let config_path = home.join(".codex").join("config.toml");
    if !config_path.exists() {
        return;
    }

    let content = fs::read_to_string(&config_path).unwrap_or_default();
    if let Ok(toml_val) = content.parse::<toml::Value>() {
        if let Some(model) = toml_val.get("model").and_then(|v| v.as_str()) {
            configs.push(ToolConfig {
                tool: "codex".into(),
                key: "model".into(),
                value: model.into(),
            });
        }
        if let Some(provider) = toml_val.get("model_provider").and_then(|v| v.as_str()) {
            configs.push(ToolConfig {
                tool: "codex".into(),
                key: "provider".into(),
                value: provider.into(),
            });
        }
        if let Some(approval) = toml_val.get("approval_policy").and_then(|v| v.as_str()) {
            configs.push(ToolConfig {
                tool: "codex".into(),
                key: "approval".into(),
                value: approval.into(),
            });
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

fn scan_gemini_config(configs: &mut Vec<ToolConfig>) {
    let home = dirs::home_dir().unwrap_or_default();
    let settings_path = home.join(".gemini").join("settings.json");
    if let Some(data) = read_json_file(&settings_path) {
        if let Some(model) = data.get("model").and_then(|v| v.as_str()) {
            configs.push(ToolConfig {
                tool: "gemini".into(),
                key: "model".into(),
                value: model.into(),
            });
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
pub fn toggle_claude_mcp(server_name: &str, enabled: bool) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("无法获取 home 目录")?;
    let path = home.join(".claude").join("settings.json");
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut data: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    if let Some(servers) = data
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
    {
        if let Some(server) = servers.get_mut(server_name) {
            if let Some(obj) = server.as_object_mut() {
                obj.insert(
                    "disabled".to_string(),
                    serde_json::Value::Bool(!enabled),
                );
            }
        }
    }

    let output = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    fs::write(&path, output).map_err(|e| e.to_string())?;
    Ok(())
}
