mod common;
use common::*;
use bum_core::store::notes::{export_notes, import_notes, Note};
use bum_core::store::Store;

#[test]
fn set_and_get_note() {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    assert_eq!(s.get_note("p1").unwrap(), None);
    s.set_note("p1", true, "xem lại context", "2026-09-24T00:00:00.000Z").unwrap();
    assert_eq!(s.get_note("p1").unwrap(), Some(Note { prompt_key: "p1".into(), starred: true, note: "xem lại context".into(), updated_at: "2026-09-24T00:00:00.000Z".into() }));
    s.set_note("p1", false, "", "2026-09-24T01:00:00.000Z").unwrap();
    assert!(!s.get_note("p1").unwrap().unwrap().starred);
}

#[test]
fn notes_survive_a_rebuilt_database_via_json_backup() {
    let dir = tempfile::tempdir().unwrap();
    let json = dir.path().join("notes.json");
    let mut a = Store::open_in_memory().unwrap();
    a.set_note("p1", true, "hay", "t1").unwrap();
    a.set_note("p2", false, "ghi chú", "t2").unwrap();
    export_notes(&a, &json).unwrap();

    let mut b = Store::open_in_memory().unwrap();
    assert_eq!(import_notes(&mut b, &json).unwrap(), 2);
    assert!(b.get_note("p1").unwrap().unwrap().starred);
    // Không ghi đè ghi chú mới hơn đã có
    b.set_note("p2", true, "mới", "t9").unwrap();
    import_notes(&mut b, &json).unwrap();
    assert_eq!(b.get_note("p2").unwrap().unwrap().note, "mới");
}

#[test]
fn v1_database_migrates_to_v2_without_losing_data() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    {
        let mut s = Store::open(&db).unwrap();
        s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    }
    {
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch("DROP TABLE notes; PRAGMA user_version = 1;").unwrap();
    }
    let mut s = Store::open(&db).unwrap();
    assert_eq!(s.usage_totals().unwrap().total(), 3489);
    s.set_note("p1", true, "", "t").unwrap();
    assert!(s.get_note("p1").unwrap().unwrap().starred);
}

#[test]
fn readonly_client_on_an_old_v1_database_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("t.db");
    {
        let mut s = Store::open(&db).unwrap();
        s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    }
    rusqlite::Connection::open(&db).unwrap().execute_batch("DROP TABLE notes; PRAGMA user_version = 1;").unwrap();
    let r = Store::open_readonly(&db).unwrap();
    assert_eq!(r.prompt_items(None, None, None, None).unwrap().len(), 2);
}
