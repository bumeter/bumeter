mod common;
use common::*;
use bum_core::config::Config;
use bum_core::handlers::{list_prompts, sidebar_counts, PromptFilter, Quick, Sort};
use bum_core::model::RawEvent;
use bum_core::pricing::Pricing;
use bum_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    // Phiên thứ hai, project khác
    let other = vec![
        RawEvent::Session { session_id: "s2".into(), tool: "claude_code".into(), project: "TICKET-201".into(), cwd: None, version: None, ts: "2026-09-12T01:00:00.000Z".into() },
        RawEvent::Prompt { key: "q1".into(), session_id: "s2".into(), ts: "2026-09-12T01:00:00.000Z".into(), text: "review 100% tài liệu".into() },
        bum_core::model::RawEvent::ApiCall { request_id: "req_Q".into(), session_id: "s2".into(), prompt_key: Some("q1".into()), ts: "2026-09-12T01:00:05.000Z".into(), model: "claude-opus-5".into(), usage: u(1, 0, 500, 9), tool_uses: vec![], sidechain: false },
    ];
    s.ingest_batch(&offset("/root/-other/s2.jsonl"), &other, 0).unwrap();
    s
}

fn keys(list: &bum_core::handlers::PromptList) -> Vec<String> {
    list.groups.iter().flat_map(|g| g.rows.iter().map(|r| r.stats.key.clone())).collect()
}

fn run(s: &Store, f: PromptFilter) -> bum_core::handlers::PromptList {
    list_prompts(s, &Pricing::default_table(), &Config::default(), &f).unwrap()
}

#[test]
fn default_list_is_newest_first_grouped_by_session() {
    let s = loaded();
    let l = run(&s, PromptFilter::default());
    assert_eq!(keys(&l), vec!["q1", "p2", "p1"]);
    assert_eq!(l.groups.len(), 2);
    assert_eq!(l.groups[1].session_id, "s1");
    assert_eq!(l.groups[1].project, "TICKET-101");
    assert_eq!(l.groups[1].total, 3549);
    assert_eq!(l.total_prompts, 3);
    assert!(l.median.is_some());
}

#[test]
fn search_matches_text_and_treats_wildcards_literally() {
    let s = loaded();
    assert_eq!(keys(&run(&s, PromptFilter { q: Some("ntn".into()), ..Default::default() })), vec!["p2"]);
    assert_eq!(keys(&run(&s, PromptFilter { q: Some("100%".into()), ..Default::default() })), vec!["q1"]);
    assert!(keys(&run(&s, PromptFilter { q: Some("%".into()), ..Default::default() })).len() == 1);
}

#[test]
fn filters_by_project_skill_and_time() {
    let s = loaded();
    assert_eq!(keys(&run(&s, PromptFilter { project: Some("TICKET-201".into()), ..Default::default() })), vec!["q1"]);
    assert_eq!(keys(&run(&s, PromptFilter { skill: Some("pj-review".into()), ..Default::default() })), vec!["p2", "p1"]);
    assert_eq!(keys(&run(&s, PromptFilter { from: Some("2026-09-12T00:00:00Z".into()), ..Default::default() })), vec!["q1"]);
}

#[test]
fn quick_filters() {
    let mut s = loaded();
    assert_eq!(keys(&run(&s, PromptFilter { quick: Some(Quick::Interrupted), ..Default::default() })), vec!["p2"]);
    let cfg = Config { context_threshold: 1150, ..Config::default() };
    let l = list_prompts(&s, &Pricing::default_table(), &cfg, &PromptFilter { quick: Some(Quick::BigContext), ..Default::default() }).unwrap();
    assert_eq!(keys(&l), vec!["p2"]);
    s.set_note("p1", true, "", "t").unwrap();
    assert_eq!(keys(&run(&s, PromptFilter { quick: Some(Quick::Starred), ..Default::default() })), vec!["p1"]);
    let rows = run(&s, PromptFilter::default());
    assert!(rows.groups[1].rows[1].starred);
}

#[test]
fn sort_by_tokens_and_paginate() {
    let s = loaded();
    let l = run(&s, PromptFilter { sort: Sort::Tokens, ..Default::default() });
    assert_eq!(keys(&l), vec!["p1", "p2", "q1"]);
    let page = run(&s, PromptFilter { limit: Some(1), offset: Some(1), ..Default::default() });
    assert_eq!(keys(&page), vec!["p2"]);
    assert_eq!(page.total_prompts, 3);
}

#[test]
fn sidebar_counts_cover_all_prompts() {
    let mut s = loaded();
    s.set_note("q1", true, "", "t").unwrap();
    let c = sidebar_counts(&s, &Config::default()).unwrap();
    assert_eq!(c.prompts, 3);
    assert_eq!(c.interrupted, 1);
    assert_eq!(c.starred, 1);
    assert_eq!(c.projects, 2);
    assert_eq!(c.skills, 2); // pj-review, /model
    assert_eq!(c.by_tool.get("claude_code"), Some(&3));
    assert_eq!(c.by_tool.len(), 1);
}
