mod common;
use chrono::NaiveDate;
use common::*;
use tk_core::config::{Config, ToolLimit};
use tk_core::meter::credit_period;
use tk_core::model::RawEvent;
use tk_core::store::Store;

#[test]
fn old_config_without_tool_limits_reads_as_empty() {
    let cfg: Config = toml::from_str("anomaly_multiplier = 2.5\nlanguage = \"en\"\n").unwrap();
    assert!(cfg.tool_limits.is_empty());
    assert_eq!(cfg.anomaly_multiplier, 2.5);
}

#[test]
fn tool_limits_round_trip_through_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut cfg = Config::default();
    cfg.tool_limits.insert("kiro".into(), ToolLimit { monthly_credits: Some(1000.0), reset_day: 15 });
    cfg.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("[tool_limits.kiro]"), "{text}");
    assert_eq!(Config::load(&path), cfg);
}

#[test]
fn tool_limit_reset_day_defaults_to_1_and_blank_limit_is_none() {
    let cfg: Config = toml::from_str("[tool_limits.kiro]\n").unwrap();
    assert_eq!(cfg.tool_limits["kiro"], ToolLimit { monthly_credits: None, reset_day: 1 });
    assert_eq!(ToolLimit::default().reset_day, 1);
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

#[test]
fn credit_period_starts_on_the_latest_reset_day_not_after_today() {
    // Hôm nay đúng ngày reset: kỳ bắt đầu hôm nay.
    assert_eq!(credit_period(d(2026, 10, 15), 15), (d(2026, 10, 15), d(2026, 11, 15)));
    // Sau ngày reset.
    assert_eq!(credit_period(d(2026, 10, 20), 15), (d(2026, 10, 15), d(2026, 11, 15)));
    // Trước ngày reset: kỳ từ tháng trước.
    assert_eq!(credit_period(d(2026, 10, 3), 15), (d(2026, 9, 15), d(2026, 10, 15)));
    // Qua năm.
    assert_eq!(credit_period(d(2026, 1, 3), 15), (d(2025, 12, 15), d(2026, 1, 15)));
    assert_eq!(credit_period(d(2026, 12, 31), 1), (d(2026, 12, 1), d(2027, 1, 1)));
    // Cuối tháng ngắn với ngày reset 28.
    assert_eq!(credit_period(d(2026, 2, 28), 28), (d(2026, 2, 28), d(2026, 3, 28)));
    assert_eq!(credit_period(d(2026, 3, 1), 28), (d(2026, 2, 28), d(2026, 3, 28)));
    // Giá trị ngoài 1–28 bị kẹp.
    assert_eq!(credit_period(d(2026, 2, 28), 31), (d(2026, 2, 28), d(2026, 3, 28)));
    assert_eq!(credit_period(d(2026, 2, 5), 0), (d(2026, 2, 1), d(2026, 3, 1)));
}

fn kiro_call(sid: &str, key: &str, n: u32, ts: &str, credits: f64) -> Vec<RawEvent> {
    let rid = format!("kiro:{sid}:{n}");
    vec![
        RawEvent::Prompt { key: key.into(), session_id: sid.into(), ts: ts.into(), text: "x".into() },
        RawEvent::ApiCall { request_id: rid.clone(), session_id: sid.into(), prompt_key: Some(key.into()), ts: ts.into(), model: "kiro:auto".into(), usage: u(1000, 0, 0, 0), tool_uses: vec![], sidechain: false },
        RawEvent::Metered { request_id: rid, credits },
    ]
}

fn loaded() -> Store {
    let mut s = Store::open_in_memory().unwrap();
    s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap();
    let mut ev = vec![RawEvent::Session { session_id: "k1".into(), tool: "kiro".into(), project: "P".into(), cwd: None, version: None, ts: "2026-09-01T00:00:00.000Z".into() }];
    ev.extend(kiro_call("k1", "kp1", 0, "2026-09-10T01:00:00.000Z", 0.5));
    ev.extend(kiro_call("k1", "kp2", 1, "2026-09-11T05:00:00.000Z", 1.25));
    ev.extend(kiro_call("k1", "kp3", 2, "2026-09-11T06:00:00.000Z", 2.0));
    s.ingest_batch(&offset("/kiro/k1.json"), &ev, 0).unwrap();
    s
}

#[test]
fn tool_credits_between_counts_only_that_tool_in_range() {
    let s = loaded();
    let c = s.tool_credits_between("kiro", "2026-09-11T00:00:00.000Z", "2026-09-12T00:00:00.000Z").unwrap();
    assert!((c - 3.25).abs() < 1e-9, "{c}");
    let all = s.tool_credits_between("kiro", "2026-01-01T00:00:00.000Z", "2027-01-01T00:00:00.000Z").unwrap();
    assert!((all - 3.75).abs() < 1e-9, "{all}");
    assert_eq!(s.tool_credits_between("claude_code", "2026-01-01T00:00:00.000Z", "2027-01-01T00:00:00.000Z").unwrap(), 0.0);
}

#[test]
fn tool_activity_between_groups_tokens_credits_and_turns_by_tool() {
    let s = loaded();
    let a = s.tool_activity_between("2026-09-11T00:00:00.000Z", "2026-09-12T00:00:00.000Z").unwrap();
    let get = |t: &str| a.iter().find(|x| x.tool == t).cloned().unwrap();
    let cc = get("claude_code");
    assert_eq!((cc.prompts, cc.tokens), (2, 3489));
    let k = get("kiro");
    assert_eq!((k.prompts, k.tokens), (2, 2000));
    assert!((k.credits - 3.25).abs() < 1e-9);
    assert!(s.tool_activity_between("2030-01-01T00:00:00.000Z", "2030-01-02T00:00:00.000Z").unwrap().is_empty());
}

#[test]
fn blank_limit_from_settings_json_saves_and_reloads() {
    // UI gửi `monthly_credits: null` khi ô trống.
    let cfg: Config = serde_json::from_value(serde_json::json!({ "tool_limits": { "kiro": { "monthly_credits": null, "reset_day": 5 } } })).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    cfg.save(&path).unwrap();
    assert_eq!(Config::load(&path).tool_limits["kiro"], ToolLimit { monthly_credits: None, reset_day: 5 });
}
