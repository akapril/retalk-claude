use clap::{Parser, Subcommand};
use colored::*;
use retalk_lib::indexer::SessionIndex;
use retalk_lib::searcher::{self, SearchResult};

#[derive(Parser)]
#[command(name = "retalk", version, about = "AI 编码助手会话管理 CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 搜索会话
    Search {
        /// 搜索关键词
        query: String,
        /// 按工具筛选 (claude/codex/gemini/opencode/kilo)
        #[arg(short, long)]
        provider: Option<String>,
        /// 最大结果数
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// JSON 格式输出
        #[arg(long)]
        json: bool,
    },
    /// 列出最近会话
    List {
        /// 按工具筛选
        #[arg(short, long)]
        provider: Option<String>,
        /// 最大结果数
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// JSON 格式输出
        #[arg(long)]
        json: bool,
    },
    /// 恢复会话
    Resume {
        /// 会话序号或 ID（不提供则交互式选择）
        id: Option<String>,
    },
    /// 查看会话对话时间线
    Show {
        /// 会话 ID
        session_id: String,
        /// Provider (claude/codex/gemini/opencode/kilo)
        #[arg(short, long, default_value = "claude")]
        provider: String,
    },
}

fn main() {
    let cli = Cli::parse();

    // 初始化搜索索引
    let index = match SessionIndex::new() {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("索引初始化失败: {}", e);
            std::process::exit(1);
        }
    };

    // 如果索引为空，先执行一次全量扫描
    if index.doc_count() == 0 {
        eprintln!("首次运行，正在扫描会话数据...");
        let sessions = retalk_lib::scanner::scan_all_sessions();
        if let Err(e) = index.rebuild(&sessions) {
            eprintln!("索引构建失败: {}", e);
            std::process::exit(1);
        }
        eprintln!("扫描完成，共 {} 条会话", sessions.len());
    }

    match cli.command {
        Commands::Search {
            query,
            provider,
            limit,
            json,
        } => {
            let filter = provider.as_deref();
            let results = searcher::search(&index, &query, limit, filter);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&results).unwrap_or_default()
                );
            } else {
                print_results(&results);
            }
        }
        Commands::List {
            provider,
            limit,
            json,
        } => {
            let filter = provider.as_deref();
            let results = searcher::list_all(&index, limit, filter);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&results).unwrap_or_default()
                );
            } else {
                print_results(&results);
            }
        }
        Commands::Resume { id } => {
            let results = searcher::list_all(&index, 20, None);
            if results.is_empty() {
                println!("没有找到会话");
                return;
            }

            let target = match id {
                Some(ref id_str) => {
                    // 尝试作为序号解析
                    if let Ok(idx) = id_str.parse::<usize>() {
                        if idx >= 1 && idx <= results.len() {
                            Some(&results[idx - 1])
                        } else {
                            eprintln!("序号超出范围 (1-{})", results.len());
                            None
                        }
                    } else {
                        // 作为 session_id 查找
                        results.iter().find(|r| r.session_id.starts_with(id_str))
                    }
                }
                None => {
                    // 交互式选择
                    print_results(&results);
                    println!();
                    eprint!("输入序号恢复 (q 退出): ");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap();
                    let input = input.trim();
                    if input == "q" || input.is_empty() {
                        return;
                    }
                    if let Ok(idx) = input.parse::<usize>() {
                        if idx >= 1 && idx <= results.len() {
                            Some(&results[idx - 1])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            };

            if let Some(r) = target {
                let cmd = retalk_lib::terminal::build_resume_command(
                    &r.provider,
                    &r.project_path,
                    &r.session_id,
                );
                println!("执行: {}", cmd);
                let shell = if cfg!(windows) { "cmd" } else { "sh" };
                let flag = if cfg!(windows) { "/c" } else { "-c" };
                let _ = std::process::Command::new(shell)
                    .args([flag, &cmd])
                    .status();
            } else {
                eprintln!("找不到匹配的会话");
            }
        }
        Commands::Show {
            session_id,
            provider,
        } => {
            let messages = retalk_lib::timeline::read_timeline(&provider, &session_id);
            if messages.is_empty() {
                println!("没有找到该会话的消息记录");
                return;
            }
            println!("共 {} 条消息\n", messages.len());
            for msg in &messages {
                let role_label = match msg.role.as_str() {
                    "user" => "USER",
                    "assistant" => "ASSISTANT",
                    "tool" => "TOOL",
                    _ => "SYSTEM",
                };
                let tool_str = msg
                    .tool_name
                    .as_ref()
                    .map(|t| format!(" [{}]", t))
                    .unwrap_or_default();
                let token_str = if msg.token_count > 0 {
                    format!(" ({} tokens)", msg.token_count)
                } else {
                    String::new()
                };
                println!(
                    "--- {} {} {}{} ---",
                    role_label, msg.timestamp, tool_str, token_str
                );
                println!("{}\n", msg.content);
            }
        }
    }
}

/// 彩色格式化输出搜索结果
fn print_results(results: &[SearchResult]) {
    if results.is_empty() {
        println!("{}", "没有找到会话".dimmed());
        return;
    }
    for (i, r) in results.iter().enumerate() {
        let prompt = if r.last_prompt.chars().count() > 50 {
            format!(
                "{}...",
                r.last_prompt.chars().take(47).collect::<String>()
            )
        } else {
            r.last_prompt.clone()
        };

        let provider_str = format!("[{:<8}]", r.provider);
        let provider_colored = match r.provider.as_str() {
            "claude" => provider_str.truecolor(228, 160, 96),
            "codex" => provider_str.truecolor(16, 163, 127),
            "gemini" => provider_str.truecolor(66, 133, 244),
            "opencode" => provider_str.truecolor(249, 115, 22),
            "kilo" => provider_str.truecolor(236, 72, 153),
            _ => provider_str.normal(),
        };

        println!(
            " {}  {} {} {}  {}",
            format!("{:>3}", i + 1).dimmed(),
            provider_colored,
            r.project_name.bold(),
            r.updated_at.dimmed(),
            prompt.dimmed()
        );
    }
}
