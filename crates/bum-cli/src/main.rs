//! The `tk` command: statusline, audit, scan, daemon, status.
mod audit;
mod daemon;
mod statusline;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::Value;
use bum_adapters::claude_code::ClaudeCode;
use bum_core::config::Config;
use bum_core::engine::Engine;
use bum_core::paths::Paths;
use bum_core::store::Store;
use bum_service::host::claude_root;
use bum_service::lock::HostLock;

#[derive(Parser)]
#[command(name = "bum", about = "Bumeter: AI usage statistics")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read Claude Code's statusline JSON from stdin, write limits.json, print the status line.
    Statusline {
        /// Only write the file, print nothing.
        #[arg(long)]
        quiet: bool,
        /// Write the file, then pass the input JSON through to stdout unchanged, so it can be chained into an existing statusline.
        #[arg(long, conflicts_with = "quiet")]
        tee: bool,
    },
    /// Compare the app's total token count against an independent count.
    Audit,
    /// Scan logs into the DB (when no host is running).
    Scan,
    /// Run the headless host.
    Daemon,
    /// View today's summary from the running host.
    Status,
    /// Analyze the context composition of a session (read-only).
    Session { session_id: String },
    /// List skills: always-on cost and usage (read-only).
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
        anyhow::bail!("host is running and has already scanned; bum scan is not needed");
    };
    let root = claude_root(&config(paths));
    let kiro = bum_service::host::kiro_root(&config(paths));
    let mut engine = Engine::new(Store::open(&paths.db())?, vec![Box::new(ClaudeCode::new(&root)), Box::new(bum_adapters::kiro::Kiro::new(kiro))]);
    let started = std::time::Instant::now();
    let r = engine.full_scan()?;
    eprintln!("scanned in {:?}", started.elapsed());
    println!("Scanned {} lines ({} parse errors), {} new/updated prompts", r.lines, r.parse_errors, r.summary.touched_prompts.len());
    Ok(true)
}

fn status(paths: &Paths) -> anyhow::Result<bool> {
    let today = bum_service::client::call(paths, "today_summary", Value::Null)
        .map_err(|_| anyhow::anyhow!("could not connect to host (run the app or `bum daemon` first)"))?;
    let lang = config(paths).language;
    let tokens = bum_core::insight::fmt_tokens(today["total"].as_u64().unwrap_or(0));
    println!("{}: {} {}, {tokens} token", lang.pick("Hôm nay", "Today"), today["prompts"], lang.pick("lượt", "turns"));
    // Like statusline: % quota from limits.json (the host is running, so the file is always up to date).
    let st = bum_core::limits::state(bum_core::limits::read(&paths.limits()), None, chrono::Utc::now().timestamp());
    println!("{}: {}", lang.pick("Giới hạn", "Limits"), bum_core::limits::render_line(&st, None, lang));
    for line in tool_lines(paths, &st, chrono::Utc::now()) {
        println!("{line}");
    }
    Ok(true)
}

/// Per-tool lines like the tray menu (percent bar, credit/limit), reading the DB read-only. DB error → print nothing.
fn tool_lines(paths: &Paths, limits: &bum_core::limits::LimitState, now: chrono::DateTime<chrono::Utc>) -> Vec<String> {
    let Ok(store) = Store::open_readonly(&paths.db()) else { return vec![] };
    let cfg = config(paths);
    let sources = bum_adapters::installed(&claude_root(&cfg), &bum_service::host::kiro_root(&cfg));
    let quotas = bum_core::meter::read_quotas(&paths.tool_quotas());
    let snap = bum_core::meter::snapshot(&store, &cfg, &quotas, &sources, now).unwrap_or_default();
    bum_core::meter::render(&sources, limits, &snap, now.timestamp(), cfg.language).tools
}

fn skills(paths: &Paths, json: bool) -> anyhow::Result<bool> {
    use bum_skills::catalog::{scan_personal, SkillState};
    use bum_skills::plugins::{scan_plugins, ClaudeCli};
    use bum_skills::report::{build, Usage};
    let claude_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let mut entries = scan_personal(&claude_dir);
    let (plugins, warnings) = scan_plugins(&ClaudeCli::system());
    entries.extend(plugins);
    let usage: Option<Vec<Usage>> = Store::open_readonly(&paths.db())
        .ok()
        .and_then(|s| bum_core::handlers::skill_stats(&s).ok())
        .map(|v| v.into_iter().map(|s| Usage { name: s.name, uses: s.uses, projects: s.projects, tokens: s.tokens, avg_tokens: s.avg_tokens, last_used: s.last_used }).collect());
    let r = build(entries, usage, warnings, chrono::Utc::now());
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
        return Ok(true);
    }
    println!("Always-on cost: ~{} tok/session · {} unused for 30 days (~{} tok)", r.total_always_on, r.unused.len(), r.unused_always_on);
    for w in &r.warnings {
        println!("⚠ {w}");
    }
    println!("{:<34} {:>7} {:>8} {:>5} {:>10}  {}", "SKILL", "BASE", "/CALL", "USES", "AVG/CALL", "LAST");
    for row in r.rows.iter().chain(&r.commands) {
        let e = &row.entry;
        let mark = if e.state == SkillState::Enabled { "●" } else { "○" };
        let opt = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
        println!("{mark} {:<32} {:>7} {:>8} {:>5} {:>10}  {}{}", e.name, opt(e.always_on_tokens), opt(e.on_invoke_tokens), row.uses,
            opt(row.avg_tokens), row.last_used.as_deref().map(|t| t.get(..10).unwrap_or(t)).unwrap_or("—"), if row.unused_30d { "  💡 unused for 30 days" } else { "" });
    }
    Ok(true)
}

fn session(paths: &Paths, id: &str) -> anyhow::Result<bool> {
    let store = Store::open_readonly(&paths.db())?;
    let file = store.session_file(id)?.ok_or_else(|| anyhow::anyhow!("no session {id}"))?;
    let t = std::time::Instant::now();
    let a = bum_adapters::claude_code::composition::analyze_file(std::path::Path::new(&file))?;
    println!("{} prompts · {} calls · {} compactions · analyzed in {:?}", a.prompts.len(), a.calls.len(), a.compactions.len(), t.elapsed());
    if let Some(peak) = a.calls.iter().max_by_key(|c| c.context) {
        println!("Largest context {} at {}:", peak.context, peak.request_id);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_lines_show_kiro_credit_against_user_limit() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        drop(Store::open(&paths.db()).unwrap());
        // Point both tools at existing empty dirs so the test does not depend on real logs in $HOME.
        let claude = dir.path().join("claude");
        let kiro = dir.path().join("kiro");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::create_dir_all(&kiro).unwrap();
        let mut cfg = Config::default();
        cfg.claude_projects_dir = Some(claude);
        cfg.kiro_sessions_dir = Some(kiro);
        cfg.tool_limits.insert("kiro".into(), bum_core::config::ToolLimit { monthly_credits: Some(1000.0), reset_day: 1 });
        cfg.save(&paths.config()).unwrap();
        let st = bum_core::limits::state(None, None, 0);
        let lines = tool_lines(&paths, &st, chrono::Utc::now());
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[2].starts_with("Kiro") && lines[2].contains("0/1000 credit"), "{lines:?}");
        assert!(tool_lines(&Paths::new(dir.path().join("none")), &st, chrono::Utc::now()).is_empty());
    }
}
