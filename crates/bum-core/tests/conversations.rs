mod common;
use common::*;
use bum_core::config::Config;
use bum_core::handlers::{conversation_turns, list_conversations, ConversationFilter};
use bum_core::model::RawEvent;
use bum_core::pricing::Pricing;
use bum_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    s.ingest_batch(&offset(MAIN), &[RawEvent::Title { session_id: "s1".into(), title: "Cập nhật tiến độ".into() }], 0).unwrap();
    let kiro = vec![
        RawEvent::Session { session_id: "k1".into(), tool: "kiro".into(), project: "TICKET-203".into(), cwd: None, version: None, ts: "2026-09-12T20:00:00.000Z".into() },
        RawEvent::Prompt { key: "kp1".into(), session_id: "k1".into(), ts: "2026-09-12T20:00:00.000Z".into(), text: "đọc dự án voice".into() },
        RawEvent::ApiCall { request_id: "kiro:k1:0".into(), session_id: "k1".into(), prompt_key: Some("kp1".into()), ts: "2026-09-12T20:00:07.000Z".into(), model: "kiro:auto".into(), usage: u(125_000, 0, 0, 0), tool_uses: vec![], sidechain: false },
        RawEvent::Metered { request_id: "kiro:k1:0".into(), credits: 0.4 },
    ];
    s.ingest_batch(&offset("/kiro/k1.json"), &kiro, 0).unwrap();
    s
}

fn run(s: &Store, f: ConversationFilter, offset_secs: i32) -> bum_core::handlers::ConversationList {
    list_conversations(s, &Pricing::default_table(), &Config::default(), &f, offset_secs).unwrap()
}

#[test]
fn conversations_are_grouped_by_local_day_newest_first() {
    let s = loaded();
    let l = run(&s, ConversationFilter::default(), 7 * 3600);
    let days: Vec<_> = l.days.iter().map(|d| (d.day.clone(), d.conversations.iter().map(|c| c.session_id.clone()).collect::<Vec<_>>())).collect();
    assert_eq!(days, vec![("2026-09-13".to_string(), vec!["k1".to_string()]), ("2026-09-11".to_string(), vec!["s1".to_string()])]);
    let c = &l.days[1].conversations[0];
    assert_eq!(c.title, "Cập nhật tiến độ");
    assert_eq!((c.tool.as_str(), c.project.as_str(), c.turns, c.tokens), ("claude_code", "TICKET-101", 2, 3549));
    let mut models = c.models.clone();
    models.sort();
    assert_eq!(models, vec!["claude-opus-5", "claude-sonnet-5"]);
    assert_eq!(c.interrupted, 1);
    let k = &l.days[0].conversations[0];
    assert_eq!(k.title, "đọc dự án voice", "không có tiêu đề thì dùng câu hỏi đầu tiên");
    assert!((k.credits - 0.4).abs() < 1e-9);
    assert_eq!(l.total_conversations, 2);
}

#[test]
fn filters_by_project_models_tool_and_search() {
    let s = loaded();
    let ids = |f: ConversationFilter| run(&s, f, 0).days.iter().flat_map(|d| d.conversations.iter().map(|c| c.session_id.clone())).collect::<Vec<_>>();
    assert_eq!(ids(ConversationFilter { project: Some("TICKET-203".into()), ..Default::default() }), vec!["k1"]);
    assert_eq!(ids(ConversationFilter { models: vec!["claude-sonnet-5".into(), "nope".into()], ..Default::default() }), vec!["s1"]);
    assert_eq!(ids(ConversationFilter { tool: Some("kiro".into()), ..Default::default() }), vec!["k1"]);
    assert_eq!(ids(ConversationFilter { q: Some("ntn".into()), ..Default::default() }), vec!["s1"]);
    let found = run(&s, ConversationFilter { q: Some("ntn".into()), ..Default::default() }, 0);
    assert_eq!(found.days[0].conversations[0].turns, 2, "số lượt là của cả hội thoại, không chỉ lượt khớp");
}

#[test]
fn turns_of_a_conversation_are_in_order() {
    let s = loaded();
    let c = conversation_turns(&s, &Pricing::default_table(), &Config::default(), "s1").unwrap();
    assert_eq!(c.title, "Cập nhật tiến độ");
    let t = c.turns;
    assert_eq!(t.iter().map(|r| r.stats.key.as_str()).collect::<Vec<_>>(), vec!["p1", "p2"]);
    assert_eq!(t[1].skills[0].skill, "pj-review");
}

#[test]
fn project_and_day_stats_follow_the_tool_scope() {
    let s = loaded();
    let p = bum_core::handlers::project_stats_for(&s, &Pricing::default_table(), Some("kiro")).unwrap();
    assert_eq!(p.iter().map(|x| x.project.as_str()).collect::<Vec<_>>(), vec!["TICKET-203"]);
    assert_eq!(p[0].tokens, 125_000);
    let d = bum_core::handlers::day_stats_for(&s, &Pricing::default_table(), 0, Some("claude_code")).unwrap();
    assert_eq!(d.iter().map(|x| (x.day.as_str(), x.prompts, x.tokens)).collect::<Vec<_>>(), vec![("2026-09-11", 2, 3549)]);
}
