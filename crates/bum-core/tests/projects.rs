mod common;
use common::*;
use bum_core::handlers::{day_stats, project_stats};
use bum_core::model::RawEvent;
use bum_core::pricing::Pricing;
use bum_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    let late = vec![
        RawEvent::Session { session_id: "s2".into(), tool: "claude_code".into(), project: "<b>PROJECT-X</b>".into(), cwd: None, version: None, ts: "2026-09-11T20:00:00.000Z".into() },
        RawEvent::Prompt { key: "q1".into(), session_id: "s2".into(), ts: "2026-09-11T20:00:00.000Z".into(), text: "tối muộn".into() },
        RawEvent::ApiCall { request_id: "req_Q".into(), session_id: "s2".into(), prompt_key: Some("q1".into()), ts: "2026-09-11T20:00:05.000Z".into(), model: "claude-opus-5".into(), usage: u(1, 0, 99, 0), tool_uses: vec![], sidechain: false },
    ];
    s.ingest_batch(&offset("/root/-o/s2.jsonl"), &late, 0).unwrap();
    s
}

#[test]
fn projects_are_ranked_by_tokens_with_top_skills_and_priciest_prompt() {
    let s = loaded();
    let p = project_stats(&s, &Pricing::default_table()).unwrap();
    assert_eq!(p.len(), 2);
    let g = &p[0];
    assert_eq!(g.project, "TICKET-101");
    assert_eq!((g.sessions, g.prompts, g.tokens), (1, 2, 3549));
    assert_eq!(g.top_skills, vec!["pj-review".to_string()]);
    assert_eq!(g.priciest.as_ref().unwrap().key, "p1");
    assert_eq!(g.priciest.as_ref().unwrap().total, 2343);
    assert!(g.cost_usd > 0.0);
    assert_eq!(g.last_used, "2026-09-11T04:35:00.000Z");
    assert_eq!(p[1].tokens, 100);
}

#[test]
fn days_follow_the_given_utc_offset() {
    let s = loaded();
    let utc = day_stats(&s, &Pricing::default_table(), 0).unwrap();
    assert_eq!(utc.iter().map(|d| (d.day.as_str(), d.tokens, d.prompts)).collect::<Vec<_>>(), vec![("2026-09-11", 3649, 3)]);
    let vn = day_stats(&s, &Pricing::default_table(), 7 * 3600).unwrap();
    assert_eq!(vn.iter().map(|d| (d.day.as_str(), d.tokens, d.prompts)).collect::<Vec<_>>(), vec![("2026-09-11", 3549, 2), ("2026-09-12", 100, 1)]);
    assert!(vn[0].from < vn[0].to);
}
