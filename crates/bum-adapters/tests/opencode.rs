//! OpenCode adapter against a small fixture DB shaped like OpenCode v2 (`session_v2` + `session_message`).
use bum_adapters::opencode::OpenCode;
use bum_core::model::{RawEvent, ToolUse, Usage};
use bum_core::source::DbSource;
use rusqlite::{params, Connection};

const T0: i64 = 1_792_200_000_000; // ms

fn user(text: &str) -> String {
    serde_json::json!({ "time": { "created": 0 }, "text": text, "files": [], "agents": [] }).to_string()
}

fn assistant(input: u64, output: u64, reasoning: u64, read: u64, write: u64, cost: f64, tools: &[(&str, &str)]) -> String {
    let mut content = vec![serde_json::json!({ "type": "reasoning", "text": "…" })];
    content.extend(tools.iter().map(|(id, name)| serde_json::json!({ "type": "tool", "id": id, "name": name, "state": {} })));
    serde_json::json!({
        "time": { "created": 0, "completed": 1 }, "agent": "build",
        "model": { "id": "deepseek-v4.1-flash", "providerID": "deepseek", "variant": "default" },
        "content": content, "finish": "stop", "cost": cost,
        "tokens": { "input": input, "output": output, "reasoning": reasoning, "cache": { "read": read, "write": write } }
    })
    .to_string()
}

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("opencode.db");
    let c = Connection::open(&db).unwrap();
    c.execute_batch(
        "CREATE TABLE session_v2(id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT, directory TEXT, title TEXT, version TEXT,
           time_created INTEGER, time_updated INTEGER);
         CREATE TABLE session_message(id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER, time_created INTEGER,
           time_updated INTEGER, data TEXT);",
    )
    .unwrap();
    let s = |id: &str, parent: Option<&str>, dir: &str, title: &str, t: i64| {
        c.execute("INSERT INTO session_v2 VALUES(?1, 'p', ?2, ?3, ?4, '2.0.27', ?5, ?5)", params![id, parent, dir, title, t]).unwrap();
    };
    s("ses_a", None, "/home/u/project/bumeter", "Add forecast", T0);
    // Subagent spawned during the first turn (after msg_u1, before msg_u2).
    s("ses_sub", Some("ses_a"), "/home/u/project/bumeter", "Explore the code", T0 + 22_000);
    let m = |id: &str, sid: &str, ty: &str, seq: i64, t: i64, data: String| {
        c.execute("INSERT INTO session_message VALUES(?1, ?2, ?3, ?4, ?5, ?5, ?6)", params![id, sid, ty, seq, t, data]).unwrap();
    };
    m("msg_u1", "ses_a", "user", 1, T0 + 10_000, user("read the project"));
    m("msg_syn", "ses_a", "synthetic", 2, T0 + 15_000, serde_json::json!({ "text": "<system-reminder>opened a file</system-reminder>", "time": {} }).to_string());
    m("msg_a1", "ses_a", "assistant", 3, T0 + 20_000, assistant(100, 20, 30, 5_000, 400, 0.01, &[("call-1", "read")]));
    m("msg_sys", "ses_a", "system", 4, T0 + 25_000, serde_json::json!({ "text": "tools changed", "time": {} }).to_string());
    m("msg_a2", "ses_a", "assistant", 5, T0 + 30_000, assistant(10, 5, 0, 6_000, 0, 0.002, &[]));
    m("msg_idle", "ses_a", "idle", 6, T0 + 35_000, serde_json::json!({ "outcome": "done", "time": {} }).to_string());
    m("msg_cmp", "ses_a", "compaction", 7, T0 + 40_000, serde_json::json!({ "status": "completed", "reason": "auto", "summary": "…", "time": { "created": T0 + 40_000 } }).to_string());
    m("msg_u2", "ses_a", "user", 8, T0 + 50_000, user("now add tests"));
    m("msg_su", "ses_sub", "user", 1, T0 + 23_000, user("find where limits are computed"));
    m("msg_s1", "ses_sub", "assistant", 2, T0 + 55_000, assistant(7, 3, 0, 0, 0, 0.0, &[]));
    (dir, db)
}

/// (request_id, prompt_key, model, usage, tool_uses, sidechain, cost_usd) of each ApiCall.
type Call = (String, Option<String>, String, Usage, Vec<ToolUse>, bool, Option<f64>);

fn calls(ev: &[RawEvent]) -> Vec<Call> {
    ev.iter()
        .filter_map(|e| match e {
            RawEvent::ApiCall { request_id, prompt_key, model, usage, tool_uses, sidechain, cost_usd, .. } => {
                Some((request_id.clone(), prompt_key.clone(), model.clone(), usage.clone(), tool_uses.clone(), *sidechain, *cost_usd))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn maps_sessions_turns_calls_and_compactions() {
    let (_d, db) = fixture();
    let (ev, wm) = OpenCode::new(&db).scan(None).unwrap();
    assert_eq!(wm, T0 + 55_000, "watermark = newest time_updated");

    let sessions: Vec<_> = ev.iter().filter_map(|e| match e { RawEvent::Session { session_id, tool, project, cwd, .. } => Some((session_id.as_str(), tool.as_str(), project.as_str(), cwd.clone())), _ => None }).collect();
    assert!(sessions.contains(&("ses_a", "opencode", "bumeter", Some("/home/u/project/bumeter".into()))), "{sessions:?}");
    assert!(ev.contains(&RawEvent::Title { session_id: "ses_a".into(), title: "Add forecast".into() }));
    assert!(!ev.iter().any(|e| matches!(e, RawEvent::Session { session_id, .. } | RawEvent::Title { session_id, .. } if session_id == "ses_sub")), "a subagent is not a conversation of its own");

    let prompts: Vec<_> = ev.iter().filter_map(|e| match e { RawEvent::Prompt { key, text, .. } => Some((key.as_str(), text.as_str())), _ => None }).collect();
    assert_eq!(prompts, vec![("opencode:msg_u1", "read the project"), ("opencode:msg_u2", "now add tests")], "synthetic/system/idle and the subagent's task are not turns");

    let c = calls(&ev);
    assert_eq!(c.len(), 3);
    let a1 = &c[0];
    assert_eq!(a1.0, "opencode:msg_a1");
    assert_eq!(a1.1.as_deref(), Some("opencode:msg_u1"));
    assert_eq!(a1.2, "opencode:deepseek/deepseek-v4.1-flash");
    assert_eq!(a1.3, Usage { input: 100, output: 20, reasoning: 30, cache_read: 5_000, cache_write_5m: 400, cache_write_1h: 0 });
    assert_eq!(a1.4, vec![ToolUse { id: "call-1".into(), name: "read".into() }]);
    assert!(!a1.5);
    assert_eq!(a1.6, Some(0.01));
    assert_eq!(c[1].1.as_deref(), Some("opencode:msg_u1"), "second call of the same turn");
    assert!(c[2].5, "subagent call → sidechain");
    assert_eq!(c[2].1.as_deref(), Some("opencode:msg_u1"), "counted in the parent turn that spawned it");
    let sub_sid = ev.iter().find_map(|e| match e { RawEvent::ApiCall { request_id, session_id, .. } if request_id == "opencode:msg_s1" => Some(session_id.clone()), _ => None });
    assert_eq!(sub_sid.as_deref(), Some("ses_a"), "attached to the parent session, like Claude Code subagents");
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Compacted { session_id, .. } if session_id == "ses_a")));
}

#[test]
fn incremental_scan_reads_changed_rows_and_finds_the_turn_before_the_watermark() {
    let (_d, db) = fixture();
    let mut src = OpenCode::new(&db);
    let (_, wm) = src.scan(None).unwrap();
    let c = Connection::open(&db).unwrap();
    // A new answer to msg_u2 (asked before the watermark), and an old row rewritten long after it was created.
    c.execute("INSERT INTO session_message VALUES('msg_a3', 'ses_a', 'assistant', 9, ?1, ?1, ?2)", params![wm + 100, assistant(50, 10, 0, 0, 0, 0.003, &[])]).unwrap();
    c.execute("UPDATE session_message SET time_updated = ?1, data = ?2 WHERE id = 'msg_a1'", params![wm + 200, assistant(100, 25, 30, 5_000, 400, 0.012, &[])]).unwrap();
    let (ev, wm2) = src.scan(Some(wm)).unwrap();
    assert_eq!(wm2, wm + 200);
    let c = calls(&ev);
    // Only changed rows: the lookback window is re-read, but rows already returned and unchanged are not sent again.
    assert_eq!(c.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["opencode:msg_a1", "opencode:msg_a3"]);
    assert_eq!(c[1].1.as_deref(), Some("opencode:msg_u2"), "turn looked up before the watermark");
    assert_eq!(c[0].3.output, 25, "rewritten row re-read");
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Session { session_id, .. } if session_id == "ses_a")), "session emitted for changed messages");
    assert!(!ev.iter().any(|e| matches!(e, RawEvent::Prompt { .. })), "unchanged user rows not re-sent");

    let (again, wm3) = src.scan(Some(wm2)).unwrap();
    assert!(again.is_empty(), "rows already returned and unchanged are not re-sent: {again:?}");
    assert_eq!(wm3, wm2, "watermark kept when nothing changed");
}

#[test]
fn an_old_schema_is_an_error_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("opencode.db");
    Connection::open(&db).unwrap().execute_batch("CREATE TABLE session(id TEXT); CREATE TABLE message(id TEXT);").unwrap();
    let err = OpenCode::new(&db).scan(None).unwrap_err().to_string();
    assert!(err.contains("unsupported OpenCode schema"), "{err}");
}

#[test]
fn accepts_a_sessions_table_named_session() {
    // OpenCode builds differ: some call the table `session_v2`, others (after migration) `session`.
    let (_d, db) = fixture();
    Connection::open(&db).unwrap().execute_batch("ALTER TABLE session_v2 RENAME TO session;").unwrap();
    let (ev, _) = OpenCode::new(&db).scan(None).unwrap();
    assert!(ev.iter().any(|e| matches!(e, RawEvent::Session { session_id, .. } if session_id == "ses_a")));
    assert!(ev.iter().any(|e| matches!(e, RawEvent::ApiCall { request_id, .. } if request_id == "opencode:msg_a1")));
}

#[test]
fn scanning_never_writes_to_the_opencode_database() {
    let (_d, db) = fixture();
    let before = std::fs::read(&db).unwrap();
    OpenCode::new(&db).scan(None).unwrap();
    assert_eq!(std::fs::read(&db).unwrap(), before);
}

#[test]
fn info_describes_a_token_source_without_extras() {
    let i = bum_adapters::opencode::info();
    assert_eq!((i.id.as_str(), i.label.as_str(), i.letter.as_str()), ("opencode", "OpenCode", "O"));
    assert!(i.real_tokens && !i.composition && !i.limits && !i.credits && !i.transcript);
}

#[test]
fn a_row_committed_at_the_watermark_is_not_lost() {
    let (_d, db) = fixture();
    let mut src = OpenCode::new(&db);
    let (_, wm) = src.scan(None).unwrap();
    // Same millisecond as the newest row, committed after our read (another process, or a final streaming update).
    Connection::open(&db).unwrap().execute("INSERT INTO session_message VALUES('msg_late', 'ses_a', 'assistant', 10, ?1, ?1, ?2)", params![wm, assistant(1, 1, 0, 0, 0, 0.0, &[])]).unwrap();
    let (ev, _) = src.scan(Some(wm)).unwrap();
    assert!(calls(&ev).iter().any(|c| c.0 == "opencode:msg_late"), "lookback re-reads the boundary");
}

#[test]
fn a_late_updated_old_question_does_not_capture_a_new_answer() {
    let (_d, db) = fixture();
    let mut src = OpenCode::new(&db);
    let (_, wm) = src.scan(None).unwrap();
    let c = Connection::open(&db).unwrap();
    c.execute("UPDATE session_message SET time_updated = ?1 WHERE id = 'msg_u1'", [wm + 50]).unwrap();
    c.execute("INSERT INTO session_message VALUES('msg_a3', 'ses_a', 'assistant', 9, ?1, ?1, ?2)", params![wm + 100, assistant(5, 5, 0, 0, 0, 0.0, &[])]).unwrap();
    let (ev, _) = src.scan(Some(wm)).unwrap();
    let a3 = calls(&ev).into_iter().find(|c| c.0 == "opencode:msg_a3").unwrap();
    assert_eq!(a3.1.as_deref(), Some("opencode:msg_u2"), "belongs to the latest question before it, not to the re-read old one");
}

#[test]
fn an_idle_database_yields_no_events_on_later_scans() {
    let (_d, db) = fixture();
    let mut src = OpenCode::new(&db);
    let (_, wm) = src.scan(None).unwrap();
    let (ev, _) = src.scan(Some(wm)).unwrap();
    assert!(ev.is_empty(), "the lookback window was already returned: {ev:?}");
}

#[test]
fn an_empty_question_is_never_used_as_a_turn() {
    let (_d, db) = fixture();
    let c = Connection::open(&db).unwrap();
    c.execute("INSERT INTO session_message VALUES('msg_empty', 'ses_a', 'user', 11, ?1, ?1, ?2)", params![T0 + 60_000, user("  ")]).unwrap();
    c.execute("INSERT INTO session_message VALUES('msg_a9', 'ses_a', 'assistant', 12, ?1, ?1, ?2)", params![T0 + 61_000, assistant(1, 1, 0, 0, 0, 0.0, &[])]).unwrap();
    let (ev, _) = OpenCode::new(&db).scan(None).unwrap();
    let a9 = calls(&ev).into_iter().find(|c| c.0 == "opencode:msg_a9").unwrap();
    assert_eq!(a9.1.as_deref(), Some("opencode:msg_u2"), "the empty row isn't a Prompt, so it can't be a turn either");
}
