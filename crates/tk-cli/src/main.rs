//! Lệnh `tk`: statusline, audit, scan, daemon, status.
mod audit;
mod daemon;
mod statusline;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::Value;
use tk_adapters::claude_code::ClaudeCode;
use tk_core::config::Config;
use tk_core::engine::Engine;
use tk_core::paths::Paths;
use tk_core::store::Store;
use tk_service::host::claude_root;
use tk_service::lock::HostLock;

#[derive(Parser)]
#[command(name = "bum", about = "Bumeter: thống kê sử dụng AI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Đọc JSON statusline của Claude Code từ stdin, ghi limits.json, in dòng trạng thái.
    Statusline {
        /// Chỉ ghi file, không in gì.
        #[arg(long)]
        quiet: bool,
        /// Ghi file rồi chuyển nguyên JSON đầu vào ra stdout, để nối tiếp vào statusline có sẵn.
        #[arg(long, conflicts_with = "quiet")]
        tee: bool,
    },
    /// So tổng token của app với một cách đếm độc lập.
    Audit,
    /// Quét log vào DB (khi không có host chạy).
    Scan,
    /// Chạy host không giao diện.
    Daemon,
    /// Xem tóm tắt hôm nay từ host đang chạy.
    Status,
    /// Phân tích thành phần context của một phiên (chỉ đọc).
    Session { session_id: String },
    /// Liệt kê skill: chi phí nền và mức dùng (chỉ đọc).
    Skills {
        #[arg(long)]
        json: bool,
    },
}

fn config(paths: &Paths) -> Config {
    Config::load(&paths.config())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let paths = Paths::resolve();
    let result: anyhow::Result<bool> = match cli.cmd {
        Cmd::Statusline { quiet, tee } => {
            statusline::run(quiet, tee);
            Ok(true)
        }
        Cmd::Audit => audit::run(&claude_root(&config(&paths))),
        Cmd::Scan => scan(&paths),
        Cmd::Daemon => daemon::run(paths.clone(), config(&paths)).map(|_| true),
        Cmd::Status => status(&paths),
        Cmd::Skills { json } => skills(&paths, json),
        Cmd::Session { session_id } => session(&paths, &session_id),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("bum: {e}");
            ExitCode::FAILURE
        }
    }
}

fn scan(paths: &Paths) -> anyhow::Result<bool> {
    paths.ensure()?;
    let Some(_lock) = HostLock::try_acquire(&paths.lock())? else {
        anyhow::bail!("host đang chạy và đã tự quét; không cần bum scan");
    };
    let root = claude_root(&config(paths));
    let kiro = tk_service::host::kiro_root(&config(paths));
    let mut engine = Engine::new(Store::open(&paths.db())?, vec![Box::new(ClaudeCode::new(&root)), Box::new(tk_adapters::kiro::Kiro::new(kiro))]);
    let started = std::time::Instant::now();
    let r = engine.full_scan()?;
    eprintln!("quét trong {:?}", started.elapsed());
    println!("Đã quét {} dòng ({} lỗi parse), {} lần hỏi mới/cập nhật", r.lines, r.parse_errors, r.summary.touched_prompts.len());
    Ok(true)
}

fn status(paths: &Paths) -> anyhow::Result<bool> {
    let today = tk_service::client::call(paths, "today_summary", Value::Null)
        .map_err(|_| anyhow::anyhow!("không kết nối được host (chạy app hoặc `tk daemon` trước)"))?;
    let lang = config(paths).language;
    let tokens = tk_core::insight::fmt_tokens(today["total"].as_u64().unwrap_or(0));
    println!("{}: {} {}, {tokens} token", lang.pick("Hôm nay", "Today"), today["prompts"], lang.pick("lượt", "turns"));
    // Như statusline: % quota từ limits.json (host đang chạy nên file luôn mới nhất).
    let st = tk_core::limits::state(tk_core::limits::read(&paths.limits()), None, chrono::Utc::now().timestamp());
    println!("{}: {}", lang.pick("Giới hạn", "Limits"), tk_core::limits::render_line(&st, None, lang));
    Ok(true)
}

fn skills(paths: &Paths, json: bool) -> anyhow::Result<bool> {
    use tk_skills::catalog::{scan_personal, SkillState};
    use tk_skills::plugins::{scan_plugins, ClaudeCli};
    use tk_skills::report::{build, Usage};
    let claude_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let mut entries = scan_personal(&claude_dir);
    let (plugins, warnings) = scan_plugins(&ClaudeCli::system());
    entries.extend(plugins);
    let usage: Option<Vec<Usage>> = Store::open_readonly(&paths.db())
        .ok()
        .and_then(|s| tk_core::handlers::skill_stats(&s).ok())
        .map(|v| v.into_iter().map(|s| Usage { name: s.name, uses: s.uses, projects: s.projects, tokens: s.tokens, avg_tokens: s.avg_tokens, last_used: s.last_used }).collect());
    let r = build(entries, usage, warnings, chrono::Utc::now());
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
        return Ok(true);
    }
    println!("Chi phí nền: ~{} tok/phiên · {} chưa dùng 30 ngày (~{} tok)", r.total_always_on, r.unused.len(), r.unused_always_on);
    for w in &r.warnings {
        println!("⚠ {w}");
    }
    println!("{:<34} {:>7} {:>8} {:>5} {:>10}  {}", "SKILL", "NỀN", "/LẦN", "DÙNG", "TB/LẦN", "CUỐI");
    for row in r.rows.iter().chain(&r.commands) {
        let e = &row.entry;
        let mark = if e.state == SkillState::Enabled { "●" } else { "○" };
        let opt = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
        println!("{mark} {:<32} {:>7} {:>8} {:>5} {:>10}  {}{}", e.name, opt(e.always_on_tokens), opt(e.on_invoke_tokens), row.uses,
            opt(row.avg_tokens), row.last_used.as_deref().map(|t| t.get(..10).unwrap_or(t)).unwrap_or("—"), if row.unused_30d { "  💡 chưa dùng 30 ngày" } else { "" });
    }
    Ok(true)
}

fn session(paths: &Paths, id: &str) -> anyhow::Result<bool> {
    let store = Store::open_readonly(&paths.db())?;
    let file = store.session_file(id)?.ok_or_else(|| anyhow::anyhow!("không có phiên {id}"))?;
    let t = std::time::Instant::now();
    let a = tk_adapters::claude_code::composition::analyze_file(std::path::Path::new(&file))?;
    println!("{} lần hỏi · {} lượt gọi · {} lần compact · phân tích {:?}", a.prompts.len(), a.calls.len(), a.compactions.len(), t.elapsed());
    if let Some(peak) = a.calls.iter().max_by_key(|c| c.context) {
        println!("Context lớn nhất {} tại {}:", peak.context, peak.request_id);
        for (cat, v) in &peak.parts {
            println!("  {:<10} {:>9} ({:.0}%)", format!("{cat:?}"), v, *v as f64 * 100.0 / peak.context.max(1) as f64);
        }
    }
    let lang = config(paths).language;
    for s in &a.suggestions {
        println!("⚠ {}", s.text(lang));
    }
    Ok(true)
}
