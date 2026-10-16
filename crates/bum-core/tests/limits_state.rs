use bum_core::limits::{state, Level, LimitsFile, Window};

#[test]
fn level_follows_highest_fresh_window_and_blocking() {
    let now = 1_000_000;
    let f = |p5: f64, p7: f64| LimitsFile { updated_at: now, five_hour: Some(Window { used_percentage: p5, resets_at: now + 100 }), seven_day: Some(Window { used_percentage: p7, resets_at: now + 100 }) };
    assert_eq!(state(Some(f(10.0, 20.0)), None, now).level(), Level::Green);
    assert_eq!(state(Some(f(71.0, 20.0)), None, now).level(), Level::Yellow);
    assert_eq!(state(Some(f(10.0, 91.0)), None, now).level(), Level::Red);
    assert_eq!(state(None, Some(("five_hour".into(), now + 60)), now).level(), Level::Red);
    assert!(state(None, Some(("five_hour".into(), now - 60)), now).blocked.is_none());
}

#[test]
fn window_past_its_reset_counts_as_zero() {
    let now = 1_000_000;
    let file = LimitsFile { updated_at: now, five_hour: Some(Window { used_percentage: 95.0, resets_at: now - 1 }), seven_day: None };
    assert_eq!(state(Some(file), None, now).level(), Level::Green);
}

#[test]
fn stale_data_is_grey_not_an_alarm_color() {
    let now = 1_000_000;
    let f = LimitsFile { updated_at: now - 3600, five_hour: Some(Window { used_percentage: 95.0, resets_at: now + 100 }), seven_day: None };
    let st = state(Some(f), None, now);
    assert!(st.stale);
    assert_eq!(st.level(), Level::Grey);
    // Đang bị chặn thì vẫn đỏ dù dữ liệu % đã cũ
    assert_eq!(state(None, Some(("five_hour".into(), now + 60)), now).level(), Level::Red);
}
