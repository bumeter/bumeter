//! Per-tool usage for the tray menu / `bum status`: % bar, credit period, "Today" line.
use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{Config, ToolLimit};
use crate::forecast::{self, Forecast};
use crate::i18n::Lang;
use crate::insight::fmt_tokens;
use crate::limits::{LimitState, Window};
use crate::source::SourceInfo;
use crate::store::{Store, ToolActivity};

pub use crate::ring::{ring_key, ring_png, ring_rgba, RingTone, RING_SIZE};

/// Credit period containing `today`: (most recent reset day ≤ today, next reset day). `reset_day` is clamped to 1–28.
pub fn credit_period(today: NaiveDate, reset_day: u32) -> (NaiveDate, NaiveDate) {
    let day = reset_day.clamp(1, 28);
    let this_month = today.with_day(day).expect("day 1-28 always exists");
    let start = if today.day() >= day { this_month } else { this_month - Months::new(1) };
    (start, start + Months::new(1))
}

/// Tool self-reported quota (e.g. Kiro `/usage`), with the fetch time (epoch seconds).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolQuota {
    pub used: f64,
    pub limit: f64,
    pub resets_on: NaiveDate,
    pub plan: String,
    pub fetched_at: i64,
}

/// Self-reported data older than this is discarded; the user-entered quota is used instead.
pub const QUOTA_MAX_AGE_SECS: i64 = 3600;
/// Older than this gets the "updated … ago" note.
pub const QUOTA_NOTE_AFTER_SECS: i64 = 20 * 60;

pub fn read_quotas(path: &Path) -> BTreeMap<String, ToolQuota> {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Write to a temp file then rename.
pub fn write_quotas(path: &Path, q: &BTreeMap<String, ToolQuota>) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(q)?)?;
    std::fs::rename(&tmp, path)
}

/// Credits used / limit for the current period.
#[derive(Debug, Clone, PartialEq)]
pub struct CreditMeter {
    pub used: f64,
    pub limit: f64,
    pub next_reset: NaiveDate,
    /// Set when the data came from the tool (rather than being computed from logs).
    pub fetched_at: Option<i64>,
    /// When the credits run out at this period's pace; set by `with_forecast`.
    pub forecast: Option<Forecast>,
}

impl CreditMeter {
    /// Average-pace forecast over the current period (starts one month before `next_reset`, local midnight).
    pub fn with_forecast(mut self, now: DateTime<Utc>) -> Self {
        let ts = |d: NaiveDate| DateTime::parse_from_rfc3339(&crate::handlers::local_date_start(d)).ok().map(|t| t.timestamp());
        let period = ts(self.next_reset - Months::new(1)).zip(ts(self.next_reset));
        self.forecast = period.and_then(|(start, end)| forecast::paced(self.used, self.limit, start, end, now.timestamp(), forecast::CREDIT_MIN_ELAPSED));
        self
    }
}

/// Pick the source: fresh tool-reported data → user-entered quota + credits computed from logs (`log_credits(from, to)`) → none.
pub fn pick_credit(
    quota: Option<&ToolQuota>,
    user: Option<&ToolLimit>,
    now: DateTime<Utc>,
    log_credits: impl FnOnce(&str, &str) -> rusqlite::Result<f64>,
) -> rusqlite::Result<Option<CreditMeter>> {
    if let Some(q) = quota.filter(|q| now.timestamp() - q.fetched_at < QUOTA_MAX_AGE_SECS && q.limit > 0.0) {
        return Ok(Some(CreditMeter { used: q.used, limit: q.limit, next_reset: q.resets_on, fetched_at: Some(q.fetched_at), forecast: None }.with_forecast(now)));
    }
    let Some((limit, reset_day)) = user.and_then(|u| u.monthly_credits.filter(|c| *c > 0.0).map(|c| (c, u.reset_day))) else { return Ok(None) };
    let today = now.with_timezone(&chrono::Local).date_naive();
    let (start, next) = credit_period(today, reset_day);
    let used = log_credits(&crate::handlers::local_date_start(start), &crate::handlers::local_date_start(next))?;
    Ok(Some(CreditMeter { used, limit, next_reset: next, fetched_at: None, forecast: None }.with_forecast(now)))
}

/// Menu data: per-tool credits (only tools with a quota) and today's activity.
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

/// A 10-cell bar; any usage fills at least 1 cell, full only at ≥ 100%.
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

/// Padding character for the name column (digit width, close to glyph width in a proportional font).
pub const NAME_PAD: char = '\u{2007}';

/// Number of menu lines for a tool: 2 with 5-hour/7-day quota, otherwise 1.
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

/// Credit: no decimals at ≥ 100, otherwise 1 decimal (dropping ".0").
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

/// A quota window's figures: %, reset time/date, stale-data note. `by_date`: a long window (7 days) shows the reset date instead of the time.
fn window_parts(w: Window, by_date: bool, limits: &LimitState, now: i64, lang: Lang) -> (f64, String, String) {
    let local = DateTime::from_timestamp(w.resets_at, 0).map(|t| t.with_timezone(&chrono::Local));
    let when = local.map(|t| if by_date { reset_date(t.date_naive(), lang) } else { t.format("%H:%M").to_string() }).unwrap_or_default();
    let fresh = match (limits.stale, limits.updated_at) {
        (true, Some(t)) => updated(now - t, lang),
        _ => String::new(),
    };
    (w.used_percentage, when, fresh)
}

fn no_data(name: &str, lang: Lang) -> String {
    format!("{name} · {}", lang.pick("chưa có dữ liệu", "no data yet"))
}

/// "hết ~14:20 · " before the reset part when the forecast says the limit comes first; a date for long windows. Empty otherwise.
fn out_part(f: Option<Forecast>, by_date: bool, lang: Lang) -> String {
    let Some(t) = f.filter(|f| f.at_risk).and_then(|f| f.hits_at).and_then(|h| DateTime::from_timestamp(h, 0)) else { return String::new() };
    let t = t.with_timezone(&chrono::Local);
    let when = if by_date { reset_date(t.date_naive(), lang) } else { t.format("%H:%M").to_string() };
    format!("{} ~{when} · ", lang.pick("hết", "out"))
}

fn window(name: &str, w: Option<Window>, by_date: bool, f: Option<Forecast>, limits: &LimitState, now: i64, lang: Lang) -> String {
    match w {
        Some(w) => {
            let (pct, when, fresh) = window_parts(w, by_date, limits, now, lang);
            format!("{}  {} · {name} · {}reset {when}{fresh}", bar(pct), pct_str(pct), out_part(f, by_date, lang))
        }
        None => no_data(name, lang),
    }
}

fn reset_date(d: NaiveDate, lang: Lang) -> String {
    match lang {
        Lang::Vi => d.format("%d/%m").to_string(),
        Lang::En => d.format("%b %-d").to_string(),
    }
}

/// %, the "used/limit credit · reset …" part, stale note (/usage data older than 20 minutes).
fn credit_parts(c: &CreditMeter, now: i64, lang: Lang) -> (f64, String, String) {
    let pct = c.used / c.limit * 100.0;
    let note = c.fetched_at.filter(|t| now - t > QUOTA_NOTE_AFTER_SECS).map(|t| updated(now - t, lang)).unwrap_or_default();
    let body = format!("{}/{} {} · {}reset {}", fmt_credit(c.used), fmt_credit(c.limit), credit_unit(lang), out_part(c.forecast, true, lang), reset_date(c.next_reset, lang));
    (pct, body, note)
}

fn credit_line(c: &CreditMeter, now: i64, lang: Lang) -> String {
    let (pct, body, note) = credit_parts(c, now, lang);
    format!("{}  {} · {body}{note}", bar(pct), pct_str(pct))
}

fn today_usage(s: &SourceInfo, snap: &Snapshot, lang: Lang) -> String {
    let idle = ToolActivity::default();
    let a = snap.today.iter().find(|a| a.tool == s.id).unwrap_or(&idle);
    format!("{} {} · {} {}", lang.pick("hôm nay", "today"), amount(s, a, lang), a.prompts, lang.pick("lượt", "turns"))
}

/// A tool's usage today: credits if billed in credits, otherwise tokens.
fn amount(info: &SourceInfo, a: &ToolActivity, lang: Lang) -> String {
    if info.credits {
        format!("{} {}", fmt_credit(a.credits), credit_unit(lang))
    } else {
        format!("{} {}", fmt_tokens(a.tokens), token_unit(lang))
    }
}

/// Menu text: per-tool lines (in `sources` order, `line_count` lines per tool) and the "Today" line.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuText {
    pub tools: Vec<String>,
    pub today: String,
}

pub fn render(sources: &[SourceInfo], limits: &LimitState, snap: &Snapshot, now: i64, lang: Lang) -> MenuText {
    let width = sources.iter().map(|s| s.label.chars().count()).max().unwrap_or(0);
    let col = |label: &str| format!("{label}{}  ", NAME_PAD.to_string().repeat(width - label.chars().count()));
    let blank = col("");
    let mut tools = Vec::new();
    for s in sources {
        let name = col(&s.label);
        if s.limits {
            tools.push(name + &window(lang.pick("5 giờ", "5 hours"), limits.five_hour, false, limits.forecast_five_hour, limits, now, lang));
            tools.push(blank.clone() + &window(lang.pick("7 ngày", "7 days"), limits.seven_day, true, limits.forecast_seven_day, limits, now, lang));
        } else if let Some(c) = snap.credits.get(&s.id) {
            tools.push(name + &credit_line(c, now, lang));
        } else {
            tools.push(name + &today_usage(s, snap, lang));
        }
    }
    MenuText { tools, today: today_line(sources, snap, lang) }
}

fn today_line(sources: &[SourceInfo], snap: &Snapshot, lang: Lang) -> String {
    let active: Vec<String> = sources
        .iter()
        .filter_map(|s| snap.today.iter().find(|a| a.tool == s.id && (a.prompts > 0 || a.tokens > 0 || a.credits > 0.0)).map(|a| format!("{} {}", s.label, amount(s, a, lang))))
        .collect();
    let body = if active.is_empty() { lang.pick("chưa có hoạt động", "no activity yet").to_string() } else { active.join(" · ") };
    format!("{}: {body}", lang.pick("Hôm nay", "Today"))
}

/// Icon for a tray menu row.
#[derive(Debug, Clone, PartialEq)]
pub enum TrayIcon {
    /// No icon (row without %).
    None,
    /// Tool logo (header row).
    Logo,
    /// % ring: `pct` None = no data yet; `stale` = stale figure; `at_risk` = forecast says the limit comes before the reset.
    Ring { pct: Option<f64>, stale: bool, at_risk: bool },
}

/// A tray menu row belonging to tool `tool`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayRow {
    pub tool: String,
    pub text: String,
    pub icon: TrayIcon,
}

/// Tray menu: per-tool rows (in `sources` order, `tray_row_count` rows per tool), the "Today" line,
/// and the text next to the icon on the top bar (highest %, None if no %).
#[derive(Debug, Clone, PartialEq)]
pub struct TrayText {
    pub rows: Vec<TrayRow>,
    pub today: String,
    pub title: Option<String>,
}

/// Number of tray menu rows for a tool: 1 header + `line_count`.
pub fn tray_row_count(info: &SourceInfo) -> usize {
    1 + line_count(info)
}

pub fn render_tray(sources: &[SourceInfo], limits: &LimitState, snap: &Snapshot, now: i64, lang: Lang) -> TrayText {
    let mut rows = Vec::new();
    for s in sources {
        let row = |text: String, icon| TrayRow { tool: s.id.clone(), text, icon };
        rows.push(row(s.label.clone(), TrayIcon::Logo));
        if s.limits {
            let windows = [
                (lang.pick("5 giờ", "5 hours"), limits.five_hour, false, limits.forecast_five_hour),
                (lang.pick("7 ngày", "7 days"), limits.seven_day, true, limits.forecast_seven_day),
            ];
            for (name, w, by_date, f) in windows {
                rows.push(match w {
                    Some(w) => {
                        let (pct, when, fresh) = window_parts(w, by_date, limits, now, lang);
                        let icon = TrayIcon::Ring { pct: Some(pct), stale: limits.stale, at_risk: f.is_some_and(|f| f.at_risk) };
                        row(format!("{name} · {} · {}reset {when}{fresh}", pct_str(pct), out_part(f, by_date, lang)), icon)
                    }
                    None => row(no_data(name, lang), TrayIcon::Ring { pct: None, stale: false, at_risk: false }),
                });
            }
        } else if let Some(c) = snap.credits.get(&s.id) {
            let (pct, body, note) = credit_parts(c, now, lang);
            let stale = !note.is_empty();
            let icon = TrayIcon::Ring { pct: Some(pct), stale, at_risk: c.forecast.is_some_and(|f| f.at_risk) };
            rows.push(row(format!("{} · {} · {body}{note}", lang.pick("Tháng", "Month"), pct_str(pct)), icon));
        } else {
            rows.push(row(format!("{} · {}", s.label, today_usage(s, snap, lang)), TrayIcon::None));
        }
    }
    let title = rows
        .iter()
        .filter_map(|r| match r.icon {
            TrayIcon::Ring { pct: Some(p), .. } if !p.is_nan() => Some(p),
            _ => None,
        })
        .reduce(f64::max)
        .map(pct_str);
    TrayText { rows, today: today_line(sources, snap, lang), title }
}
