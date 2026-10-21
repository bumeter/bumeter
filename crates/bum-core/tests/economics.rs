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
