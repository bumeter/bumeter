use bum_core::forecast::Sample;
use bum_core::i18n::Lang;
use bum_core::limits::{render_line, state, Level, LimitsFile, Window};

fn rising(now: i64, resets_at: i64) -> Vec<Sample> {
    (0..=6).map(|i| Sample { ts: now - 1800 + i * 300, pct: 28.0 + 2.0 * i as f64, resets_at }).collect()
}

fn fresh(now: i64, pct5: f64, resets_at: i64) -> LimitsFile {
    LimitsFile { updated_at: now, five_hour: Some(Window { used_percentage: pct5, resets_at }), seven_day: None }
}

#[test]
fn forecast_is_attached_and_raises_the_level_when_at_risk() {
    let now = 1_000_000;
    let st = state(Some(fresh(now, 40.0, now + 4 * 3600)), None, now).with_forecast(&rising(now, now + 4 * 3600), now);
    let f = st.forecast_five_hour.expect("forecast");
    assert!(f.at_risk, "60 points at 24 %/h = 2.5 h, before the 4 h reset");
    assert!(st.at_risk());
    assert_eq!(st.level(), Level::Yellow, "40% but at risk");
    assert!(render_line(&st, None, Lang::Vi).contains("hết ~"), "{}", render_line(&st, None, Lang::Vi));
    assert!(render_line(&st, None, Lang::En).contains("out ~"));
}

#[test]
fn safe_stale_blocked_or_mismatched_window_has_no_visible_forecast() {
    let now = 1_000_000;
    let safe = state(Some(fresh(now, 40.0, now + 3600)), None, now).with_forecast(&rising(now, now + 3600), now);
    assert!(!safe.at_risk());
    assert_eq!(safe.level(), Level::Green);
    assert!(!render_line(&safe, None, Lang::Vi).contains("hết ~"));

    let stale_file = fresh(now - 3600, 40.0, now + 4 * 3600); // read an hour ago
    assert_eq!(state(Some(stale_file), None, now).with_forecast(&rising(now, now + 4 * 3600), now).forecast_five_hour, None);

    let blocked = state(Some(fresh(now, 40.0, now + 4 * 3600)), Some(("five_hour".into(), now + 60)), now).with_forecast(&rising(now, now + 4 * 3600), now);
    assert_eq!(blocked.forecast_five_hour, None);

    // Samples belong to an older window than limits.json: ignored.
    let other = state(Some(fresh(now, 40.0, now + 5 * 3600)), None, now).with_forecast(&rising(now, now + 4 * 3600), now);
    assert_eq!(other.forecast_five_hour, None);
}

#[test]
fn seven_day_forecast_uses_the_window_from_limits_json() {
    let now = 1_000_000;
    let file = LimitsFile { updated_at: now, five_hour: None, seven_day: Some(Window { used_percentage: 80.0, resets_at: now + 3 * 86_400 }) };
    let st = state(Some(file), None, now).with_forecast(&[], now);
    assert!(st.forecast_seven_day.expect("7-day forecast").at_risk);
}

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
    // While blocked it stays red even if the % data is stale
    assert_eq!(state(None, Some(("five_hour".into(), now + 60)), now).level(), Level::Red);
}
