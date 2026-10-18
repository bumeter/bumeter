mod common;
use common::*;
use serde_json::json;
use bum_core::config::Config;
use bum_core::handlers::{dispatch, set_note, HandlerCtx};
use bum_core::paths::Paths;
use bum_core::pricing::Pricing;
use bum_core::store::Store;

#[test]
fn dispatch_serves_list_detail_and_counts() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut s = Store::open(&paths.db()).unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    let (pricing, cfg) = (Pricing::default_table(), Config::default());
    let ctx = HandlerCtx { store: &s, pricing: &pricing, cfg: &cfg, paths: &paths };
    let now = chrono::Utc::now();

    let list = dispatch(&ctx, "list_prompts", &json!({"q": "mean"}), now).unwrap();
    assert_eq!(list["groups"][0]["rows"][0]["stats"]["key"], "p2");
    let detail = dispatch(&ctx, "prompt_detail", &json!({"key": "p1"}), now).unwrap();
    assert_eq!(detail["calls"].as_array().unwrap().len(), 2);
    assert!(dispatch(&ctx, "prompt_detail", &json!({}), now).is_err());
    let counts = dispatch(&ctx, "sidebar_counts", &json!(null), now).unwrap();
    assert_eq!(counts["prompts"], 2);
    let bad = dispatch(&ctx, "list_prompts", &json!({"quick": "nope"}), now);
    assert!(bad.is_err());
}

#[test]
fn set_note_writes_db_and_backup_file() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    Store::open(&paths.db()).unwrap();
    set_note(&paths, "p1", true, "review", chrono::Utc::now()).unwrap();
    let s = Store::open_readonly(&paths.db()).unwrap();
    assert!(s.get_note("p1").unwrap().unwrap().starred);
    let backup = std::fs::read_to_string(paths.notes()).unwrap();
    assert!(backup.contains("review"));
}

#[test]
fn dispatch_latest_anomaly_ts_matches_store() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut s = Store::open(&paths.db()).unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    let (pricing, cfg) = (Pricing::default_table(), Config::default());
    let now = chrono::Utc::now();
    {
        let ctx = HandlerCtx { store: &s, pricing: &pricing, cfg: &cfg, paths: &paths };
        assert_eq!(dispatch(&ctx, "latest_anomaly_ts", &json!(null), now).unwrap(), json!(null));
    }
    s.alert_once("anomaly:p2", "2026-09-11T06:00:00.000Z").unwrap();
    let ctx = HandlerCtx { store: &s, pricing: &pricing, cfg: &cfg, paths: &paths };
    assert_eq!(dispatch(&ctx, "latest_anomaly_ts", &json!({}), now).unwrap(), json!("2026-09-11T06:00:00.000Z"));
    assert_eq!(dispatch(&ctx, "latest_anomaly_ts", &json!({"tool": "kiro"}), now).unwrap(), json!(null));
}

#[test]
fn dispatch_effectiveness_covers_the_ui_range_and_points_at_the_session_to_split() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut s = Store::open(&paths.db()).unwrap();
    let mut events = main_events();
    // A second overflow in the same session: now it is worth splitting.
    events.push(bum_core::model::RawEvent::Compacted { session_id: "s1".into(), ts: "2026-09-11T04:50:00.000Z".into() });
    s.ingest_batch(&offset(MAIN), &events, 0).unwrap();
    let (pricing, cfg) = (Pricing::default_table(), Config::default());
    let ctx = HandlerCtx { store: &s, pricing: &pricing, cfg: &cfg, paths: &paths };
    let now = "2026-09-12T00:00:00Z".parse::<chrono::DateTime<chrono::Utc>>().unwrap();

    // `from` is local midnight from the UI, with its offset: 07:00+07:00 is 00:00Z.
    let e = dispatch(&ctx, "effectiveness", &json!({"from": "2026-09-11T07:00:00+07:00"}), now).unwrap();
    assert_eq!(e["turns"], 2);
    assert_eq!(e["interrupted_turns"], 1);
    assert_eq!(e["compactions"], 2);
    let split = e["advice"].as_array().unwrap().iter().find(|a| a["code"] == "split_session").unwrap();
    assert_eq!(split["session_id"], "s1");
    assert_eq!(split["confidence"], "likely");

    // A range that starts after the history, and another tool, are both empty.
    let later = dispatch(&ctx, "effectiveness", &json!({"from": "2026-09-11T05:00:00Z"}), now).unwrap();
    assert_eq!((later["turns"].as_u64(), later["compactions"].as_u64()), (Some(0), Some(0)));
    let kiro = dispatch(&ctx, "effectiveness", &json!({"from": "2026-09-11T00:00:00Z", "tool": "kiro"}), now).unwrap();
    assert_eq!(kiro["turns"], 0);
    assert!(dispatch(&ctx, "effectiveness", &json!({"from": "yesterday"}), now).is_err());
}
