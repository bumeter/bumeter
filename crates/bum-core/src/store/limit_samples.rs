//! 5h/7d percentage readings over time, the input of the quota forecast (quota-forecast spec §2).
use rusqlite::{params, OptionalExtension};

use super::Store;
use crate::forecast::Sample;
use crate::limits::LimitsFile;

/// Same percentage again within this gap: not stored (the statusline can refresh every second).
pub const MIN_GAP_SECS: i64 = 60;
/// Readings older than this are deleted (enough for the 7-day window).
pub const KEEP_SECS: i64 = 8 * 86_400;

impl Store {
    /// Store a reading unless it repeats the latest one within `MIN_GAP_SECS` or is not newer. Returns whether it was stored.
    pub fn record_limit_sample(&mut self, win: &str, s: Sample) -> rusqlite::Result<bool> {
        let last: Option<(i64, f64)> = self
            .conn()
            .query_row("SELECT ts, pct FROM limit_samples WHERE win = ?1 ORDER BY ts DESC LIMIT 1", params![win], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        if let Some((ts, pct)) = last {
            if s.ts <= ts || (pct == s.pct && s.ts - ts < MIN_GAP_SECS) {
                return Ok(false);
            }
        }
        self.conn().execute("INSERT INTO limit_samples(win, ts, pct, resets_at) VALUES (?1, ?2, ?3, ?4)", params![win, s.ts, s.pct, s.resets_at])?;
        Ok(true)
    }

    /// Readings of `win` at or after `since`, oldest first.
    pub fn limit_samples_since(&self, win: &str, since: i64) -> rusqlite::Result<Vec<Sample>> {
        let mut stmt = self.conn().prepare("SELECT ts, pct, resets_at FROM limit_samples WHERE win = ?1 AND ts >= ?2 ORDER BY ts")?;
        let rows = stmt.query_map(params![win, since], |r| Ok(Sample { ts: r.get(0)?, pct: r.get(1)?, resets_at: r.get(2)? }))?;
        rows.collect()
    }

    pub fn prune_limit_samples(&mut self, before: i64) -> rusqlite::Result<usize> {
        self.conn().execute("DELETE FROM limit_samples WHERE ts < ?1", params![before])
    }
}

/// Host side: store the readings of a fresh `limits.json`; a file dated in the future (clock skew) is ignored.
pub fn record_file(store: &mut Store, file: &LimitsFile, now: i64) -> rusqlite::Result<()> {
    if file.updated_at > now {
        return Ok(());
    }
    for (win, w) in [("five_hour", file.five_hour), ("seven_day", file.seven_day)] {
        if let Some(w) = w {
            store.record_limit_sample(win, Sample { ts: file.updated_at, pct: w.used_percentage, resets_at: w.resets_at })?;
        }
    }
    store.prune_limit_samples(now - KEEP_SECS)?;
    Ok(())
}
