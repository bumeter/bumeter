mod common;
use common::*;
use tk_core::store::Store;

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 1).unwrap();
    s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap();
    s
}

#[test]
fn totals_are_deduplicated_by_request_id() {
    let s = loaded();
    assert_eq!(s.usage_totals().unwrap().total(), 3549);
    assert_eq!(s.api_call_count().unwrap(), 4);
}

#[test]
fn ingesting_the_same_batch_twice_changes_nothing() {
    let mut s = loaded();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    assert_eq!(s.usage_totals().unwrap().total(), 3549);
}

#[test]
fn prompt_stats_include_subagent_calls() {
    let s = loaded();
    let p1 = s.prompt_stats("p1").unwrap().unwrap();
    assert_eq!(p1.usage.total(), 2343);
    assert_eq!(p1.api_calls, 3);
    assert_eq!(p1.context_at_start, 1102);
    assert_eq!(p1.project, "TICKET-101");
    let p2 = s.prompt_stats("p2").unwrap().unwrap();
    assert!(p2.interrupted);
    assert_eq!(p2.usage.total(), 1206);
}

#[test]
fn skill_uses_are_unique_per_prompt() {
    let s = loaded();
    assert_eq!(s.skill_use_count().unwrap(), 2);
}

#[test]
fn summary_reports_new_rate_limits_once() {
    let mut s = Store::open_in_memory().unwrap();
    let first = s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    assert_eq!(first.new_rate_limits, vec![("five_hour".to_string(), 1789902000)]);
    assert_eq!(first.touched_prompts, vec!["p1".to_string(), "p2".to_string()]);
    assert_eq!(first.touched_sessions, vec!["s1".to_string()]);
    let second = s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    assert!(second.new_rate_limits.is_empty());
}

#[test]
fn offsets_and_parse_errors_are_persisted() {
    let s = loaded();
    assert_eq!(s.get_offset(MAIN).unwrap().unwrap().offset, 100);
    assert_eq!(s.parse_errors().unwrap(), 1);
}

#[test]
fn purge_file_removes_only_that_files_rows() {
    let mut s = loaded();
    s.purge_file(MAIN).unwrap();
    assert_eq!(s.usage_totals().unwrap().total(), 60);
    assert!(s.get_offset(MAIN).unwrap().is_none());
    assert!(s.prompt_stats("p1").unwrap().is_none());
}

#[test]
fn readonly_connection_sees_committed_data() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    let mut w = Store::open(&db).unwrap();
    w.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    let r = Store::open_readonly(&db).unwrap();
    assert_eq!(r.usage_totals().unwrap().total(), 2283 + 1206);
}

#[test]
fn session_file_is_the_main_log_not_the_subagent_one() {
    let s = loaded();
    assert_eq!(s.session_file("s1").unwrap().as_deref(), Some(MAIN));
    assert_eq!(s.session_file("nope").unwrap(), None);
}

#[test]
fn session_title_is_stored_and_latest_wins() {
    use tk_core::model::RawEvent;
    let mut s = loaded();
    s.ingest_batch(&offset(MAIN), &[RawEvent::Title { session_id: "s1".into(), title: "A".into() }, RawEvent::Title { session_id: "s1".into(), title: "B".into() }], 0).unwrap();
    assert_eq!(s.session_title("s1").unwrap().as_deref(), Some("B"));
}

#[test]
fn latest_prompt_ts_is_newest_or_none() {
    assert_eq!(Store::open_in_memory().unwrap().latest_prompt_ts().unwrap(), None);
    assert_eq!(loaded().latest_prompt_ts().unwrap().as_deref(), Some("2026-09-11T04:35:00.000Z"));
}

#[test]
fn latest_alert_ts_filters_by_prefix() {
    let mut s = Store::open_in_memory().unwrap();
    assert_eq!(s.latest_alert_ts("anomaly:").unwrap(), None);
    s.alert_once("anomaly:p1", "2026-09-11T05:00:00.000Z").unwrap();
    s.alert_once("anomaly:p2", "2026-09-11T06:00:00.000Z").unwrap();
    s.alert_once("quota:70", "2026-09-11T07:00:00.000Z").unwrap();
    assert_eq!(s.latest_alert_ts("anomaly:").unwrap().as_deref(), Some("2026-09-11T06:00:00.000Z"));
}

#[test]
fn active_days_count_local_days_with_prompts() {
    let s = loaded();
    let (a, b) = ("2026-09-11T00:00:00.000Z", "2026-09-12T00:00:00.000Z");
    assert_eq!(s.active_days_between(a, b, 0).unwrap(), 1);
    assert_eq!(s.active_days_between(b, "2026-09-13T00:00:00.000Z", 0).unwrap(), 0);
    // lệch múi giờ không làm hai lần hỏi cùng ngày tách thành hai ngày
    assert_eq!(s.active_days_between(a, b, 7 * 3600).unwrap(), 1);
}

#[test]
fn latest_anomaly_ts_filters_by_tool() {
    let mut s = loaded();
    assert_eq!(s.latest_anomaly_ts(None).unwrap(), None);
    s.alert_once("anomaly:p1", "2026-09-11T05:00:00.000Z").unwrap();
    s.alert_once("quota:70", "2026-09-11T07:00:00.000Z").unwrap();
    assert_eq!(s.latest_anomaly_ts(None).unwrap().as_deref(), Some("2026-09-11T05:00:00.000Z"));
    assert_eq!(s.latest_anomaly_ts(Some("claude_code")).unwrap().as_deref(), Some("2026-09-11T05:00:00.000Z"));
    assert_eq!(s.latest_anomaly_ts(Some("kiro")).unwrap(), None);
}
