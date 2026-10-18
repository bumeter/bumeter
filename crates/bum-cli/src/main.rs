//! The `bum` command: statusline, audit, scan, daemon, status, skills.
mod audit;
mod daemon;
mod statusline;

use std::path::PathBuf;
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
use bum_skills::ops::{Manager, OpRequest};
use bum_skills::plugins::ClaudeCli;

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
    /// Skills: list, enable/disable, undo, log.
    Skills {
        #[command(subcommand)]
        cmd: Option<SkillsCmd>,
        /// Tool whose `skills/` directory is used: `claude_code` (default) or `kiro`.
        #[arg(long, default_value = "claude_code", global = true)]
        tool: String,
        /// Print JSON instead of a table (for agents and scripts).
        #[arg(long, global = true)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SkillsCmd {
    /// List skills: always-on cost and usage (read-only).
    List,
    /// Move skills into `skills-disabled/`. Reversible: the journal keeps the way back.
    Disable {
        /// Skill names, as shown by `bum skills list`.
        names: Vec<String>,
        /// Apply the plan. Without it the plan is printed and nothing changes.
        #[arg(long)]
        yes: bool,
    },
    /// Move skills back into `skills/`.
    Enable {
        names: Vec<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Put the files of one journal entry back, by id.
    Undo { id: String },
    /// Operations done from the app or the CLI, newest first.
    Log,
    /// Check installed skills for hijacked instructions, credentials and duplicates (read-only).
    Audit,
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
        Cmd::Skills { cmd, tool, json } => skills(&paths, &tool, json, cmd),
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
    let opencode = bum_service::host::opencode_db(&config(paths));
    let mut engine = Engine::new(Store::open(&paths.db())?, vec![Box::new(ClaudeCode::new(&root)), Box::new(bum_adapters::kiro::Kiro::new(kiro))])
        .with_db_sources(vec![Box::new(bum_adapters::opencode::OpenCode::new(opencode))]);
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
    let mut st = bum_core::limits::state(bum_core::limits::read(&paths.limits()), None, chrono::Utc::now().timestamp());
    if let Some(f) = statusline::host_forecasts(paths) {
        (st.forecast_five_hour, st.forecast_seven_day) = f;
    }
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
    let sources = bum_adapters::installed(&bum_service::host::roots(&cfg));
    let quotas = bum_core::meter::read_quotas(&paths.tool_quotas());
    let snap = bum_core::meter::snapshot(&store, &cfg, &quotas, &sources, now).unwrap_or_default();
    bum_core::meter::render(&sources, limits, &snap, now.timestamp(), cfg.language).tools
}

/// `BUM_HOME` (or the platform home) + `.claude` by default, `.kiro` for Kiro: both use
/// `skills/` + `skills-disabled/`.
fn tool_dir(tool: &str) -> PathBuf {
    let home = bum_core::paths::home();
    if tool == "kiro" {
        home.join(".kiro")
    } else {
        home.join(".claude")
    }
}

fn check_tool(tool: &str) -> anyhow::Result<()> {
    if tool == "claude_code" || tool == "kiro" {
        return Ok(());
    }
    anyhow::bail!("unknown tool '{tool}' (use claude_code or kiro)")
}

/// Same layout as the app, and the same journal: Kiro's ops live in `data_dir/kiro`.
fn skill_manager(paths: &Paths, tool: &str) -> Manager {
    let data = if tool == "kiro" { paths.data_dir.join("kiro") } else { paths.data_dir.clone() };
    Manager::new(tool_dir(tool), data, ClaudeCli::system())
}

fn skills(paths: &Paths, tool: &str, json: bool, cmd: Option<SkillsCmd>) -> anyhow::Result<bool> {
    check_tool(tool)?;
    match cmd.unwrap_or(SkillsCmd::List) {
        SkillsCmd::List => skills_list(paths, tool, json),
        SkillsCmd::Log => {
            let entries = skill_manager(paths, tool).log()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if entries.is_empty() {
                println!("No operations in the journal.");
            } else {
                for e in &entries {
                    let ts = e.ts.get(..19).unwrap_or(&e.ts).replace('T', " ");
                    println!("{ts}  {:<9} {:<28} {}{}", e.op, e.id, e.description.lines().next().unwrap_or_default(), if e.undone { "  (undone)" } else { "" });
                }
            }
            Ok(true)
        }
        SkillsCmd::Undo { id } => {
            skill_manager(paths, tool).undo(&id)?;
            if json {
                println!("{}", serde_json::json!({ "undone": id, "tool": tool }));
            } else {
                println!("Restored: {id}");
            }
            Ok(true)
        }
        SkillsCmd::Disable { names, yes } => move_skills(paths, tool, json, "disable", names, yes),
        SkillsCmd::Enable { names, yes } => move_skills(paths, tool, json, "enable", names, yes),
        SkillsCmd::Audit => {
            let (entries, _) = scan_entries(tool);
            let audit = bum_skills::audit::audit(&entries);
            if json {
                println!("{}", serde_json::to_string_pretty(&audit)?);
            } else {
                print_audit(&audit);
            }
            if !audit.ok {
                eprintln!("bum: {} high-risk finding(s)", audit.findings.iter().filter(|f| f.severity == bum_skills::audit::Severity::High).count());
            }
            Ok(audit.ok)
        }
    }
}

fn print_audit(a: &bum_skills::audit::Audit) {
    use bum_skills::audit::Severity::{High, Info, Warn};
    let count = |s| a.findings.iter().filter(|f| f.severity == s).count();
    println!("{} skills · {} with a SKILL.md", a.skills, a.scanned);
    for (label, sev) in [("HIGH", High), ("WARN", Warn), ("INFO", Info)] {
        let found: Vec<_> = a.findings.iter().filter(|f| f.severity == sev).collect();
        if found.is_empty() {
            continue;
        }
        println!("\n{label} ({})", found.len());
        for f in found {
            println!("  {} [{}] line {}", f.skill, f.kind, f.line);
            println!("    {}", f.text);
            println!("    ↳ {}", f.why);
            println!("    {}", f.path.display());
        }
    }
    if !a.duplicates.is_empty() {
        println!("\nDUPLICATES ({})", a.duplicates.len());
        for d in &a.duplicates {
            println!("  [{}] {} — {}", d.kind, d.skills.join(", "), d.why);
        }
    }
    if a.findings.is_empty() && a.duplicates.is_empty() {
        println!("\nNothing suspicious.");
        return;
    }
    println!("\n{} high, {} warn, {} info · {} duplicates", count(High), count(Warn), count(Info), a.duplicates.len());
    println!("Heuristics: read the matched line before you act on it.");
}

/// disable/enable are reversible (a move plus a journal entry) but they do change what the next
/// session loads, so they are never applied implicitly: without `--yes` only the plan is printed.
fn move_skills(paths: &Paths, tool: &str, json: bool, op: &str, targets: Vec<String>, yes: bool) -> anyhow::Result<bool> {
    if targets.is_empty() {
        anyhow::bail!("no skill name given");
    }
    let m = skill_manager(paths, tool);
    let req = OpRequest { op: op.to_string(), targets, source: None, tool: Some(tool.to_string()), skill: None };
    let plan = m.plan(&req)?;
    let entry = yes.then(|| m.apply(&plan)).transpose()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "applied": entry.is_some(), "op": op, "tool": tool,
            "plan": { "id": plan.id, "description": plan.description },
            "entry": entry,
        }))?);
    } else {
        println!("{}", plan.description);
        if let Some(e) = &entry {
            println!("\nApplied: {}\nUndo with: bum skills undo {}", e.id, e.id);
        }
    }
    if entry.is_none() {
        eprintln!("\nbum: nothing changed — re-run with --yes to apply");
    }
    Ok(entry.is_some())
}

/// Installed skills for one tool, plus whatever the plugin scan could not read.
fn scan_entries(tool: &str) -> (Vec<bum_skills::catalog::SkillEntry>, Vec<String>) {
    use bum_skills::catalog::scan_personal_for;
    use bum_skills::plugins::scan_plugins;
    let mut entries = scan_personal_for(&tool_dir(tool), tool);
    let mut warnings = Vec::new();
    // Plugins are a Claude Code concept (`claude plugin list`); Kiro has none.
    if tool == "claude_code" {
        let (plugins, w) = scan_plugins(&ClaudeCli::system());
        entries.extend(plugins);
        warnings = w;
    }
    (entries, warnings)
}

fn skills_list(paths: &Paths, tool: &str, json: bool) -> anyhow::Result<bool> {
    use bum_skills::catalog::SkillState;
    use bum_skills::report::{build, Usage};
    let (entries, warnings) = scan_entries(tool);
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
    println!("{:<34} {:>7} {:>8} {:>5} {:>10}  LAST", "SKILL", "BASE", "/CALL", "USES", "AVG/CALL");
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
    if file.starts_with("db:") {
        anyhow::bail!("context analysis is not available for this tool yet");
    }
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
        // Point every tool at temp paths so the test does not depend on real data in $HOME (OpenCode: no database).
        let claude = dir.path().join("claude");
        let kiro = dir.path().join("kiro");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::create_dir_all(&kiro).unwrap();
        let mut tool_limits = std::collections::BTreeMap::new();
        tool_limits.insert("kiro".into(), bum_core::config::ToolLimit { monthly_credits: Some(1000.0), reset_day: 1 });
        let cfg = Config { claude_projects_dir: Some(claude), kiro_sessions_dir: Some(kiro), opencode_db: Some(dir.path().join("opencode.db")), tool_limits, ..Default::default() };
        cfg.save(&paths.config()).unwrap();
        let st = bum_core::limits::state(None, None, 0);
        let lines = tool_lines(&paths, &st, chrono::Utc::now());
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[2].starts_with("Kiro") && lines[2].contains("0/1000 credit"), "{lines:?}");
        assert!(tool_lines(&Paths::new(dir.path().join("none")), &st, chrono::Utc::now()).is_empty());
    }
}
