//! "Did the tokens produce a result?" — post-hoc analysis of the local history.
//!
//! Everything here is a *behavioural proxy*, computed from rows that are already in the local
//! database: a turn the user interrupted, or a turn the user immediately corrected, is very
//! likely work that had to be redone. No model is called (so this costs no tokens), no semantic
//! judgement is made, and nothing leaves the machine.
use std::collections::HashMap;

use serde::Serialize;

use crate::store::queries::PromptListItem;

/// A turn is treated as "corrected" when the *next* turn in the same session opens with one of
/// these (lowercased, English + Vietnamese). Kept deliberately short to limit false positives:
/// a bare "no" is *not* enough, "no," is.
const CORRECTIONS: &[&str] = &[
    "no,",
    "no.",
    "nope",
    "wrong",
    "not right",
    "that's not",
    "thats not",
    "doesn't work",
    "does not work",
    "still",
    "again",
    "undo",
    "revert",
    "broken",
    "try again",
    "fix it",
    "fix this",
    "sai",
    "không đúng",
    "khong dung",
    "không phải",
    "khong phai",
    "sửa lại",
    "sua lai",
    "làm lại",
    "lam lai",
    "vẫn lỗi",
    "van loi",
    "chưa đúng",
    "chua dung",
    "thử lại",
    "thu lai",
    "hỏng",
    "không chạy",
    "khong chay",
    "lỗi rồi",
    "đâu có",
    "không được",
    "khong duoc",
];

/// Only the beginning of a follow-up is inspected: a correction is short and comes first, so a
/// long prompt that merely mentions "again" somewhere in the middle is not counted.
const CORRECTION_PREFIX_CHARS: usize = 80;

/// Word-boundary containment, so "sai" does not match "saigon" and "still" does not match
/// "stillness". `hay` is already lowercased.
fn contains_word(hay: &[char], needle: &[char]) -> bool {
    if needle.is_empty() || needle.len() > hay.len() {
        return false;
    }
    let before_ok = |i: usize| i == 0 || !hay[i - 1].is_alphanumeric();
    let after_ok = |j: usize| j == hay.len() || !hay[j].is_alphanumeric();
    for i in 0..=hay.len() - needle.len() {
        let j = i + needle.len();
        if &hay[i..j] == needle && before_ok(i) && after_ok(j) {
            return true;
        }
    }
    false
}

pub fn looks_like_correction(text: &str) -> bool {
    let head: String = text.chars().take(CORRECTION_PREFIX_CHARS).collect::<String>().to_lowercase();
    let hay: Vec<char> = head.chars().collect();
    CORRECTIONS.iter().any(|n| head.contains(n) && contains_word(&hay, &n.chars().collect::<Vec<char>>()))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectWaste {
    pub project: String,
    pub wasted: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Effect {
    pub turns: u64,
    pub sessions: u64,
    pub total_tokens: u64,
    /// Turns the user cut off (Esc).
    pub interrupted_turns: u64,
    /// Turns the user immediately corrected with the next message.
    pub rework_turns: u64,
    /// Interrupted or corrected (union, each turn counted once).
    pub waste_turns: u64,
    /// Tokens spent on `waste_turns`.
    pub wasted_tokens: u64,
    /// `wasted_tokens / total_tokens` in percent, one decimal.
    pub waste_pct: f64,
    /// Context overflows (`/compact` and auto-compaction).
    pub compactions: u64,
    /// Up to three projects with the most wasted tokens.
    pub worst_projects: Vec<ProjectWaste>,
}

pub fn summarize(items: &[PromptListItem], compactions: u64) -> Effect {
    // Group by session: "the next turn" only means something inside one conversation.
    let mut by_session: HashMap<&str, Vec<&PromptListItem>> = HashMap::new();
    for it in items {
        by_session.entry(it.stats.session_id.as_str()).or_default().push(it);
    }

    let mut total_tokens = 0u64;
    let mut interrupted_turns = 0u64;
    let mut rework_turns = 0u64;
    let mut waste_turns = 0u64;
    let mut wasted_tokens = 0u64;
    let mut by_project: HashMap<String, u64> = HashMap::new();

    for list in by_session.values_mut() {
        list.sort_by(|a, b| a.stats.ts.cmp(&b.stats.ts));
        for (i, it) in list.iter().enumerate() {
            let tokens = it.stats.usage.total();
            let interrupted = it.stats.interrupted;
            // The last turn of a session is never "corrected": there is no follow-up yet.
            let corrected = list.get(i + 1).is_some_and(|next| looks_like_correction(&next.stats.text));
            total_tokens += tokens;
            if interrupted {
                interrupted_turns += 1;
            }
            if corrected {
                rework_turns += 1;
            }
            if interrupted || corrected {
                waste_turns += 1;
                wasted_tokens += tokens;
                if !it.stats.project.is_empty() {
                    *by_project.entry(it.stats.project.clone()).or_default() += tokens;
                }
            }
        }
    }

    let mut worst_projects: Vec<ProjectWaste> =
        by_project.into_iter().map(|(project, wasted)| ProjectWaste { project, wasted }).collect();
    // Deterministic order (and stable tests): most wasted first, ties by name.
    worst_projects.sort_by(|a, b| b.wasted.cmp(&a.wasted).then_with(|| a.project.cmp(&b.project)));
    worst_projects.truncate(3);

    let waste_pct = if total_tokens == 0 { 0.0 } else { (wasted_tokens as f64 / total_tokens as f64 * 1000.0).round() / 10.0 };

    Effect {
        turns: items.len() as u64,
        sessions: by_session.len() as u64,
        total_tokens,
        interrupted_turns,
        rework_turns,
        waste_turns,
        wasted_tokens,
        waste_pct,
        compactions,
        worst_projects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Usage;
    use crate::store::queries::{PromptListItem, PromptStats};

    fn turn(session: &str, ts: &str, text: &str, tokens: u64, interrupted: bool, project: &str) -> PromptListItem {
        PromptListItem {
            stats: PromptStats {
                key: format!("{session}-{ts}"),
                tool: "claude_code".into(),
                credits: 0.0,
                models: vec!["claude".into()],
                session_id: session.into(),
                project: project.into(),
                ts: ts.into(),
                text: text.into(),
                interrupted,
                usage: Usage { input: tokens, ..Default::default() },
                api_calls: 1,
                context_at_start: 0,
                last_call_ts: None,
            },
            starred: false,
        }
    }

    #[test]
    fn correction_needs_a_word_boundary() {
        // Real corrections, both languages.
        assert!(looks_like_correction("no, that's not it"));
        assert!(looks_like_correction("Still broken"));
        assert!(looks_like_correction("Sai rồi, làm lại đi"));
        assert!(looks_like_correction("vẫn lỗi như cũ"));
        assert!(looks_like_correction("please undo that"));
        // Not corrections: substrings inside other words, or a "no" without a comma.
        assert!(!looks_like_correction("Node is fine now"));
        assert!(!looks_like_correction("no need for that"));
        assert!(!looks_like_correction("add a test for stillness"));
        assert!(!looks_like_correction("the saigon map needs an update"));
        // Only the head of the message counts.
        assert!(!looks_like_correction(&format!("{} still", "x".repeat(90))));
    }

    #[test]
    fn empty_history_is_all_zeros() {
        let e = summarize(&[], 0);
        assert_eq!(e.turns, 0);
        assert_eq!(e.sessions, 0);
        assert_eq!(e.wasted_tokens, 0);
        assert_eq!(e.waste_pct, 0.0);
        assert!(e.worst_projects.is_empty());
    }

    #[test]
    fn counts_interrupts_and_corrections_without_double_counting() {
        let items = vec![
            // Session a, in file order (newest first) on purpose: summarize must sort by ts.
            turn("a", "2026-10-10T03:00:00.000Z", "try again please", 300, false, "proj"),
            turn("a", "2026-10-10T02:00:00.000Z", "write the parser", 200, false, "proj"),
            turn("a", "2026-10-10T01:00:00.000Z", "make the tests pass", 100, true, "proj"),
            // Session b: a single turn, so it can never be "corrected".
            turn("b", "2026-10-10T04:00:00.000Z", "still one more thing", 50, false, "other"),
            // Session c: the work was cut off *and* the user immediately corrected it -> one turn,
            // counted once in the union.
            turn("c", "2026-10-10T05:30:00.000Z", "no, that's wrong", 40, false, "proj"),
            turn("c", "2026-10-10T05:00:00.000Z", "write a helper", 10, true, "proj"),
        ];
        let e = summarize(&items, 7);
        assert_eq!(e.turns, 6);
        assert_eq!(e.sessions, 3);
        assert_eq!(e.total_tokens, 700);
        assert_eq!(e.interrupted_turns, 2); // a@01:00, c@05:00
        assert_eq!(e.rework_turns, 2); // a@02:00 (next: "try again"), c@05:00 (next: "no, ...")
        assert_eq!(e.waste_turns, 3); // a@01:00, a@02:00, c@05:00
        assert_eq!(e.wasted_tokens, 100 + 200 + 10);
        assert_eq!(e.waste_pct, 44.3);
        assert_eq!(e.compactions, 7);
        assert_eq!(e.worst_projects, vec![ProjectWaste { project: "proj".into(), wasted: 310 }]);
    }

    #[test]
    fn last_turn_of_a_session_is_never_rework() {
        let items = vec![turn("a", "2026-10-10T01:00:00.000Z", "do it", 100, false, "p")];
        let e = summarize(&items, 0);
        assert_eq!(e.rework_turns, 0);
        assert_eq!(e.waste_turns, 0);
    }

    #[test]
    fn worst_projects_are_sorted_and_capped() {
        let mut items = Vec::new();
        // Four projects, each with one interrupted turn; the wasted total decides the order.
        for (i, (project, tokens)) in [("d", 10), ("c", 40), ("b", 30), ("a", 20)].iter().enumerate() {
            let ts = format!("2026-10-10T0{i}:00:00.000Z");
            items.push(turn("s", &ts, "x", *tokens, true, project));
        }
        let e = summarize(&items, 0);
        let names: Vec<&str> = e.worst_projects.iter().map(|p| p.project.as_str()).collect();
        assert_eq!(names, vec!["c", "b", "a"]);
    }
}
