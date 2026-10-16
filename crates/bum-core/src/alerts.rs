//! Decide when to send notifications, deduplicating via the `alerts_sent` table (spec section 7.3).
use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, SecondsFormat, TimeZone, Utc};

use crate::config::Config;
use crate::i18n::Lang;
use crate::engine::IngestReport;
use crate::insight::{anomaly, explain, fmt_tokens, median};
use crate::limits::LimitState;
use crate::store::Store;

/// A turn is considered finished when there are no new calls within this interval
/// (long enough not to interrupt a running build/test command).
const IDLE: Duration = Duration::from_secs(45);
/// Only send the "reset" notification when the reset time is still fresh.
const RESET_FRESH_SECS: i64 = 600;

#[derive(Debug, Clone, PartialEq)]
pub enum Alert {
    Quota { window: &'static str, pct: f64, threshold: u8, resets_at: i64 },
    Blocked { limit_type: String, resets_at: i64 },
    Reset { limit_type: String, resets_at: i64 },
    Anomaly { prompt_key: String, text: String, total: u64, ratio: Option<f64>, explanation: crate::insight::Explain },
    Context { session_id: String, project: String, context: u64, threshold: u64 },
}

fn hhmm(epoch: i64) -> String {
    Local.timestamp_opt(epoch, 0).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default()
}

fn window_label(w: &str, lang: Lang) -> &'static str {
    if w == "seven_day" { lang.pick("7 ngày", "7-day") } else { lang.pick("5 giờ", "5-hour") }
}

impl Alert {
    pub fn key(&self) -> String {
        match self {
            Alert::Quota { window, threshold, resets_at, .. } => format!("quota:{window}:{resets_at}:{threshold}"),
            Alert::Blocked { limit_type, resets_at } => format!("blocked:{limit_type}:{resets_at}"),
            Alert::Reset { limit_type, resets_at } => format!("reset:{limit_type}:{resets_at}"),
            Alert::Anomaly { prompt_key, .. } => format!("anomaly:{prompt_key}"),
            Alert::Context { session_id, threshold, .. } => format!("ctx:{session_id}:{threshold}"),
        }
    }

    pub fn title(&self, lang: Lang) -> String {
        match self {
            Alert::Quota { .. } => "Quota",
            Alert::Blocked { .. } => lang.pick("Hết lượt", "Limit reached"),
            Alert::Reset { .. } => lang.pick("Đã reset", "Limit reset"),
            Alert::Anomaly { .. } => lang.pick("Lượt tốn bất thường", "Unusually costly turn"),
            Alert::Context { .. } => lang.pick("Context lớn", "Large context"),
        }
        .into()
    }

    pub fn body(&self, lang: Lang) -> String {
        match (self, lang) {
            (Alert::Quota { window, pct, resets_at, .. }, Lang::Vi) => {
                format!("Đã dùng {pct:.0}% giới hạn {}. Reset lúc {}.", window_label(window, lang), hhmm(*resets_at))
            }
            (Alert::Quota { window, pct, resets_at, .. }, Lang::En) => {
                format!("Used {pct:.0}% of the {} limit. Resets at {}.", window_label(window, lang), hhmm(*resets_at))
            }
            (Alert::Blocked { limit_type, resets_at }, Lang::Vi) => {
                format!("Đã chạm giới hạn {}. Reset lúc {}.", window_label(limit_type, lang), hhmm(*resets_at))
            }
            (Alert::Blocked { limit_type, resets_at }, Lang::En) => {
                format!("Hit the {} limit. Resets at {}.", window_label(limit_type, lang), hhmm(*resets_at))
            }
            (Alert::Reset { limit_type, .. }, Lang::Vi) => format!("Giới hạn {} đã mở lại.", window_label(limit_type, lang)),
            (Alert::Reset { limit_type, .. }, Lang::En) => format!("The {} limit is open again.", window_label(limit_type, lang)),
            (Alert::Anomaly { text, total, ratio, explanation, .. }, _) => {
                let short: String = text.chars().take(40).collect();
                let times = ratio.map(|r| format!(" ({r:.0}× {})", lang.pick("TB", "median"))).unwrap_or_default();
                format!("\"{short}\" {} {}{times}. {}", lang.pick("tốn", "used"), fmt_tokens(*total), explanation.text(lang))
            }
            (Alert::Context { project, context, .. }, Lang::Vi) => {
                format!("Hội thoại {project} đang ở {}. Nên /compact hoặc mở phiên mới.", fmt_tokens(*context))
            }
            (Alert::Context { project, context, .. }, Lang::En) => {
                format!("Conversation {project} is at {}. Consider /compact or a new session.", fmt_tokens(*context))
            }
        }
    }
}

pub struct AlertEngine {
    cfg: Config,
    started_at: String,
    pending: HashMap<String, Instant>,
}

fn rfc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

impl AlertEngine {
    /// Only data produced at or after `started_at` is notified.
    pub fn new(cfg: Config, started_at: DateTime<Utc>) -> Self {
        Self { cfg, started_at: rfc(started_at), pending: HashMap::new() }
    }

    fn keep(store: &mut Store, out: &mut Vec<Alert>, alert: Alert, now: DateTime<Utc>) -> rusqlite::Result<()> {
        if store.alert_once(&alert.key(), &rfc(now))? {
            out.push(alert);
        }
        Ok(())
    }

    pub fn on_ingest(&mut self, store: &mut Store, report: &IngestReport, now: Instant, now_utc: DateTime<Utc>) -> rusqlite::Result<Vec<Alert>> {
        let mut out = Vec::new();
        if self.cfg.notify.blocked {
            for (limit_type, resets_at) in &report.summary.new_rate_limits {
                if *resets_at > now_utc.timestamp() {
                    Self::keep(store, &mut out, Alert::Blocked { limit_type: limit_type.clone(), resets_at: *resets_at }, now_utc)?;
                }
            }
        }
        for key in &report.summary.touched_prompts {
            self.pending.insert(key.clone(), now);
        }
        if self.cfg.notify.context {
            let threshold = self.cfg.context_threshold;
            for sid in &report.summary.touched_sessions {
                let Some((context, ts)) = store.session_current_context(sid)? else { continue };
                if context > threshold && ts >= self.started_at {
                    let project = store.session_project(sid)?.unwrap_or_default();
                    Self::keep(store, &mut out, Alert::Context { session_id: sid.clone(), project, context, threshold }, now_utc)?;
                }
            }
        }
        Ok(out)
    }

    pub fn on_tick(&mut self, store: &mut Store, now: Instant, now_utc: DateTime<Utc>) -> rusqlite::Result<Vec<Alert>> {
        let mut out = Vec::new();
        let idle: Vec<String> = self.pending.iter().filter(|(_, t)| now.duration_since(**t) >= IDLE).map(|(k, _)| k.clone()).collect();
        if !idle.is_empty() {
            let mut totals = store.prompt_totals_since(&rfc(now_utc - chrono::Duration::days(30)))?;
            let med = median(&mut totals);
            for key in idle {
                self.pending.remove(&key);
                let Some(stats) = store.prompt_stats(&key)? else { continue };
                if stats.ts < self.started_at {
                    continue;
                }
                let total = stats.usage.total();
                // Only alert when clearly above the median; a merely large context has its own Context alert.
                let over_median = |a: &crate::insight::Anomaly| a.ratio.is_some_and(|r| r > self.cfg.anomaly_multiplier);
                if let Some(a) = anomaly(total, stats.context_at_start, med, &self.cfg).filter(over_median) {
                    let explanation = explain(&stats.usage, stats.context_at_start);
                    // Always record the anomaly marker (used by the Bum face); sending the notification depends on the `notify.anomaly` flag.
                    let alert = Alert::Anomaly { prompt_key: key, text: stats.text, total, ratio: a.ratio, explanation };
                    if store.alert_once(&alert.key(), &rfc(now_utc))? && self.cfg.notify.anomaly {
                        out.push(alert);
                    }
                }
            }
        }
        if self.cfg.notify.blocked {
            if let Some((limit_type, resets_at)) = store.latest_rate_limit()? {
                let now_s = now_utc.timestamp();
                let started = DateTime::parse_from_rfc3339(&self.started_at).map(|t| t.timestamp()).unwrap_or(i64::MAX);
                if resets_at <= now_s && resets_at > now_s - RESET_FRESH_SECS && resets_at > started {
                    Self::keep(store, &mut out, Alert::Reset { limit_type, resets_at }, now_utc)?;
                }
            }
        }
        Ok(out)
    }

    /// Only alert for the highest threshold just crossed, once per threshold per reset cycle.
    pub fn on_limits(&mut self, store: &mut Store, st: &LimitState) -> rusqlite::Result<Vec<Alert>> {
        let mut out = Vec::new();
        if st.stale || !self.cfg.notify.quota {
            return Ok(out);
        }
        let mut thresholds = self.cfg.quota_thresholds.clone();
        thresholds.sort_unstable_by(|a, b| b.cmp(a));
        let now_utc = Utc::now();
        for (window, w) in [("five_hour", st.five_hour), ("seven_day", st.seven_day)] {
            let Some(w) = w else { continue };
            if let Some(&threshold) = thresholds.iter().find(|t| w.used_percentage >= f64::from(**t)) {
                Self::keep(store, &mut out, Alert::Quota { window, pct: w.used_percentage, threshold, resets_at: w.resets_at }, now_utc)?;
            }
        }
        Ok(out)
    }
}
