//! Engine side of database sources (`DbSource`): watermark, triggers, no purge, watch targets.
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bum_core::engine::Engine;
use bum_core::model::RawEvent;
use bum_core::source::{DbSource, SourceInfo};
use bum_core::store::Store;

/// Queued scan results: (events, new watermark).
type Batches = Arc<Mutex<Vec<(Vec<RawEvent>, i64)>>>;

/// Replays queued batches; records the watermark each scan received.
struct Fake {
    path: PathBuf,
    batches: Batches,
    seen: Arc<Mutex<Vec<Option<i64>>>>,
    fail: bool,
}

impl DbSource for Fake {
    fn id(&self) -> &'static str {
        "fakedb"
    }
    fn info(&self) -> SourceInfo {
        SourceInfo { id: "fakedb".into(), label: "Fake".into(), letter: "F".into(), color: "#000".into(), icon: None, real_tokens: true, composition: false, limits: false, credits: false, transcript: false }
    }
    fn db_path(&self) -> PathBuf {
        self.path.clone()
    }
    fn scan(&mut self, watermark: Option<i64>) -> anyhow::Result<(Vec<RawEvent>, i64)> {
        self.seen.lock().unwrap().push(watermark);
        if self.fail {
            anyhow::bail!("locked");
        }
        let mut b = self.batches.lock().unwrap();
        Ok(if b.is_empty() { (vec![], watermark.unwrap_or(0)) } else { b.remove(0) })
    }
}

fn session(id: &str) -> RawEvent {
    RawEvent::Session { session_id: id.into(), tool: "fakedb".into(), project: "p".into(), cwd: None, version: None, ts: "2026-10-17T01:00:00.000Z".into() }
}

fn prompt(key: &str, sid: &str) -> RawEvent {
    RawEvent::Prompt { key: key.into(), session_id: sid.into(), ts: "2026-10-17T01:00:01.000Z".into(), text: "hi".into() }
}

struct Setup {
    _dir: tempfile::TempDir,
    db: PathBuf,
    engine: Engine,
    batches: Batches,
    seen: Arc<Mutex<Vec<Option<i64>>>>,
}

fn setup(fail: bool) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("tool.db");
    std::fs::write(&db, b"x").unwrap();
    let batches = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let fake = Fake { path: db.clone(), batches: batches.clone(), seen: seen.clone(), fail };
    let engine = Engine::new(Store::open_in_memory().unwrap(), vec![]).with_db_sources(vec![Box::new(fake)]);
    Setup { _dir: dir, db, engine, batches, seen }
}

fn prompt_count(e: &Engine) -> u64 {
    e.store().tool_rows().unwrap().iter().filter(|r| r.0 == "fakedb").map(|r| r.2).sum()
}

fn wal(db: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", db.display()))
}

#[test]
fn full_scan_ingests_and_the_next_scan_gets_the_saved_watermark() {
    let mut s = setup(false);
    s.batches.lock().unwrap().push((vec![session("a"), prompt("k1", "a")], 1000));
    let r = s.engine.full_scan().unwrap();
    assert_eq!(r.lines, 2);
    assert_eq!(prompt_count(&s.engine), 1);
    s.engine.full_scan().unwrap();
    assert_eq!(*s.seen.lock().unwrap(), vec![None, Some(1000)]);
}

#[test]
fn a_change_to_the_db_or_its_wal_triggers_a_scan_other_files_do_not() {
    let mut s = setup(false);
    s.engine.ingest_path(&s.db).unwrap();
    s.engine.ingest_path(&wal(&s.db)).unwrap();
    s.engine.ingest_path(&s.db.with_file_name("auth.json")).unwrap();
    assert_eq!(s.seen.lock().unwrap().len(), 2);
}

#[test]
fn a_changed_db_file_never_purges_rows() {
    let mut s = setup(false);
    s.batches.lock().unwrap().push((vec![session("a"), prompt("k1", "a")], 1000));
    s.engine.full_scan().unwrap();
    std::fs::write(&s.db, b"a much longer file after a checkpoint").unwrap();
    s.engine.ingest_path(&s.db).unwrap();
    assert_eq!(prompt_count(&s.engine), 1, "rows kept");
}

#[test]
fn a_missing_db_file_is_a_no_op() {
    let mut s = setup(false);
    std::fs::remove_file(&s.db).unwrap();
    assert_eq!(s.engine.full_scan().unwrap().lines, 0);
    assert!(s.seen.lock().unwrap().is_empty());
}

#[test]
fn a_scan_error_is_reported_and_the_full_scan_goes_on() {
    let mut s = setup(true);
    assert!(s.engine.ingest_path(&s.db).is_err());
    let r = s.engine.full_scan().unwrap();
    assert_eq!(r.file_errors, 1);
}

#[test]
fn db_sources_are_watched_non_recursively_with_their_files_listed() {
    let s = setup(false);
    let t = s.engine.watch_targets();
    let db = t.iter().find(|t| t.path == s.db.parent().unwrap()).expect("db parent watched");
    assert!(!db.recursive);
    assert_eq!(db.files, vec![s.db.clone(), wal(&s.db)]);
}
