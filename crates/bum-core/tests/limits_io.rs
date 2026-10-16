use serde_json::json;
use bum_core::limits::{from_statusline, read, render_line, state, write_atomic, Window};

#[test]
fn parses_rate_limits_from_statusline_input() {
    let input = json!({
        "session_id": "s1",
        "rate_limits": {
            "five_hour": { "used_percentage": 78.4, "resets_at": 1789902000 },
            "seven_day": { "used_percentage": 41, "resets_at": 1790000000 }
        }
    });
    let f = from_statusline(&input, 1_789_900_000).unwrap();
    assert_eq!(f.updated_at, 1_789_900_000);
    assert_eq!(f.five_hour, Some(Window { used_percentage: 78.4, resets_at: 1789902000 }));
    assert_eq!(f.seven_day.unwrap().used_percentage, 41.0);
}

#[test]
fn input_without_rate_limits_gives_none() {
    assert!(from_statusline(&json!({"session_id": "s1"}), 0).is_none());
}

#[test]
fn write_then_read_roundtrip_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("limits.json");
    let f = from_statusline(&json!({"rate_limits": {"five_hour": {"used_percentage": 10, "resets_at": 5}}}), 1).unwrap();
    write_atomic(&p, &f).unwrap();
    assert_eq!(read(&p), Some(f));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn renders_compact_status_line() {
    let now = 1_789_900_000;
    let f = from_statusline(&json!({"rate_limits": {
        "five_hour": {"used_percentage": 78, "resets_at": now + 3600},
        "seven_day": {"used_percentage": 41, "resets_at": now + 86400}}}), now).unwrap();
    let line = render_line(&state(Some(f), None, now), Some(2_100_000), bum_core::i18n::Lang::Vi);
    assert!(line.starts_with("🟡"), "{line}");
    assert!(line.contains("5h 78%"), "{line}");
    assert!(line.contains("7d 41%"), "{line}");
    assert!(line.contains("hôm nay 2.1M"), "{line}");
}

#[test]
fn renders_placeholder_without_data() {
    let line = render_line(&state(None, None, 0), None, bum_core::i18n::Lang::En);
    assert!(line.contains("5h —"), "{line}");
}
