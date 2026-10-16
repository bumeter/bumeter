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
    assert_eq!(errors, 1, "dòng JSON hỏng phải bị đếm lỗi");

    let prompts: Vec<_> = ev.iter().filter_map(|e| match e {
        RawEvent::Prompt { key, text, .. } => Some((key.as_str(), text.as_str())),
        _ => None,
    }).collect();
    assert_eq!(prompts, vec![("p1", "giờ bạn cập nhật tiến độ nhé"), ("p2", "tức là ntn ?")]);

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
    assert_eq!(usage_a, Usage { input: 2, cache_write_5m: 0, cache_write_1h: 100, cache_read: 1000, output: 50 });
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
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Title { session_id, title } if session_id == "s1" && title == "Cập nhật tiến độ <task>")));
}
