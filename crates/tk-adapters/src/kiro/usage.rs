//! Hạn mức credit Kiro qua `kiro-cli chat --no-interactive "/usage"` (chạy cục bộ, không gọi model, không tạo phiên).
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

/// Bỏ mã ANSI (CSI `ESC [ … chữ`, OSC `ESC ] … BEL/ESC \`, và `ESC` + 1 ký tự).
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

/// Đọc text `/usage`; thiếu/hỏng phần nào thì `None`.
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

/// Tìm `kiro-cli` trong PATH.
pub fn find_cli() -> Option<PathBuf> {
    let name = if cfg!(windows) { "kiro-cli.exe" } else { "kiro-cli" };
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

/// Chạy `bin chat --no-interactive /usage`; lỗi, hết giờ hay không đọc được → `None`.
pub fn run_with(bin: &Path, timeout: Duration) -> Option<KiroUsage> {
    let mut child = Command::new(bin)
        .args(["chat", "--no-interactive", "/usage"])
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let (mut out, mut err) = (child.stdout.take()?, child.stderr.take()?);
    let (tx, rx) = std::sync::mpsc::channel();
    // Đọc ở luồng riêng: lệnh treo (hoặc con cháu giữ pipe) thì không chặn người gọi.
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
            let _ = child.wait();
            parse(&text)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

/// Lấy hạn mức nếu máy có `kiro-cli`.
pub fn fetch() -> Option<KiroUsage> {
    run_with(&find_cli()?, TIMEOUT)
}

impl KiroUsage {
    pub fn into_quota(self, fetched_at: i64) -> tk_core::meter::ToolQuota {
        tk_core::meter::ToolQuota { used: self.used, limit: self.limit, resets_on: self.resets_on, plan: self.plan, fetched_at }
    }
}
