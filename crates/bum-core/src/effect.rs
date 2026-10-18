//! "Where did the quota go without a result?" — post-hoc analysis of the local history.
//!
//! Only signals the history records as facts are used: a turn the user interrupted (Esc), a turn
//! that started with a very large context, and context overflows (compactions). Guessing from the
//! wording of the next message ("no, …", "sửa lại …") was tried and dropped: on real history most
//! matches were new requests, not corrections. No model is called, so this costs no tokens, and
//! nothing leaves the machine.
//!
//! Token amounts are `Usage::quota_weight()`, not raw sums: raw sums are almost all cache reads,
//! which weigh little against a rate limit.
use std::collections::HashMap;

use chrono::DateTime;
use serde::Serialize;

use crate::store::queries::PromptListItem;

/// An interrupted turn counts as a *late* cancel when the model had already been working this long.
pub const LATE_CANCEL_SECS: i64 = 120;
/// A session that overflowed its context this many times in the range is worth splitting.
pub const SPLIT_AFTER_COMPACTIONS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// The tokens were spent: the number is a fact from the log.
    Sure,
    /// The tokens were spent, but whether acting on the advice saves them is an estimate.
    Likely,
}

/// One piece of advice. The UI turns `code` into a sentence; the numbers are its parameters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Advice {
    /// `late_cancel` | `big_context` | `split_session`
    pub code: &'static str,
    pub confidence: Confidence,
    /// Turns (`late_cancel`, `big_context`) or sessions (`split_session`).
    pub count: u64,
    /// Quota-weighted tokens of those turns.
    pub weighted: u64,
    /// For `split_session`: the session that overflowed most, so the UI can open it.
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Effect {
    pub turns: u64,
    pub sessions: u64,
    /// Quota-weighted tokens of every turn in the range.
    pub weighted: u64,
    /// Turns the user cut off (Esc).
    pub interrupted_turns: u64,
    pub interrupted_weighted: u64,
    /// Turns that started with more context than `context_threshold`.
    pub big_context_turns: u64,
    pub big_context_weighted: u64,
    /// Context overflows (`/compact` and auto-compaction).
    pub compactions: u64,
    /// Most weighted first. Empty when there is nothing worth saying.
    pub advice: Vec<Advice>,
}

/// Seconds between the question and the last model call of the turn, when both are known.
fn run_secs(it: &PromptListItem) -> Option<i64> {
    let start = DateTime::parse_from_rfc3339(&it.stats.ts).ok()?;
    let end = DateTime::parse_from_rfc3339(it.stats.last_call_ts.as_deref()?).ok()?;
    Some((end - start).num_seconds())
}

/// `compactions` = (session id, overflows in the range) for the same tool and range as `items`.
pub fn summarize(items: &[PromptListItem], compactions: &[(String, u64)], context_threshold: u64) -> Effect {
    let mut weighted = 0u64;
    let mut interrupted = (0u64, 0u64);
    let mut late = (0u64, 0u64);
    let mut big = (0u64, 0u64);
    let mut by_session: HashMap<&str, u64> = HashMap::new();

    for it in items {
        let w = it.stats.usage.quota_weight();
        weighted += w;
        *by_session.entry(it.stats.session_id.as_str()).or_default() += w;
        if it.stats.interrupted {
            interrupted = (interrupted.0 + 1, interrupted.1 + w);
            if run_secs(it).is_some_and(|s| s >= LATE_CANCEL_SECS) {
                late = (late.0 + 1, late.1 + w);
            }
        }
        if it.stats.context_at_start > context_threshold {
            big = (big.0 + 1, big.1 + w);
        }
    }

    // Sessions that overflowed again and again; the worst one (most overflows, then most weight,
    // then id for a stable answer) is the one to open.
    let mut split: Vec<(&str, u64, u64)> = compactions
        .iter()
        .filter(|(_, n)| *n >= SPLIT_AFTER_COMPACTIONS)
        .map(|(sid, n)| (sid.as_str(), *n, by_session.get(sid.as_str()).copied().unwrap_or(0)))
        .collect();
    split.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(b.0)));

    let mut advice = Vec::new();
    if late.0 > 0 {
        advice.push(Advice { code: "late_cancel", confidence: Confidence::Sure, count: late.0, weighted: late.1, session_id: None });
    }
    if big.0 > 0 {
        advice.push(Advice { code: "big_context", confidence: Confidence::Likely, count: big.0, weighted: big.1, session_id: None });
    }
    if let Some(worst) = split.first() {
        advice.push(Advice {
            code: "split_session",
            confidence: Confidence::Likely,
            count: split.len() as u64,
            weighted: split.iter().map(|s| s.2).sum(),
            session_id: Some(worst.0.to_string()),
        });
    }
    advice.sort_by(|a, b| b.weighted.cmp(&a.weighted).then(a.code.cmp(b.code)));

    Effect {
        turns: items.len() as u64,
        sessions: by_session.len() as u64,
        weighted,
        interrupted_turns: interrupted.0,
        interrupted_weighted: interrupted.1,
        big_context_turns: big.0,
        big_context_weighted: big.1,
        compactions: compactions.iter().map(|(_, n)| n).sum(),
        advice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Usage;
    use crate::store::queries::{PromptListItem, PromptStats};

    const THRESHOLD: u64 = 300_000;

    fn turn(session: &str, ts: &str, last: Option<&str>, usage: Usage, interrupted: bool, context_at_start: u64) -> PromptListItem {
        PromptListItem {
            stats: PromptStats {
                key: format!("{session}-{ts}"),
                tool: "claude_code".into(),
                credits: 0.0,
                models: vec!["claude".into()],
                session_id: session.into(),
                project: "p".into(),
                ts: ts.into(),
                text: "x".into(),
                interrupted,
                usage,
                api_calls: 1,
                context_at_start,
                last_call_ts: last.map(String::from),
            },
            starred: false,
        }
    }

    fn input(n: u64) -> Usage {
        Usage { input: n, ..Default::default() }
    }

    #[test]
    fn quota_weight_discounts_cache_reads_and_weighs_output() {
        let u = Usage { input: 100, cache_write_5m: 100, cache_write_1h: 100, cache_read: 1000, output: 10, reasoning: 10 };
        assert_eq!(u.quota_weight(), 100 + 125 + 200 + 100 + 100);
        // A turn that is 98% cache reads by raw count is not 98% of the quota.
        let reread = Usage { cache_read: 980_000, input: 20_000, ..Default::default() };
        assert_eq!(reread.quota_weight(), 98_000 + 20_000);
    }

    #[test]
    fn empty_history_is_all_zeros() {
        let e = summarize(&[], &[], THRESHOLD);
        assert_eq!((e.turns, e.sessions, e.weighted, e.compactions), (0, 0, 0, 0));
        assert!(e.advice.is_empty());
    }

    #[test]
    fn only_long_runs_are_late_cancels() {
        let items = vec![
            // Cut off after 5 minutes: late.
            turn("a", "2026-10-10T01:00:00.000Z", Some("2026-10-10T01:05:00.000Z"), input(1000), true, 0),
            // Cut off after 30 s: a quick redirect, counted as interrupted but not as late.
            turn("a", "2026-10-10T02:00:00.000Z", Some("2026-10-10T02:00:30.000Z"), input(100), true, 0),
            // No model call recorded: duration unknown, not late.
            turn("b", "2026-10-10T03:00:00.000Z", None, input(10), true, 0),
            turn("b", "2026-10-10T04:00:00.000Z", Some("2026-10-10T04:09:00.000Z"), input(5), false, 0),
        ];
        let e = summarize(&items, &[], THRESHOLD);
        assert_eq!((e.turns, e.sessions, e.weighted), (4, 2, 1115));
        assert_eq!((e.interrupted_turns, e.interrupted_weighted), (3, 1110));
        assert_eq!(e.advice, vec![Advice { code: "late_cancel", confidence: Confidence::Sure, count: 1, weighted: 1000, session_id: None }]);
    }

    #[test]
    fn big_context_turns_use_the_configured_threshold() {
        let items = vec![
            turn("a", "2026-10-10T01:00:00.000Z", None, input(700), false, THRESHOLD + 1),
            turn("a", "2026-10-10T02:00:00.000Z", None, input(50), false, THRESHOLD),
        ];
        let e = summarize(&items, &[], THRESHOLD);
        assert_eq!((e.big_context_turns, e.big_context_weighted), (1, 700));
        assert_eq!(e.advice[0].code, "big_context");
        assert_eq!(e.advice[0].confidence, Confidence::Likely);
    }

    #[test]
    fn repeated_overflows_point_at_the_worst_session() {
        let items = vec![
            turn("a", "2026-10-10T01:00:00.000Z", None, input(400), false, 0),
            turn("b", "2026-10-10T02:00:00.000Z", None, input(300), false, 0),
            turn("c", "2026-10-10T03:00:00.000Z", None, input(9000), false, 0),
        ];
        // c overflowed once: below the bar. b overflowed most, so it is the one to open.
        let compactions = vec![("a".to_string(), 2), ("b".to_string(), 3), ("c".to_string(), 1)];
        let e = summarize(&items, &compactions, THRESHOLD);
        assert_eq!(e.compactions, 6);
        assert_eq!(e.advice, vec![Advice { code: "split_session", confidence: Confidence::Likely, count: 2, weighted: 700, session_id: Some("b".into()) }]);
    }

    #[test]
    fn advice_is_ordered_by_weight() {
        let items = vec![
            turn("a", "2026-10-10T01:00:00.000Z", Some("2026-10-10T01:10:00.000Z"), input(100), true, 0),
            turn("a", "2026-10-10T02:00:00.000Z", None, input(5000), false, THRESHOLD + 1),
        ];
        let e = summarize(&items, &[], THRESHOLD);
        let codes: Vec<&str> = e.advice.iter().map(|a| a.code).collect();
        assert_eq!(codes, vec!["big_context", "late_cancel"]);
    }
}
