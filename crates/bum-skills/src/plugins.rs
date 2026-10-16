//! Claude Code plugins, only through the official `claude plugin` CLI (spec section 8).
use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use serde::Deserialize;

use crate::catalog::{SkillEntry, SkillState, Source};
use crate::frontmatter;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct ClaudeCli {
    bin: PathBuf,
    timeout: Duration,
}

impl ClaudeCli {
    pub fn new(bin: PathBuf) -> Self {
        Self { bin, timeout: DEFAULT_TIMEOUT }
    }

    /// `claude` in PATH.
    pub fn system() -> Self {
        Self::new(PathBuf::from(if cfg!(windows) { "claude.cmd" } else { "claude" }))
    }

    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    /// Run `claude <args>` with a time limit; return stdout.
    pub fn run(&self, args: &[&str]) -> anyhow::Result<String> {
        let mut child = crate::process::command(&self.bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("could not run {}", self.bin.display()))?;
        // Read stdout/stderr on separate threads: large output will not fill the pipe and hang.
        let drain = |pipe: Option<Box<dyn Read + Send>>| std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut p) = pipe { let _ = p.read_to_string(&mut s); }
            s
        });
        let out_t = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let err_t = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(status) = child.try_wait()? {
                let out = out_t.join().unwrap_or_default();
                let err = err_t.join().unwrap_or_default();
                if !status.success() {
                    bail!("claude {} failed: {}", args.join(" "), err.trim());
                }
                return Ok(out);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                bail!("claude {} timed out after {:?}", args.join(" "), self.timeout);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub install_path: Option<PathBuf>,
}

impl PluginInfo {
    /// `superpowers@claude-plugins-official` → `superpowers`.
    pub fn short_name(&self) -> &str {
        self.id.split('@').next().unwrap_or(&self.id)
    }
}

/// `~70` → 70, `~6.3k` → 6300, `~1k` → 1000.
pub fn parse_tokens(s: &str) -> Option<u64> {
    let s = s.trim().trim_start_matches('~');
    let (num, mult) = match s.strip_suffix(['k', 'K']) {
        Some(n) => (n, 1000.0),
        None => (s, 1.0),
    };
    num.parse::<f64>().ok().map(|n| (n * mult).round() as u64)
}

/// The "Per-component" table of `claude plugin details`: (component, always-on, on-invoke).
pub fn parse_details(text: &str) -> Vec<(String, Option<u64>, Option<u64>)> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.first() == Some(&"component") {
            in_table = true;
            continue;
        }
        if !in_table {
            continue;
        }
        if cols.len() != 3 {
            if !rows.is_empty() {
                break;
            }
            continue;
        }
        rows.push((cols[0].to_string(), parse_tokens(cols[1]), parse_tokens(cols[2])));
    }
    rows
}

pub fn list_plugins(cli: &ClaudeCli) -> anyhow::Result<Vec<PluginInfo>> {
    let out = cli.run(&["plugin", "list", "--json"])?;
    serde_json::from_str(&out).context("output of `claude plugin list --json` is malformed")
}

/// Skills of installed plugins. A CLI error becomes a warning so the Skills screen still shows personal skills.
pub fn scan_plugins(cli: &ClaudeCli) -> (Vec<SkillEntry>, Vec<String>) {
    let mut warnings = Vec::new();
    let plugins = match list_plugins(cli) {
        Ok(p) => p,
        Err(e) => return (Vec::new(), vec![format!("Could not read the plugin list: {e:#}")]),
    };
    // Run `details` for the plugins in parallel so a slow CLI does not accumulate the wait time.
    let details: Vec<anyhow::Result<String>> = std::thread::scope(|s| {
        let handles: Vec<_> = plugins.iter().map(|p| s.spawn(move || cli.run(&["plugin", "details", p.short_name()]))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(anyhow::anyhow!("thread error")))).collect()
    });
    let mut out = Vec::new();
    for (p, res) in plugins.into_iter().zip(details) {
        let rows = match res {
            Ok(text) => parse_details(&text),
            Err(e) => {
                warnings.push(format!("Could not read details for plugin {}: {e:#}", p.id));
                continue;
            }
        };
        for (component, always_on, on_invoke) in rows {
            let dir = p.install_path.as_ref().map(|i| i.join("skills").join(&component));
            // Only take components that are skills (with a skills/<name> directory) when the install path is known.
            if dir.as_ref().is_some_and(|d| p.install_path.as_ref().is_some_and(|i| i.join("skills").exists()) && !d.exists()) {
                continue;
            }
            let description = dir
                .as_ref()
                .and_then(|d| std::fs::read_to_string(d.join("SKILL.md")).ok())
                .and_then(|b| frontmatter::parse(&b).description)
                .unwrap_or_default();
            out.push(SkillEntry {
                name: format!("{}:{component}", p.short_name()),
                tool: "claude_code".into(),
                description,
                source: Source::Plugin { plugin: p.id.clone() },
                state: if p.enabled { SkillState::Enabled } else { SkillState::Disabled },
                path: dir,
                symlink: false,
                installed_at: None,
                always_on_tokens: always_on,
                on_invoke_tokens: on_invoke,
                estimated: false,
                managed: true,
            });
        }
    }
    (out, warnings)
}
