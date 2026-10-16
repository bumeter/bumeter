//! Mức dùng theo công cụ cho menu tray / `bum status`: thanh %, kỳ credit, dòng "Hôm nay".
use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{Config, ToolLimit};
use crate::i18n::Lang;
use crate::insight::fmt_tokens;
use crate::limits::{LimitState, Window};
use crate::source::SourceInfo;
use crate::store::{Store, ToolActivity};

/// Kỳ credit chứa `today`: (ngày reset gần nhất ≤ hôm nay, ngày reset kế tiếp). `reset_day` kẹp 1–28.
pub fn credit_period(today: NaiveDate, reset_day: u32) -> (NaiveDate, NaiveDate) {
    let day = reset_day.clamp(1, 28);
    let this_month = today.with_day(day).expect("ngày 1–28 luôn có");
    let start = if today.day() >= day { this_month } else { this_month - Months::new(1) };
    (start, start + Months::new(1))
}

/// Hạn mức công cụ tự báo (vd Kiro `/usage`), kèm thời điểm lấy (epoch giây).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolQuota {
    pub used: f64,
    pub limit: f64,
    pub resets_on: NaiveDate,
    pub plan: String,
    pub fetched_at: i64,
}

/// Số liệu tự báo cũ hơn ngưỡng này thì bỏ, dùng hạn mức người dùng nhập.
pub const QUOTA_MAX_AGE_SECS: i64 = 3600;
/// Cũ hơn ngưỡng này thì ghi chú "cập nhật … trước".
pub const QUOTA_NOTE_AFTER_SECS: i64 = 20 * 60;

pub fn read_quotas(path: &Path) -> BTreeMap<String, ToolQuota> {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Ghi ra file tạm rồi đổi tên.
pub fn write_quotas(path: &Path, q: &BTreeMap<String, ToolQuota>) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(q)?)?;
    std::fs::rename(&tmp, path)
}

/// Credit đã dùng / hạn mức của kỳ hiện tại.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditMeter {
    pub used: f64,
    pub limit: f64,
    pub next_reset: NaiveDate,
    /// Có khi số liệu lấy từ công cụ (không phải tự tính từ log).
    pub fetched_at: Option<i64>,
}

/// Chọn nguồn: số công cụ tự báo còn mới → hạn mức người dùng nhập + credit tính từ log (`log_credits(from, to)`) → không có.
pub fn pick_credit(
    quota: Option<&ToolQuota>,
    user: Option<&ToolLimit>,
    now: DateTime<Utc>,
    log_credits: impl FnOnce(&str, &str) -> rusqlite::Result<f64>,
) -> rusqlite::Result<Option<CreditMeter>> {
    if let Some(q) = quota.filter(|q| now.timestamp() - q.fetched_at < QUOTA_MAX_AGE_SECS && q.limit > 0.0) {
        return Ok(Some(CreditMeter { used: q.used, limit: q.limit, next_reset: q.resets_on, fetched_at: Some(q.fetched_at) }));
    }
    let Some((limit, reset_day)) = user.and_then(|u| u.monthly_credits.filter(|c| *c > 0.0).map(|c| (c, u.reset_day))) else { return Ok(None) };
    let today = now.with_timezone(&chrono::Local).date_naive();
    let (start, next) = credit_period(today, reset_day);
    let used = log_credits(&crate::handlers::local_date_start(start), &crate::handlers::local_date_start(next))?;
    Ok(Some(CreditMeter { used, limit, next_reset: next, fetched_at: None }))
}

/// Dữ liệu cho menu: credit theo công cụ (chỉ công cụ có hạn mức) và hoạt động hôm nay.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub credits: BTreeMap<String, CreditMeter>,
    pub today: Vec<ToolActivity>,
}

pub fn snapshot(store: &Store, cfg: &Config, quotas: &BTreeMap<String, ToolQuota>, sources: &[SourceInfo], now: DateTime<Utc>) -> rusqlite::Result<Snapshot> {
    let mut credits = BTreeMap::new();
    for s in sources.iter().filter(|s| s.credits) {
        if let Some(c) = pick_credit(quotas.get(&s.id), cfg.tool_limits.get(&s.id), now, |from, to| store.tool_credits_between(&s.id, from, to))? {
            credits.insert(s.id.clone(), c);
        }
    }
    let (from, to) = crate::handlers::local_day_bounds(now);
    Ok(Snapshot { credits, today: store.tool_activity_between(&from, &to)? })
}

/// Thanh 10 ô; có dùng thì ít nhất 1 ô, chỉ đầy khi ≥ 100%.
pub fn bar(pct: f64) -> String {
    let n = if pct.is_nan() || pct <= 0.0 {
        0
    } else if pct >= 100.0 {
        10
    } else {
        ((pct / 10.0).round() as usize).clamp(1, 9)
    };
    "▰".repeat(n) + &"▱".repeat(10 - n)
}

/// Ký tự đệm cột tên (rộng bằng chữ số, gần bề rộng chữ trong font tỉ lệ).
pub const NAME_PAD: char = '\u{2007}';

/// Số dòng menu của một công cụ: quota 5 giờ/7 ngày thì 2, còn lại 1.
pub fn line_count(info: &SourceInfo) -> usize {
    if info.limits { 2 } else { 1 }
}

pub fn ago(secs: i64, lang: Lang) -> String {
    let (n, vi, en) = match secs.max(0) {
        s if s < 60 => (s, "giây", "s"),
        s if s < 3600 => (s / 60, "phút", "min"),
        s if s < 86_400 => (s / 3600, "giờ", "h"),
        s => (s / 86_400, "ngày", "d"),
    };
    match lang {
        Lang::Vi => format!("{n} {vi} trước"),
        Lang::En => format!("{n}{en} ago"),
    }
}

fn updated(secs: i64, lang: Lang) -> String {
    format!(" · {} {}", lang.pick("cập nhật", "updated"), ago(secs, lang))
}

fn pct_str(p: f64) -> String {
    format!("{p:.0}%")
}

/// Credit: ≥ 100 không lẻ, nhỏ hơn thì 1 chữ số lẻ (bỏ ".0").
pub fn fmt_credit(x: f64) -> String {
    if x.abs() >= 100.0 {
        format!("{x:.0}")
    } else {
        let s = format!("{x:.1}");
        s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
    }
}

fn credit_unit(lang: Lang) -> &'static str {
    lang.pick("credit", "credits")
}

fn token_unit(lang: Lang) -> &'static str {
    lang.pick("token", "tokens")
}

fn window(name: &str, w: Option<Window>, limits: &LimitState, now: i64, lang: Lang) -> String {
    match w {
        Some(w) => {
            let when = DateTime::from_timestamp(w.resets_at, 0).map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default();
            let fresh = match (limits.stale, limits.updated_at) {
                (true, Some(t)) => updated(now - t, lang),
                _ => String::new(),
            };
            format!("{}  {} · {name} · reset {when}{fresh}", bar(w.used_percentage), pct_str(w.used_percentage))
        }
        None => format!("{name} · {}", lang.pick("chưa có dữ liệu", "no data yet")),
    }
}

fn reset_date(d: NaiveDate, lang: Lang) -> String {
    match lang {
        Lang::Vi => d.format("%d/%m").to_string(),
        Lang::En => d.format("%b %-d").to_string(),
    }
}

fn credit_line(c: &CreditMeter, now: i64, lang: Lang) -> String {
    let pct = c.used / c.limit * 100.0;
    let note = c.fetched_at.filter(|t| now - t > QUOTA_NOTE_AFTER_SECS).map(|t| updated(now - t, lang)).unwrap_or_default();
    format!("{}  {} · {}/{} {} · reset {}{note}", bar(pct), pct_str(pct), fmt_credit(c.used), fmt_credit(c.limit), credit_unit(lang), reset_date(c.next_reset, lang))
}

/// Lượng dùng hôm nay của một công cụ: credit nếu tính bằng credit, còn lại token.
fn amount(info: &SourceInfo, a: &ToolActivity, lang: Lang) -> String {
    if info.credits {
        format!("{} {}", fmt_credit(a.credits), credit_unit(lang))
    } else {
        format!("{} {}", fmt_tokens(a.tokens), token_unit(lang))
    }
}

/// Text menu: các dòng theo công cụ (đúng thứ tự `sources`, `line_count` dòng mỗi công cụ) và dòng "Hôm nay".
#[derive(Debug, Clone, PartialEq)]
pub struct MenuText {
    pub tools: Vec<String>,
    pub today: String,
}

pub fn render(sources: &[SourceInfo], limits: &LimitState, snap: &Snapshot, now: i64, lang: Lang) -> MenuText {
    let width = sources.iter().map(|s| s.label.chars().count()).max().unwrap_or(0);
    let col = |label: &str| format!("{label}{}  ", NAME_PAD.to_string().repeat(width - label.chars().count()));
    let blank = col("");
    let idle = ToolActivity::default();
    let mut tools = Vec::new();
    for s in sources {
        let name = col(&s.label);
        if s.limits {
            tools.push(name + &window(lang.pick("5 giờ", "5 hours"), limits.five_hour, limits, now, lang));
            tools.push(blank.clone() + &window(lang.pick("7 ngày", "7 days"), limits.seven_day, limits, now, lang));
        } else if let Some(c) = snap.credits.get(&s.id) {
            tools.push(name + &credit_line(c, now, lang));
        } else {
            let a = snap.today.iter().find(|a| a.tool == s.id).unwrap_or(&idle);
            tools.push(format!("{name}{} {} · {} {}", lang.pick("hôm nay", "today"), amount(s, a, lang), a.prompts, lang.pick("lượt", "turns")));
        }
    }
    let active: Vec<String> = sources
        .iter()
        .filter_map(|s| snap.today.iter().find(|a| a.tool == s.id && (a.prompts > 0 || a.tokens > 0 || a.credits > 0.0)).map(|a| format!("{} {}", s.label, amount(s, a, lang))))
        .collect();
    let body = if active.is_empty() { lang.pick("chưa có hoạt động", "no activity yet").to_string() } else { active.join(" · ") };
    MenuText { tools, today: format!("{}: {body}", lang.pick("Hôm nay", "Today")) }
}
