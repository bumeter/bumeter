//! Kiro credit limits via `kiro-cli chat --no-interactive "/usage"` (runs locally, does not call the model, does not create a session).
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq)]
pub struct KiroUsage {
    pub used: f64,
    pub limit: f64,
    pub resets_on: NaiveDate,
    pub plan: String,
}

pub const TIMEOUT: Duration = Duration::from_secs(20);

/// Strip ANSI codes (CSI `ESC [ … letter`, OSC `ESC ] … BEL/ESC \`, and `ESC` + 1 char).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('[') => {
                for c in it.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = it.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && it.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn num(s: &str) -> Option<f64> {
    let v: f64 = s.trim().replace(',', "").parse().ok()?;
    v.is_finite().then_some(v)
}

/// Parse the `/usage` text; `None` if any part is missing or malformed.
pub fn parse(raw: &str) -> Option<KiroUsage> {
    let text = strip_ansi(raw);
    let lower = text.to_lowercase();
    // "… | resets on 2026-11-01 | KIRO PRO"
    let line = text.lines().find(|l| l.to_lowercase().contains("resets on"))?;
    let at = line.to_lowercase().find("resets on")? + "resets on".len();
    let rest = line[at..].trim_start();
    let resets_on = NaiveDate::parse_from_str(rest.get(..10)?, "%Y-%m-%d").ok()?;
    let plan = rest[10..].trim_start().strip_prefix('|').map(|p| p.split('|').next().unwrap_or("").trim().to_string()).unwrap_or_default();
    // "Credits (270.69 of 1000 covered in plan)"
    let start = lower.find("credits")?;
    let seg = &text[start..];
    let seg = &seg[seg.find('(')? + 1..];
    let seg = &seg[..seg.find(')').unwrap_or(seg.len())];
    let mut words = seg.split_whitespace();
    let used = num(words.next()?)?;
    if !words.next()?.eq_ignore_ascii_case("of") {
        return None;
    }
    let limit = num(words.next()?)?;
    (limit > 0.0 && used >= 0.0).then_some(KiroUsage { used, limit, resets_on, plan })
}

/// Find `kiro-cli` in PATH.
pub fn find_cli() -> Option<PathBuf> {
    let name = if cfg!(windows) { "kiro-cli.exe" } else { "kiro-cli" };
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

/// Run `bin args…` with a timeout; returns `(exited successfully, stdout + stderr)`. Cannot run or timed out → `None`.
fn run_capture(bin: &Path, args: &[&str], timeout: Duration) -> Option<(bool, String)> {
    let mut child = crate::process::command(bin)
        .args(args)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let (mut out, mut err) = (child.stdout.take()?, child.stderr.take()?);
    let (tx, rx) = std::sync::mpsc::channel();
    // Read on a separate thread: a hung command (or descendants holding the pipe) must not block the caller.
    std::thread::spawn(move || {
        use std::io::Read;
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        let mut e = String::new();
        let _ = err.read_to_string(&mut e);
        let _ = tx.send(s + "\n" + &e);
    });
    match rx.recv_timeout(timeout) {
        Ok(text) => {
            let ok = child.wait().map(|st| st.success()).unwrap_or(false);
            Some((ok, text))
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Run `bin chat --no-interactive /usage`; error, timeout, or unreadable output → `None`.
pub fn run_with(bin: &Path, timeout: Duration) -> Option<KiroUsage> {
    parse(&run_capture(bin, &["chat", "--no-interactive", "/usage"], timeout)?.1)
}

/// `bin whoami` reports logged in (exit 0, has a "Logged in" line, and no "not logged in").
/// If not logged in, do not ask for `/usage`: the command may error or demand a login.
pub fn logged_in_with(bin: &Path, timeout: Duration) -> bool {
    match run_capture(bin, &["whoami"], timeout) {
        Some((true, text)) => {
            let t = text.to_lowercase();
            t.contains("logged in") && !t.contains("not logged in")
        }
        _ => false,
    }
}

/// Ask for `/usage` only when logged in.
pub fn fetch_with(bin: &Path, timeout: Duration) -> Option<KiroUsage> {
    if !logged_in_with(bin, timeout) {
        return None;
    }
    run_with(bin, timeout)
}

/// Fetch limits only when the machine has `kiro-cli` and is logged in; otherwise do nothing.
pub fn fetch() -> Option<KiroUsage> {
    fetch_with(&find_cli()?, TIMEOUT)
}

impl KiroUsage {
    pub fn into_quota(self, fetched_at: i64) -> bum_core::meter::ToolQuota {
        bum_core::meter::ToolQuota { used: self.used, limit: self.limit, resets_on: self.resets_on, plan: self.plan, fetched_at }
    }
}
