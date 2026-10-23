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

#[test]
fn effectiveness_and_session_economics_carry_tool_signals() {
    let mut s = Store::open_in_memory().unwrap();
    let mut ev = vec![RawEvent::Session { session_id: "s3".into(), tool: "claude_code".into(), project: "p".into(), cwd: None, version: None, ts: "2026-09-13T01:00:00.000Z".into() }];
    for i in 0..4u64 {
        ev.push(RawEvent::ApiCall {
            request_id: format!("t{i}"), session_id: "s3".into(), prompt_key: None, ts: format!("2026-09-13T01:0{i}:00.000Z"), model: "claude-opus-5".into(),
            usage: Usage { input: 100, ..Default::default() }, sidechain: false, cost_usd: None,
            tool_uses: vec![ToolUse { id: format!("u{i}"), name: "Read".into(), target: Some("/x.rs".into()), target_hash: Some(1) }],
        });
        ev.push(RawEvent::ToolResult { tool_use_id: format!("u{i}"), is_error: false, chars: 350 });
    }
    s.ingest_batch(&offset("/root/-home-u-proj/s3.jsonl"), &ev, 0).unwrap();
    let e = effectiveness(&s, &Config::default(), None, FROM, TO).unwrap();
    let t = e.tools.as_ref().unwrap();
    assert_eq!((t.repeated.len(), t.repeated[0].times, t.calls_with_result), (1, 4, 4));
    assert!(e.advice.iter().any(|a| a.code == "repeated_read" && a.target.as_deref() == Some("/x.rs")));
    let se = session_economics(&s, "s3").unwrap().unwrap();
    assert_eq!(se.tools.unwrap().repeated[0].times, 4);
    // The fixture's session has no recorded results: no tool section.
    assert!(session_economics(&loaded(), "s1").unwrap().unwrap().tools.is_none());
}

/// Two projects, each with one session reading files.
fn two_projects() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    for (sid, project, file, reads) in [("pa", "alpha", "/w/alpha/src/a.rs", 4u64), ("pb", "beta", "/w/beta/b.rs", 1)] {
        let mut ev = vec![RawEvent::Session { session_id: sid.into(), tool: "claude_code".into(), project: project.into(), cwd: None, version: None, ts: "2026-09-13T01:00:00.000Z".into() }];
        for i in 0..reads {
            ev.push(RawEvent::ApiCall {
                request_id: format!("{sid}{i}"), session_id: sid.into(), prompt_key: None, ts: format!("2026-09-13T01:0{i}:00.000Z"), model: "claude-opus-5".into(),
                usage: Usage { input: 100, ..Default::default() }, sidechain: false, cost_usd: None,
                tool_uses: vec![ToolUse { id: format!("{sid}u{i}"), name: "Read".into(), target: Some(file.into()), target_hash: Some(1) }],
            });
        }
        ev.push(RawEvent::ApiCall {
            request_id: format!("{sid}e"), session_id: sid.into(), prompt_key: None, ts: "2026-09-13T02:00:00.000Z".into(), model: "claude-opus-5".into(),
            usage: Usage { input: 50, ..Default::default() }, sidechain: false, cost_usd: None,
            tool_uses: vec![ToolUse { id: format!("{sid}ue"), name: "Edit".into(), target: Some(file.into()), target_hash: Some(2) }],
        });
        s.ingest_batch(&offset(&format!("/root/-w-{project}/{sid}.jsonl")), &ev, 0).unwrap();
    }
    s
}

#[test]
fn effectiveness_can_be_limited_to_one_project() {
    let s = two_projects();
    let all = bum_core::handlers::effectiveness_for(&s, &Config::default(), None, None, FROM, TO).unwrap();
    let alpha = bum_core::handlers::effectiveness_for(&s, &Config::default(), None, Some("alpha"), FROM, TO).unwrap();
    assert_eq!(all.economics.as_ref().unwrap().calls, 7);
    assert_eq!(alpha.economics.as_ref().unwrap().calls, 5);
    assert!(bum_core::handlers::effectiveness_for(&s, &Config::default(), None, Some("nope"), FROM, TO).unwrap().economics.is_none());
}

#[test]
fn file_reads_count_reads_and_edits_per_file_of_a_project() {
    let s = two_projects();
    let r = s.file_reads("alpha", FROM).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].path.as_str(), r[0].reads, r[0].edits), ("/w/alpha/src/a.rs", 4, 1));
    assert_eq!(r[0].last_ts, "2026-09-13T02:00:00.000Z");
    assert!(s.file_reads("alpha", "2026-09-14T00:00:00.000Z").unwrap().is_empty());
}

#[test]
fn project_now_is_the_latest_main_call_of_the_project() {
    let s = two_projects();
    let now = s.project_now("alpha").unwrap().unwrap();
    assert_eq!((now.session_id.as_str(), now.ts.as_str(), now.context), ("pa", "2026-09-13T02:00:00.000Z", 50));
    assert!(s.project_now("nope").unwrap().is_none());
}
