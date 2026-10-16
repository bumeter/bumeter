mod common;
use common::*;
use bum_core::config::Config;
use bum_core::handlers::prompt_detail;
use bum_core::pricing::Pricing;
use bum_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    s
}

#[test]
fn detail_lists_every_call_in_order_with_tools() {
    let s = loaded();
    let d = prompt_detail(&s, &Pricing::default_table(), &Config::default(), "p1").unwrap().unwrap();
    let calls: Vec<_> = d.calls.iter().map(|c| (c.request_id.as_str(), c.sidechain, c.tools.clone())).collect();
    assert_eq!(calls, vec![
        ("req_A", false, vec!["Skill".to_string()]),
        ("req_B", false, vec!["Bash".to_string()]),
        ("req_S", true, vec![]),
    ]);
    assert_eq!(d.calls[0].context, 1102);
    assert_eq!(d.calls[0].output, 50);
    assert_eq!(d.row.stats.usage.total(), 2343);
    assert_eq!(d.row.skills[0].skill, "pj-review");
    assert!(d.row.cost_usd.unwrap() > 0.0);
}

#[test]
fn detail_explains_where_tokens_went() {
    let s = loaded();
    let d = prompt_detail(&s, &Pricing::default_table(), &Config::default(), "p1").unwrap().unwrap();
    let expected = 2100.0 / 2343.0;
    assert!((d.reread_share - expected).abs() < 1e-9, "{}", d.reread_share);
    assert_ne!(d.explanation, bum_core::insight::Explain::None);
    assert_eq!(d.duration_secs, Some(56)); // 04:28:23.938 → 04:29:20
    assert!(d.note.is_none());
}

#[test]
fn detail_includes_note_and_missing_prompt_is_none() {
    let mut s = loaded();
    s.set_note("p2", true, "ask again", "t").unwrap();
    let d = prompt_detail(&s, &Pricing::default_table(), &Config::default(), "p2").unwrap().unwrap();
    assert_eq!(d.note.unwrap().note, "ask again");
    assert!(d.row.starred);
    assert!(prompt_detail(&s, &Pricing::default_table(), &Config::default(), "nope").unwrap().is_none());
}
