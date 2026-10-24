mod common;
use common::*;
use bum_core::store::Store;

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
    use bum_core::model::RawEvent;
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
    // a timezone offset does not split two prompts on the same day into two days
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

fn priced_call(rid: &str, reasoning: u64, cost_usd: Option<f64>) -> bum_core::model::RawEvent {
    bum_core::model::RawEvent::ApiCall {
        request_id: rid.into(),
        session_id: "s1".into(),
        prompt_key: Some("p1".into()),
        ts: "2026-09-11T03:00:00.000Z".into(),
        model: "opencode:deepseek/x".into(),
        usage: bum_core::model::Usage { input: 10, output: 5, reasoning, ..Default::default() },
        tool_uses: vec![],
        sidechain: false,
        cost_usd,
    }
}

fn reasoning_and_cost(db: &std::path::Path, rid: &str) -> (i64, Option<f64>) {
    rusqlite::Connection::open(db).unwrap().query_row("SELECT reasoning, cost_usd FROM api_calls WHERE request_id = ?1", [rid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

#[test]
fn reasoning_and_cost_usd_keep_the_largest_value_and_cost_stays_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("b.db");
    let mut s = Store::open(&db).unwrap();
    s.ingest_batch(&offset("db:opencode"), &[priced_call("oc:1", 5, Some(0.1))], 0).unwrap();
    s.ingest_batch(&offset("db:opencode"), &[priced_call("oc:1", 7, Some(0.2))], 0).unwrap();
    assert_eq!(reasoning_and_cost(&db, "oc:1"), (7, Some(0.2)));
    s.ingest_batch(&offset("db:opencode"), &[priced_call("oc:1", 3, None)], 0).unwrap();
    assert_eq!(reasoning_and_cost(&db, "oc:1"), (7, Some(0.2)), "a later read without cost keeps it");
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    let rid = match &main_events().into_iter().find(|e| matches!(e, bum_core::model::RawEvent::ApiCall { .. })).unwrap() {
        bum_core::model::RawEvent::ApiCall { request_id, .. } => request_id.clone(),
        _ => unreachable!(),
    };
    assert_eq!(reasoning_and_cost(&db, &rid), (0, None), "Claude calls: no reasoning, no cost");
    let u = s.model_usage_for_prompt("p1").unwrap();
    let oc = u.iter().find(|(m, _)| m == "opencode:deepseek/x").unwrap();
    assert_eq!((oc.1.reasoning, oc.1.total()), (7, 22));
}

#[test]
fn readonly_open_upgrades_a_v6_db_with_the_new_columns() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("b.db");
    drop(Store::open(&db).unwrap());
    rusqlite::Connection::open(&db).unwrap().execute_batch("ALTER TABLE api_calls DROP COLUMN reasoning; ALTER TABLE api_calls DROP COLUMN cost_usd; PRAGMA user_version = 6;").unwrap();
    drop(Store::open_readonly(&db).unwrap());
    let cols: Vec<String> = rusqlite::Connection::open(&db).unwrap().prepare("SELECT name FROM pragma_table_info('api_calls')").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert!(cols.contains(&"reasoning".to_string()) && cols.contains(&"cost_usd".to_string()), "{cols:?}");
}

#[test]
fn v8_adds_tool_columns_and_rereads_claude_code_files_only() {
    use bum_core::model::RawEvent;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v8.db");
    {
        let mut s = Store::open(&db).unwrap();
        s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
        let kiro = vec![
            RawEvent::Session { session_id: "k1".into(), tool: "kiro".into(), project: "p".into(), cwd: None, version: None, ts: "2026-09-11T05:00:00.000Z".into() },
            RawEvent::ApiCall { request_id: "req_K".into(), session_id: "k1".into(), prompt_key: None, ts: "2026-09-11T05:00:01.000Z".into(), model: "kiro:auto".into(), usage: Default::default(), tool_uses: vec![], sidechain: false, cost_usd: None },
        ];
        s.ingest_batch(&offset("/kiro/k1.json"), &kiro, 0).unwrap();
        s.set_note("p1", true, "keep me", "2026-09-12T00:00:00.000Z").unwrap();
    }
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch(
        "DROP INDEX api_call_tools_req; DROP INDEX api_call_tools_file; DROP INDEX prompts_file; DROP INDEX skill_uses_file; DROP INDEX compactions_file;
         ALTER TABLE api_call_tools DROP COLUMN target; ALTER TABLE api_call_tools DROP COLUMN target_hash;
         ALTER TABLE api_call_tools DROP COLUMN error; ALTER TABLE api_call_tools DROP COLUMN result_chars;
         PRAGMA user_version = 7;",
    ).unwrap();
    drop(c);
    drop(Store::open(&db).unwrap());
    // Twice: nothing to do the second time.
    let s = Store::open(&db).unwrap();
    let c = rusqlite::Connection::open(&db).unwrap();
    let cols: Vec<String> = c.prepare("SELECT name FROM pragma_table_info('api_call_tools')").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    for col in ["target", "target_hash", "error", "result_chars"] {
        assert!(cols.contains(&col.to_string()), "{cols:?}");
    }
    let id = |p: &str| c.query_row("SELECT file_id FROM file_offsets WHERE path = ?1", [p], |r| r.get::<_, String>(0)).unwrap();
    assert_eq!(id(MAIN), "reread-v8", "Claude Code files are read again");
    assert_eq!(id("/kiro/k1.json"), "1:1", "other tools are left alone");
    assert_eq!(s.get_note("p1").unwrap().map(|n| n.note), Some("keep me".to_string()));
}

#[test]
fn a_tool_result_marks_its_call_and_one_without_a_call_is_dropped() {
    use bum_core::model::{RawEvent, ToolUse, Usage};
    let mut s = Store::open_in_memory().unwrap();
    let orphan = RawEvent::ToolResult { tool_use_id: "nope".into(), is_error: true, chars: 5 };
    s.ingest_batch(&offset(MAIN), &[orphan], 0).unwrap();
    let ev = vec![
        RawEvent::Session { session_id: "s1".into(), tool: "claude_code".into(), project: "p".into(), cwd: None, version: None, ts: "2026-09-11T04:28:00.000Z".into() },
        RawEvent::ApiCall {
            request_id: "r1".into(), session_id: "s1".into(), prompt_key: None, ts: "2026-09-11T04:28:41.000Z".into(), model: "m".into(),
            usage: Usage::default(), sidechain: false, cost_usd: None,
            tool_uses: vec![ToolUse { id: "t1".into(), name: "Read".into(), target: Some("/a.rs".into()), target_hash: Some(42) }],
        },
        RawEvent::ToolResult { tool_use_id: "t1".into(), is_error: true, chars: 120 },
    ];
    s.ingest_batch(&offset(MAIN), &ev, 0).unwrap();
    let rows = s.tool_calls_between("2026-09-01T00:00:00.000Z", "2026-10-01T00:00:00.000Z", None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].target.as_deref(), rows[0].target_hash, rows[0].error, rows[0].result_chars), (Some("/a.rs"), Some(42), Some(true), Some(120)));
}

#[test]
fn a_session_seen_first_without_cwd_takes_the_project_when_cwd_comes() {
    use bum_core::model::RawEvent;
    let mut s = Store::open_in_memory().unwrap();
    let ses = |project: &str, cwd: Option<&str>| RawEvent::Session { session_id: "v".into(), tool: "claude_code".into(), project: project.into(), cwd: cwd.map(String::from), version: None, ts: "2026-10-24T03:35:17.414Z".into() };
    let first = s.ingest_batch(&offset(MAIN), &[ses("-home-u-work-bumeter", None)], 0).unwrap();
    assert_eq!(first.new_sessions, vec!["v".to_string()]);
    let second = s.ingest_batch(&offset(MAIN), &[ses("bumeter", Some("/home/u/work/bumeter"))], 0).unwrap();
    assert!(second.new_sessions.is_empty(), "not a new session");
    assert_eq!(s.session_project("v").unwrap().as_deref(), Some("bumeter"));
    // Once known, a later line does not change it again.
    s.ingest_batch(&offset(MAIN), &[ses("other", Some("/x/other"))], 0).unwrap();
    assert_eq!(s.session_project("v").unwrap().as_deref(), Some("bumeter"));
}

#[test]
fn v9_rereads_the_files_of_sessions_without_cwd() {
    use bum_core::model::RawEvent;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("v9.db");
    {
        let mut s = Store::open(&db).unwrap();
        s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap(); // has cwd
        let vs = vec![
            RawEvent::Session { session_id: "v".into(), tool: "claude_code".into(), project: "-w-app".into(), cwd: None, version: None, ts: "2026-10-24T03:35:17.414Z".into() },
            RawEvent::Prompt { key: "vq".into(), session_id: "v".into(), ts: "2026-10-24T03:35:19.000Z".into(), text: "hi".into() },
        ];
        s.ingest_batch(&offset("/root/-w-app/v.jsonl"), &vs, 0).unwrap();
    }
    rusqlite::Connection::open(&db).unwrap().execute_batch("PRAGMA user_version = 8;").unwrap();
    drop(Store::open(&db).unwrap());
    let c = rusqlite::Connection::open(&db).unwrap();
    let id = |p: &str| c.query_row("SELECT file_id FROM file_offsets WHERE path = ?1", [p], |r| r.get::<_, String>(0)).unwrap();
    assert_eq!(id("/root/-w-app/v.jsonl"), "reread-v9");
    assert_eq!(id(MAIN), "1:1");
}
