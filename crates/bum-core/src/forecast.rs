//! Quota forecast: when the limit is reached at the current pace (quota-forecast spec §3). Pure functions.
use serde::{Deserialize, Serialize};

/// 5-hour window: only samples from the last 30 minutes count.
pub const FIVE_HOUR_LOOKBACK: i64 = 30 * 60;
/// 7-day window: no forecast during the first 12 hours (one busy hour would extrapolate wildly).
pub const SEVEN_DAY_MIN_ELAPSED: i64 = 12 * 3600;
/// Monthly credits: no forecast during the first 2 days of the period.
pub const CREDIT_MIN_ELAPSED: i64 = 2 * 86_400;
const SEVEN_DAYS: i64 = 7 * 86_400;
const MIN_SAMPLES: usize = 3;
const MIN_SPAN: i64 = 10 * 60;
const MIN_RISE: f64 = 2.0;

/// One reading of a window's percentage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub ts: i64,
    pub pct: f64,
    pub resets_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Forecast {
    /// % per hour (Claude) or credits per hour (credit tools).
    pub rate_per_hour: f64,
    /// When the limit is reached at this rate; None when the rate is zero.
    pub hits_at: Option<i64>,
    pub resets_at: i64,
    /// The limit comes before the reset.
    pub at_risk: bool,
}

fn build(rate_per_sec: f64, remaining: f64, now: i64, resets_at: i64) -> Forecast {
    let hits_at = (rate_per_sec > 0.0).then(|| now + (remaining.max(0.0) / rate_per_sec).round() as i64);
    Forecast { rate_per_hour: rate_per_sec * 3600.0, hits_at, resets_at, at_risk: hits_at.is_some_and(|h| h < resets_at) }
}

/// 5-hour window: least-squares slope of the recent samples of the current window (spec §3.1).
pub fn five_hour(samples: &[Sample], now: i64) -> Option<Forecast> {
    let mut s: Vec<Sample> = samples.iter().copied().filter(|x| x.ts <= now).collect();
    s.sort_by_key(|x| x.ts);
    let current = s.last()?.resets_at;
    s.retain(|x| x.resets_at == current && x.ts >= now - FIVE_HOUR_LOOKBACK);
    if let Some(i) = s.windows(2).rposition(|w| w[1].pct < w[0].pct) {
        s.drain(..=i);
    }
    let (first, last) = (*s.first()?, *s.last()?);
    if s.len() < MIN_SAMPLES || last.ts - first.ts < MIN_SPAN || last.pct - first.pct < MIN_RISE {
        return None;
    }
    let n = s.len() as f64;
    let mean_t = s.iter().map(|x| x.ts as f64).sum::<f64>() / n;
    let mean_p = s.iter().map(|x| x.pct).sum::<f64>() / n;
    let num: f64 = s.iter().map(|x| (x.ts as f64 - mean_t) * (x.pct - mean_p)).sum();
    let den: f64 = s.iter().map(|x| (x.ts as f64 - mean_t).powi(2)).sum();
    (den > 0.0).then(|| build(num / den, 100.0 - last.pct, now, current))
}

/// Average pace since `start`: used for long windows where the recent slope swings with working hours (spec §3.2).
/// None once the limit is reached: the "limited" state is shown instead (spec §3.3).
pub fn paced(used: f64, limit: f64, start: i64, end: i64, now: i64, min_elapsed: i64) -> Option<Forecast> {
    let elapsed = now - start;
    if limit <= 0.0 || used <= 0.0 || used >= limit || elapsed < min_elapsed.max(1) || now >= end {
        return None;
    }
    Some(build(used / elapsed as f64, limit - used, now, end))
}

pub fn seven_day(pct: f64, resets_at: i64, now: i64) -> Option<Forecast> {
    paced(pct, 100.0, resets_at - SEVEN_DAYS, resets_at, now, SEVEN_DAY_MIN_ELAPSED)
}
