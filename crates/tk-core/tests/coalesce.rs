use std::time::{Duration, Instant};

use tk_core::coalesce::Coalescer;

#[test]
fn first_change_goes_out_immediately_then_bursts_are_merged() {
    let t0 = Instant::now();
    let mut c = Coalescer::new(Duration::from_millis(250));
    c.add(&["p1".into()], &["s1".into()]);
    let first = c.take(t0).unwrap();
    assert_eq!(first.prompt_keys, vec!["p1"]);

    c.add(&["p2".into()], &["s1".into()]);
    c.add(&["p3".into(), "p2".into()], &["s1".into()]);
    assert!(c.take(t0 + Duration::from_millis(100)).is_none(), "chưa đủ 250ms");
    let merged = c.take(t0 + Duration::from_millis(260)).unwrap();
    assert_eq!(merged.prompt_keys, vec!["p2", "p3"]);
    assert_eq!(merged.session_ids, vec!["s1"]);
    assert!(c.take(t0 + Duration::from_secs(5)).is_none(), "không có gì mới");
}
