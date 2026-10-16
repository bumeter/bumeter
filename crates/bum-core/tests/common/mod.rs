#![allow(dead_code)]
use bum_core::model::{RawEvent, SkillKind, ToolUse, Usage};
use bum_core::store::FileOffset;

pub const MAIN: &str = "/root/-home-u-proj/s1.jsonl";
pub const SUB: &str = "/root/-home-u-proj/s1/subagents/agent-x.jsonl";

pub fn u(input: u64, cw1h: u64, read: u64, output: u64) -> Usage {
    Usage { input, cache_write_5m: 0, cache_write_1h: cw1h, cache_read: read, output }
}

pub fn call(rid: &str, pk: &str, ts: &str, model: &str, usage: Usage, tools: &[(&str, &str)], sidechain: bool) -> RawEvent {
    RawEvent::ApiCall {
        request_id: rid.into(),
        session_id: "s1".into(),
        prompt_key: Some(pk.into()),
        ts: ts.into(),
        model: model.into(),
        usage,
        tool_uses: tools.iter().map(|(id, n)| ToolUse { id: (*id).into(), name: (*n).into() }).collect(),
        sidechain,
    }
}

/// Equivalent to `tests/fixtures/claude_code/-home-u-proj/s1.jsonl` after parsing.
pub fn main_events() -> Vec<RawEvent> {
    vec![
        RawEvent::Session { session_id: "s1".into(), tool: "claude_code".into(), project: "TICKET-101".into(), cwd: Some("/home/u/proj/TICKET-101".into()), version: Some("2.1.247".into()), ts: "2026-09-11T04:28:23.938Z".into() },
        RawEvent::Prompt { key: "p1".into(), session_id: "s1".into(), ts: "2026-09-11T04:28:23.938Z".into(), text: "update the progress now, ok?".into() },
        call("req_A", "p1", "2026-09-11T04:28:41.000Z", "claude-opus-5", u(2, 100, 1000, 50), &[], false),
        RawEvent::Skill { prompt_key: Some("p1".into()), session_id: "s1".into(), ts: "2026-09-11T04:28:42.000Z".into(), skill: "pj-review".into(), kind: SkillKind::Direct },
        RawEvent::Skill { prompt_key: Some("p1".into()), session_id: "s1".into(), ts: "2026-09-11T04:28:42.000Z".into(), skill: "pj-review".into(), kind: SkillKind::Direct },
        call("req_A", "p1", "2026-09-11T04:28:42.000Z", "claude-opus-5", u(2, 100, 1000, 50), &[("tu_1", "Skill")], false),
        call("req_B", "p1", "2026-09-11T04:29:01.000Z", "claude-opus-5", u(1, 10, 1100, 20), &[("tu_2", "Bash")], false),
        RawEvent::Prompt { key: "p2".into(), session_id: "s1".into(), ts: "2026-09-11T04:35:00.000Z".into(), text: "what does that mean?".into() },
        call("req_C", "p2", "2026-09-11T04:35:05.000Z", "claude-opus-5", u(1, 0, 1200, 5), &[], false),
        RawEvent::Interrupted { prompt_key: Some("p2".into()), session_id: "s1".into(), ts: "2026-09-11T04:35:10.000Z".into() },
        RawEvent::Skill { prompt_key: Some("p3".into()), session_id: "s1".into(), ts: "2026-09-11T04:36:00.000Z".into(), skill: "/model".into(), kind: SkillKind::Command },
        RawEvent::Compacted { session_id: "s1".into(), ts: "2026-09-11T04:40:00.000Z".into() },
        RawEvent::RateLimited { ts: "2026-09-11T04:41:00.000Z".into(), limit_type: "five_hour".into(), resets_at: 1789902000 },
    ]
}

pub fn sub_events() -> Vec<RawEvent> {
    vec![call("req_S", "p1", "2026-09-11T04:29:20.000Z", "claude-sonnet-5", u(3, 50, 0, 7), &[], true)]
}

pub fn offset(path: &str) -> FileOffset {
    FileOffset { path: path.into(), file_id: "1:1".into(), size: 100, offset: 100, mtime: 0, ctx_json: "{}".into() }
}
