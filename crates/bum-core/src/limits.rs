//! 5h/7d limits: data from the statusline (`limits.json`) and 429 events (spec section 7.1).
use serde::{Deserialize, Serialize};

use crate::forecast::{self, Forecast, Sample};

/// Data older than this threshold does not raise an alert.
pub const STALE_SECS: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub used_percentage: f64,
    pub resets_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LimitsFile {
    pub updated_at: i64,
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Green,
    Yellow,
    Red,
    /// Stale % data: not colored as an alert (spec section 7.1).
    Grey,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Blocked {
    pub limit_type: String,
    pub resets_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LimitState {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    pub updated_at: Option<i64>,
    pub stale: bool,
    pub blocked: Option<Blocked>,
    /// Set by `with_forecast`; None when there is not enough evidence, the data is stale or the window is blocked.
    pub forecast_five_hour: Option<Forecast>,
    pub forecast_seven_day: Option<Forecast>,
}

/// A window past its reset time is treated as 0%.
fn live(w: Option<Window>, now: i64) -> Option<Window> {
    w.map(|w| if w.resets_at <= now { Window { used_percentage: 0.0, ..w } } else { w })
}

pub fn state(file: Option<LimitsFile>, last_block: Option<(String, i64)>, now: i64) -> LimitState {
    let updated_at = file.as_ref().map(|f| f.updated_at);
    let stale = updated_at.is_none_or(|t| now - t > STALE_SECS);
    let (five_hour, seven_day) = file.map(|f| (live(f.five_hour, now), live(f.seven_day, now))).unwrap_or_default();
    let blocked = last_block.filter(|(_, r)| *r > now).map(|(limit_type, resets_at)| Blocked { limit_type, resets_at });
    LimitState { five_hour, seven_day, updated_at, stale, blocked, forecast_five_hour: None, forecast_seven_day: None }
}

impl LimitState {
    /// Attach forecasts (spec §3.3): none when stale, blocked, already at 100% or past the reset;
    /// 5-hour samples must belong to the current window.
    pub fn with_forecast(mut self, five_hour_samples: &[Sample], now: i64) -> Self {
        if self.stale || self.blocked.is_some() {
            return self;
        }
        let open = |w: Option<Window>| w.filter(|w| w.used_percentage < 100.0 && w.resets_at > now);
        self.forecast_five_hour = open(self.five_hour).and_then(|w| forecast::five_hour(five_hour_samples, now).filter(|f| f.resets_at == w.resets_at));
        self.forecast_seven_day = open(self.seven_day).and_then(|w| forecast::seven_day(w.used_percentage, w.resets_at, now));
        self
    }

    pub fn at_risk(&self) -> bool {
        [self.forecast_five_hour, self.forecast_seven_day].iter().flatten().any(|f| f.at_risk)
    }

    pub fn max_percentage(&self) -> f64 {
        [self.five_hour, self.seven_day].iter().flatten().map(|w| w.used_percentage).fold(0.0, f64::max)
    }

    pub fn level(&self) -> Level {
        let pct = self.max_percentage();
        if self.blocked.is_some() {
            return Level::Red;
        }
        if self.stale {
            return Level::Grey;
        }
        if pct >= 90.0 {
            Level::Red
        } else if pct >= 70.0 || self.at_risk() {
            Level::Yellow
        } else {
            Level::Green
        }
    }
}

fn window_from(v: &serde_json::Value) -> Option<Window> {
    Some(Window { used_percentage: v["used_percentage"].as_f64()?, resets_at: v["resets_at"].as_i64()? })
}

/// Read `rate_limits` from the JSON that Claude Code passes to the statusline.
pub fn from_statusline(input: &serde_json::Value, now: i64) -> Option<LimitsFile> {
    let rl = input.get("rate_limits")?;
    let five_hour = window_from(&rl["five_hour"]);
    let seven_day = window_from(&rl["seven_day"]);
    (five_hour.is_some() || seven_day.is_some()).then_some(LimitsFile { updated_at: now, five_hour, seven_day })
}

/// Write to a temp file then rename, so readers never see a partial file.
pub fn write_atomic(path: &std::path::Path, f: &LimitsFile) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(f)?)?;
    std::fs::rename(&tmp, path)
}

pub fn read(path: &std::path::Path) -> Option<LimitsFile> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn hhmm(epoch: i64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(epoch, 0).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default()
}

/// Compact status line for the statusline / tray tooltip.
pub fn render_line(st: &LimitState, today_total: Option<u64>, lang: crate::i18n::Lang) -> String {
    let dot = match st.level() {
        Level::Green => "🟢",
        Level::Yellow => "🟡",
        Level::Red => "🔴",
        Level::Grey => "⚪",
    };
    let five = match st.five_hour {
        Some(w) => format!("5h {:.0}% ↻{}", w.used_percentage, hhmm(w.resets_at)),
        None => "5h —".to_string(),
    };
    let mut parts = vec![format!("{dot}{five}")];
    if let Some(h) = st.forecast_five_hour.filter(|f| f.at_risk).and_then(|f| f.hits_at) {
        parts.push(format!("{} ~{}", lang.pick("hết", "out"), hhmm(h)));
    }
    if let Some(w) = st.seven_day {
        parts.push(format!("7d {:.0}%", w.used_percentage));
    }
    if let Some(b) = &st.blocked {
        parts.push(format!("{} ↻{}", lang.pick("hết lượt", "limited"), hhmm(b.resets_at)));
    }
    if let Some(t) = today_total {
        parts.push(format!("{} {}", lang.pick("hôm nay", "today"), crate::insight::fmt_tokens(t)));
    }
    parts.join(" · ")
}
