mod common;
use common::*;
use bum_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    s
}

const FROM: &str = "2026-09-01T00:00:00.000Z";
const TO: &str = "2026-10-01T00:00:00.000Z";

#[test]
fn calls_in_a_range_come_per_session_in_time_order() {
    let s = loaded();
    let calls = s.session_calls_between(FROM, TO, None).unwrap();
    let ids: Vec<&str> = calls.iter().map(|c| c.request_id.as_str()).collect();
    assert_eq!(ids, vec!["req_A", "req_B", "req_S", "req_C"]);
    assert!(calls[2].sidechain);
    assert!(!calls[0].sidechain);
    assert_eq!(calls[0].usage.cache_read, 1000);
    assert_eq!(calls[0].model, "claude-opus-5");
}

#[test]
fn calls_are_limited_to_the_range_and_the_tool() {
    let s = loaded();
    assert!(s.session_calls_between(FROM, TO, Some("kiro")).unwrap().is_empty());
    assert_eq!(s.session_calls_between(FROM, TO, Some("claude_code")).unwrap().len(), 4);
    // `to` is exclusive.
    assert_eq!(s.session_calls_between(FROM, "2026-09-11T04:29:01.000Z", None).unwrap().len(), 1);
}

#[test]
fn the_first_main_call_of_a_session_ignores_subagents() {
    let s = loaded();
    let first = s.first_main_calls(&["s1".to_string(), "nope".to_string()]).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first["s1"].request_id, "req_A");
}

#[test]
fn rate_limit_events_in_a_range() {
    let s = loaded();
    let ev = s.rate_limit_events_between(FROM, TO).unwrap();
    assert_eq!(ev, vec![("five_hour".to_string(), 1789902000, "2026-09-11T04:41:00.000Z".to_string())]);
    assert!(s.rate_limit_events_between("2026-09-12T00:00:00.000Z", TO).unwrap().is_empty());
}

use bum_core::config::Config;
use bum_core::handlers::{effectiveness, session_economics};
use bum_core::model::{RawEvent, ToolUse, Usage};

/// A second session `s2` with 60 growing main-chain calls (sends its context again ~30× over).
fn long_session() -> Vec<RawEvent> {
    let mut ev = vec![
        RawEvent::Session { session_id: "s2".into(), tool: "claude_code".into(), project: "proj".into(), cwd: None, version: None, ts: "2026-09-12T01:00:00.000Z".into() },
        RawEvent::Prompt { key: "q1".into(), session_id: "s2".into(), ts: "2026-09-12T01:00:00.000Z".into(), text: "go".into() },
    ];
    for i in 0..60u64 {
        ev.push(RawEvent::ApiCall {
            request_id: format!("long_{i}"),
            session_id: "s2".into(),
            prompt_key: Some("q1".into()),
            ts: format!("2026-09-12T01:{:02}:00.000Z", i),
            model: "claude-opus-5".into(),
            usage: Usage { cache_read: 1_000 + i * 10, cache_write_5m: 10, ..Default::default() },
            tool_uses: Vec::<ToolUse>::new(),
            sidechain: false,
            cost_usd: None,
        });
    }
    ev
}

#[test]
fn effectiveness_adds_economics_and_puts_a_rate_limit_hit_first() {
    let mut s = loaded();
    s.ingest_batch(&offset("/root/-home-u-proj/s2.jsonl"), &long_session(), 0).unwrap();
    let e = effectiveness(&s, &Config::default(), None, FROM, TO).unwrap();
    let ec = e.economics.as_ref().unwrap();
    assert_eq!(ec.startup.sessions, 2);
    assert_eq!(ec.resend.sessions, 2);
    assert_eq!(ec.resend.flagged, 0);
    assert_eq!(ec.resend.worst.as_ref().unwrap().0, "s2");
    assert_eq!(e.blocked.as_ref().unwrap().count, 1);
    assert_eq!(e.advice[0].code, "blocked");
    // A tool without 5h/7d limits has no rate-limit section.
    assert!(effectiveness(&s, &Config::default(), Some("kiro"), FROM, TO).unwrap().blocked.is_none());
}

#[test]
fn session_economics_reads_one_session_from_the_database() {
    let s = loaded();
    let e = session_economics(&s, "s1").unwrap().unwrap();
    assert_eq!(e.calls, 3);
    assert_eq!(e.startup_ctx, 2 + 100 + 1000);
    assert_eq!(e.max_ctx, 1 + 1200);
    assert!(e.cold.is_empty());
    let chain: Vec<&str> = e.by_chain.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(chain, vec!["main", "subagent"]);
    assert!(session_economics(&s, "nope").unwrap().is_none());
}
