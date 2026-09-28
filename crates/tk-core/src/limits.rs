//! Giới hạn 5h/7d: dữ liệu từ statusline (`limits.json`) và sự kiện 429 (spec mục 7.1).
use serde::{Deserialize, Serialize};

/// Dữ liệu cũ hơn ngưỡng này thì không báo động.
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
    /// Dữ liệu % đã cũ: không tô màu báo động (spec mục 7.1).
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
}

/// Cửa sổ đã qua giờ reset thì coi như 0%.
fn live(w: Option<Window>, now: i64) -> Option<Window> {
    w.map(|w| if w.resets_at <= now { Window { used_percentage: 0.0, ..w } } else { w })
}

pub fn state(file: Option<LimitsFile>, last_block: Option<(String, i64)>, now: i64) -> LimitState {
    let updated_at = file.as_ref().map(|f| f.updated_at);
    let stale = updated_at.is_none_or(|t| now - t > STALE_SECS);
    let (five_hour, seven_day) = file.map(|f| (live(f.five_hour, now), live(f.seven_day, now))).unwrap_or_default();
    let blocked = last_block.filter(|(_, r)| *r > now).map(|(limit_type, resets_at)| Blocked { limit_type, resets_at });
    LimitState { five_hour, seven_day, updated_at, stale, blocked }
}

impl LimitState {
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
        } else if pct >= 70.0 {
            Level::Yellow
        } else {
            Level::Green
        }
    }
}

fn window_from(v: &serde_json::Value) -> Option<Window> {
    Some(Window { used_percentage: v["used_percentage"].as_f64()?, resets_at: v["resets_at"].as_i64()? })
}

/// Đọc `rate_limits` từ JSON mà Claude Code truyền vào statusline.
pub fn from_statusline(input: &serde_json::Value, now: i64) -> Option<LimitsFile> {
    let rl = input.get("rate_limits")?;
    let five_hour = window_from(&rl["five_hour"]);
    let seven_day = window_from(&rl["seven_day"]);
    (five_hour.is_some() || seven_day.is_some()).then_some(LimitsFile { updated_at: now, five_hour, seven_day })
}

/// Ghi ra file tạm rồi đổi tên, để người đọc không bao giờ thấy file dở.
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

/// Dòng trạng thái gọn cho statusline / tooltip tray.
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
