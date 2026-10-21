use std::path::{Path, PathBuf};

use bum_adapters::claude_code::ClaudeCode;
use bum_core::model::{RawEvent, SkillKind, Usage};
use bum_core::source::{LogSource, ParseCtx};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code")
}

fn parse_file(src: &ClaudeCode, rel: &str) -> (Vec<RawEvent>, usize) {
    let path = root().join(rel);
    let mut ctx = ParseCtx::default();
    let mut events = Vec::new();
    let mut errors = 0;
    for line in std::fs::read_to_string(&path).unwrap().lines() {
        match src.parse_line(&path, &mut ctx, line) {
            Ok(mut ev) => events.append(&mut ev),
            Err(_) => errors += 1,
        }
    }
    (events, errors)
}

#[test]
fn main_file_yields_expected_events() {
    let src = ClaudeCode::new(root());
    let (ev, errors) = parse_file(&src, "-home-u-proj/s1.jsonl");
    assert_eq!(errors, 1, "a broken JSON line must be counted as an error");

    let prompts: Vec<_> = ev.iter().filter_map(|e| match e {
        RawEvent::Prompt { key, text, .. } => Some((key.as_str(), text.as_str())),
        _ => None,
    }).collect();
    assert_eq!(prompts, vec![("p1", "please update the progress now"), ("p2", "what do you mean ?")]);

    let calls: Vec<_> = ev.iter().filter_map(|e| match e {
        RawEvent::ApiCall { request_id, prompt_key, .. } => Some((request_id.as_str(), prompt_key.as_deref())),
        _ => None,
    }).collect();
    assert_eq!(calls, vec![("req_A", Some("p1")), ("req_A", Some("p1")), ("req_B", Some("p1")), ("req_C", Some("p2"))]);

    let skills: Vec<_> = ev.iter().filter_map(|e| match e {
        RawEvent::Skill { skill, kind, prompt_key, .. } => Some((skill.as_str(), *kind, prompt_key.as_deref())),
        _ => None,
    }).collect();
    assert!(skills.contains(&("pj-review", SkillKind::Direct, Some("p1"))));
    assert!(skills.contains(&("/model", SkillKind::Command, Some("p3"))));

    assert!(ev.iter().any(|e| matches!(e, RawEvent::Interrupted { prompt_key: Some(k), .. } if k == "p2")));
    assert_eq!(ev.iter().filter(|e| matches!(e, RawEvent::Compacted { .. })).count(), 1);
    assert!(ev.iter().any(|e| matches!(e, RawEvent::RateLimited { limit_type, resets_at: 1789902000, .. } if limit_type == "five_hour")));

    let sessions: Vec<_> = ev.iter().filter_map(|e| match e {
        RawEvent::Session { project, tool, .. } => Some((project.as_str(), tool.as_str())),
        _ => None,
    }).collect();
    assert_eq!(sessions, vec![("TICKET-101", "claude_code")]);
}

#[test]
fn cache_creation_is_split_by_ttl() {
    let src = ClaudeCode::new(root());
    let (ev, _) = parse_file(&src, "-home-u-proj/s1.jsonl");
    let usage_a = ev.iter().find_map(|e| match e {
        RawEvent::ApiCall { request_id, usage, tool_uses, .. } if request_id == "req_A" && tool_uses.is_empty() => Some(usage.clone()),
        _ => None,
    }).unwrap();
    assert_eq!(usage_a, Usage { input: 2, cache_write_5m: 0, cache_write_1h: 100, cache_read: 1000, output: 50, reasoning: 0 });
}

#[test]
fn subagent_calls_belong_to_parent_prompt_and_are_not_prompts() {
    let src = ClaudeCode::new(root());
    let (ev, errors) = parse_file(&src, "-home-u-proj/s1/subagents/agent-x.jsonl");
    assert_eq!(errors, 0);
    assert!(!ev.iter().any(|e| matches!(e, RawEvent::Prompt { .. })));
    assert!(ev.iter().any(|e| matches!(e,
        RawEvent::ApiCall { request_id, prompt_key: Some(k), sidechain: true, .. } if request_id == "req_S" && k == "p1")));
}

#[test]
fn accepts_only_jsonl_under_root() {
    let src = ClaudeCode::new(root());
    assert!(src.accepts(&root().join("-home-u-proj/s1.jsonl")));
    assert!(!src.accepts(&root().join("-home-u-proj/memory/MEMORY.md")));
    assert!(!src.accepts(Path::new("/tmp/other.jsonl")));
}

#[test]
fn ai_title_becomes_the_conversation_title() {
    let src = ClaudeCode::new(root());
    let (ev, _) = parse_file(&src, "-home-u-proj/s1.jsonl");
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Title { session_id, title } if session_id == "s1" && title == "Update progress <task>")));
}

fn parse_lines(lines: &[String]) -> Vec<RawEvent> {
    let src = ClaudeCode::new(root());
    let path = root().join("-home-u-proj/inline.jsonl");
    let mut ctx = ParseCtx::default();
    lines.iter().flat_map(|l| src.parse_line(&path, &mut ctx, l).unwrap()).collect()
}

fn assistant_with_tools(tools: serde_json::Value) -> String {
    serde_json::json!({
        "type": "assistant", "sessionId": "s9", "timestamp": "2026-10-10T01:00:00.000Z", "requestId": "req_T",
        "message": { "id": "m1", "model": "claude-opus-5", "content": tools, "usage": { "input_tokens": 1, "output_tokens": 1 } }
    }).to_string()
}

fn tools_of(ev: &[RawEvent]) -> Vec<bum_core::model::ToolUse> {
    ev.iter().find_map(|e| match e { RawEvent::ApiCall { tool_uses, .. } => Some(tool_uses.clone()), _ => None }).unwrap()
}

#[test]
fn tool_targets_are_short_and_never_a_whole_command() {
    let line = assistant_with_tools(serde_json::json!([
        { "type": "tool_use", "id": "t1", "name": "Read", "input": { "file_path": "C:\\src\\a.rs" } },
        { "type": "tool_use", "id": "t2", "name": "Bash", "input": { "command": "API_TOKEN=s3cret cargo test -p bum-core --all" } },
        { "type": "tool_use", "id": "t3", "name": "Grep", "input": { "pattern": "fn main" } },
        { "type": "tool_use", "id": "t4", "name": "mcp__x__y", "input": { "q": 1 } },
    ]));
    let tools = tools_of(&parse_lines(&[line]));
    let targets: Vec<Option<&str>> = tools.iter().map(|t| t.target.as_deref()).collect();
    assert_eq!(targets, vec![Some("C:/src/a.rs"), Some("cargo test"), Some("fn main"), None]);
    assert!(tools.iter().all(|t| t.target_hash.is_some()));
}

#[test]
fn the_input_hash_ignores_key_order_but_not_values() {
    let a = assistant_with_tools(serde_json::json!([{ "type": "tool_use", "id": "t1", "name": "Bash", "input": { "command": "ls", "timeout": 5 } }]));
    let b = r#"{"type":"assistant","sessionId":"s9","timestamp":"2026-10-10T01:00:00.000Z","requestId":"req_T","message":{"id":"m1","model":"claude-opus-5","usage":{"input_tokens":1,"output_tokens":1},"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"timeout":5,"command":"ls"}}]}}"#.to_string();
    let c = assistant_with_tools(serde_json::json!([{ "type": "tool_use", "id": "t1", "name": "Bash", "input": { "command": "ls -la", "timeout": 5 } }]));
    let h = |l: String| tools_of(&parse_lines(&[l]))[0].target_hash;
    assert_eq!(h(a.clone()), h(b));
    assert_ne!(h(a), h(c));
}

#[test]
fn tool_results_say_whether_the_call_failed_and_how_long_the_result_was() {
    let line = serde_json::json!({
        "type": "user", "sessionId": "s9", "timestamp": "2026-10-10T01:00:05.000Z", "promptId": "p1",
        "message": { "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": "t1", "content": "hello" },
            { "type": "tool_result", "tool_use_id": "t2", "is_error": true, "content": [{ "type": "text", "text": "boom!" }, { "type": "text", "text": "ab" }] },
        ] }
    }).to_string();
    let ev = parse_lines(&[line]);
    let results: Vec<(&str, bool, u64)> = ev.iter().filter_map(|e| match e {
        RawEvent::ToolResult { tool_use_id, is_error, chars } => Some((tool_use_id.as_str(), *is_error, *chars)),
        _ => None,
    }).collect();
    assert_eq!(results, vec![("t1", false, 5), ("t2", true, 7)]);
    assert!(!ev.iter().any(|e| matches!(e, RawEvent::Prompt { .. })), "a tool result is not a question");
}
