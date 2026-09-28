mod common;
use common::*;
use tk_core::attribution::How;
use tk_core::handlers::{recent_prompts, today_summary};
use tk_core::pricing::Pricing;
use tk_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    s
}

#[test]
fn today_summary_sums_calls_in_window_and_prices_them() {
    let s = loaded();
    let t = today_summary(&s, &Pricing::default_table(), "2026-09-11T00:00:00Z", "2026-09-12T00:00:00Z").unwrap();
    assert_eq!(t.prompts, 2);
    assert_eq!(t.total, 3549);
    assert!(t.cost_usd > 0.0);
    assert!(t.unpriced_models.is_empty());
    let empty = today_summary(&s, &Pricing::default_table(), "2026-09-12T00:00:00Z", "2026-09-13T00:00:00Z").unwrap();
    assert_eq!(empty.total, 0);
}

#[test]
fn recent_prompts_are_newest_first_with_skills() {
    let s = loaded();
    let rows = recent_prompts(&s, &Pricing::default_table(), 10).unwrap();
    assert_eq!(rows.iter().map(|r| r.stats.key.as_str()).collect::<Vec<_>>(), vec!["p2", "p1"]);
    assert_eq!(rows[0].skills[0].how, How::Continued);
    assert_eq!(rows[1].skills[0].how, How::Direct);
    assert_eq!(rows[1].stats.usage.total(), 2343);
    assert!(rows[1].cost_usd.is_some());
}
