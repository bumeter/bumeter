use bum_core::forecast::Sample;
use bum_core::limits::{LimitsFile, Window};
use bum_core::store::limit_samples::{record_file, KEEP_SECS};
use bum_core::store::Store;

fn s(ts: i64, pct: f64) -> Sample {
    Sample { ts, pct, resets_at: 9_000 }
}

#[test]
fn same_percentage_within_a_minute_is_not_stored_again() {
    let mut st = Store::open_in_memory().unwrap();
    assert!(st.record_limit_sample("five_hour", s(1000, 40.0)).unwrap());
    assert!(!st.record_limit_sample("five_hour", s(1030, 40.0)).unwrap(), "same pct, 30 s later");
    assert!(st.record_limit_sample("five_hour", s(1061, 40.0)).unwrap(), "same pct, over 60 s later");
    assert!(st.record_limit_sample("five_hour", s(1062, 41.0)).unwrap(), "pct changed");
    assert!(!st.record_limit_sample("five_hour", s(1062, 42.0)).unwrap(), "not newer than the last sample");
    assert!(st.record_limit_sample("seven_day", s(1030, 40.0)).unwrap(), "windows are independent");
    let got = st.limit_samples_since("five_hour", 0).unwrap();
    assert_eq!(got, vec![s(1000, 40.0), s(1061, 40.0), s(1062, 41.0)]);
    assert_eq!(st.limit_samples_since("five_hour", 1061).unwrap().len(), 2);
}

#[test]
fn record_file_stores_both_windows_skips_future_files_and_prunes_old_rows() {
    let mut st = Store::open_in_memory().unwrap();
    let now = 2_000_000;
    st.record_limit_sample("five_hour", s(now - KEEP_SECS - 1, 10.0)).unwrap();
    let w = |pct| Some(Window { used_percentage: pct, resets_at: now + 3600 });
    record_file(&mut st, &LimitsFile { updated_at: now, five_hour: w(42.0), seven_day: w(63.0) }, now).unwrap();
    record_file(&mut st, &LimitsFile { updated_at: now + 600, five_hour: w(99.0), seven_day: None }, now).unwrap();
    let five = st.limit_samples_since("five_hour", 0).unwrap();
    assert_eq!(five, vec![Sample { ts: now, pct: 42.0, resets_at: now + 3600 }], "old row pruned, future file ignored");
    assert_eq!(st.limit_samples_since("seven_day", 0).unwrap().len(), 1);
}

#[test]
fn readonly_open_upgrades_an_older_db_with_the_samples_table() {
    // The app's RPC handlers open the DB read-only; an older DB must still get the new table.
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("b.db");
    drop(Store::open(&db).unwrap());
    rusqlite::Connection::open(&db).unwrap().execute_batch("DROP TABLE limit_samples; PRAGMA user_version = 5;").unwrap();
    let ro = Store::open_readonly(&db).unwrap();
    assert!(ro.limit_samples_since("five_hour", 0).unwrap().is_empty());
}
