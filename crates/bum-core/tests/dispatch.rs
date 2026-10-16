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

    let list = dispatch(&ctx, "list_prompts", &json!({"q": "ntn"}), now).unwrap();
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
    set_note(&paths, "p1", true, "xem lại", chrono::Utc::now()).unwrap();
    let s = Store::open_readonly(&paths.db()).unwrap();
    assert!(s.get_note("p1").unwrap().unwrap().starred);
    let backup = std::fs::read_to_string(paths.notes()).unwrap();
    assert!(backup.contains("xem lại"));
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
