mod common;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use common::*;
use bum_core::alerts::{Alert, AlertEngine};
use bum_core::config::Config;
use bum_core::engine::IngestReport;
use bum_core::limits::{state, LimitsFile, Window};
use bum_core::store::Store;

fn t(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn setup(cfg: Config) -> (Store, AlertEngine, IngestReport) {
    let mut s = Store::open_in_memory().unwrap();
    let mut report = IngestReport { summary: s.ingest_batch(&offset(MAIN), &main_events(), 0).unwrap(), ..Default::default() };
    report.summary.merge(s.ingest_batch(&offset(SUB), &sub_events(), 0).unwrap());
    (s, AlertEngine::new(cfg, t("2026-09-11T00:00:00Z")), report)
}

fn limits(pct5: f64, updated_at: i64) -> LimitsFile {
    LimitsFile { updated_at, five_hour: Some(Window { used_percentage: pct5, resets_at: 2_000_000_000 }), seven_day: None }
}

#[test]
fn quota_alerts_fire_once_per_threshold_and_cycle() {
    let (mut s, mut a, _) = setup(Config::default());
    let now = 1_789_100_000;
    let st = state(Some(limits(75.0, now)), None, now);
    let first = a.on_limits(&mut s, &st).unwrap();
    assert!(matches!(first.as_slice(), [Alert::Quota { threshold: 70, .. }]));
    assert!(a.on_limits(&mut s, &st).unwrap().is_empty());
    let st = state(Some(limits(92.0, now)), None, now);
    assert!(matches!(a.on_limits(&mut s, &st).unwrap().as_slice(), [Alert::Quota { threshold: 90, .. }]));
}

#[test]
fn stale_limits_never_alert() {
    let (mut s, mut a, _) = setup(Config::default());
    let now = 1_789_100_000;
    let st = state(Some(limits(95.0, now - 3600)), None, now);
    assert!(st.stale);
    assert!(a.on_limits(&mut s, &st).unwrap().is_empty());
}

#[test]
fn new_rate_limit_with_future_reset_alerts_once() {
    let (mut s, mut a, report) = setup(Config::default());
    let now = Instant::now();
    let alerts = a.on_ingest(&mut s, &report, now, t("2026-09-11T05:00:00Z")).unwrap();
    assert!(alerts.iter().any(|x| matches!(x, Alert::Blocked { resets_at: 1789902000, .. })));
    let again = a.on_ingest(&mut s, &report, now, t("2026-09-11T05:00:00Z")).unwrap();
    assert!(!again.iter().any(|x| matches!(x, Alert::Blocked { .. })));
}

#[test]
fn context_alert_fires_once_per_session() {
    let cfg = Config { context_threshold: 1_000, ..Config::default() };
    let (mut s, mut a, report) = setup(cfg);
    let now = Instant::now();
    let alerts = a.on_ingest(&mut s, &report, now, t("2026-09-11T05:00:00Z")).unwrap();
    assert_eq!(alerts.iter().filter(|x| matches!(x, Alert::Context { context: 1201, .. })).count(), 1);
    let again = a.on_ingest(&mut s, &report, now, t("2026-09-11T05:00:00Z")).unwrap();
    assert!(!again.iter().any(|x| matches!(x, Alert::Context { .. })));
}

#[test]
fn anomaly_waits_until_prompt_is_idle_long_enough_for_tool_runs() {
    // p1 = 2343 > 1.5 × median (1206 when only counting other prompts)… use a low multiplier so p1 is anomalous by ratio.
    let cfg = Config { anomaly_multiplier: 0.5, ..Config::default() };
    let (mut s, mut a, report) = setup(cfg);
    let t0 = Instant::now();
    let now_utc = t("2026-09-11T05:00:00Z");
    a.on_ingest(&mut s, &report, t0, now_utc).unwrap();
    assert!(a.on_tick(&mut s, t0 + Duration::from_secs(10), now_utc).unwrap().is_empty(), "10s of build/test is not considered done");
    let fired = a.on_tick(&mut s, t0 + Duration::from_secs(46), now_utc).unwrap();
    assert!(fired.iter().any(|x| matches!(x, Alert::Anomaly { .. })));
    assert!(a.on_tick(&mut s, t0 + Duration::from_secs(100), now_utc).unwrap().is_empty());
}

#[test]
fn big_context_alone_is_left_to_the_context_alert() {
    let cfg = Config { context_threshold: 1_000, anomaly_multiplier: 100.0, ..Config::default() };
    let (mut s, mut a, report) = setup(cfg);
    let t0 = Instant::now();
    let now_utc = t("2026-09-11T05:00:00Z");
    a.on_ingest(&mut s, &report, t0, now_utc).unwrap();
    let fired = a.on_tick(&mut s, t0 + Duration::from_secs(46), now_utc).unwrap();
    assert!(!fired.iter().any(|x| matches!(x, Alert::Anomaly { .. })), "does not double-fire with the context alert");
}

#[test]
fn history_before_start_never_alerts() {
    let cfg = Config { context_threshold: 1_000, ..Config::default() };
    let (mut s, _, report) = setup(cfg.clone());
    let mut a = AlertEngine::new(cfg, t("2026-09-12T00:00:00Z"));
    let t0 = Instant::now();
    let now_utc = t("2026-09-12T01:00:00Z");
    let alerts = a.on_ingest(&mut s, &report, t0, now_utc).unwrap();
    assert!(!alerts.iter().any(|x| matches!(x, Alert::Context { .. })));
    assert!(a.on_tick(&mut s, t0 + Duration::from_secs(46), now_utc).unwrap().is_empty());
}

#[test]
fn disabled_notification_kind_is_skipped() {
    let mut cfg = Config::default();
    cfg.notify.quota = false;
    let (mut s, mut a, _) = setup(cfg);
    let now = 1_789_100_000;
    assert!(a.on_limits(&mut s, &state(Some(limits(95.0, now)), None, now)).unwrap().is_empty());
}

#[test]
fn anomaly_mark_is_recorded_even_when_notification_is_off() {
    // The "anomaly" ring does not depend on the notification flag: it still records the mark, it just does not return an Alert to send.
    let mut cfg = Config { anomaly_multiplier: 0.5, ..Config::default() };
    cfg.notify.anomaly = false;
    let (mut s, mut a, report) = setup(cfg);
    let t0 = Instant::now();
    let now_utc = t("2026-09-11T05:00:00Z");
    a.on_ingest(&mut s, &report, t0, now_utc).unwrap();
    let fired = a.on_tick(&mut s, t0 + Duration::from_secs(46), now_utc).unwrap();
    assert!(!fired.iter().any(|x| matches!(x, Alert::Anomaly { .. })));
    assert_eq!(s.latest_anomaly_ts(None).unwrap().as_deref(), Some("2026-09-11T05:00:00.000Z"));
}

fn at_risk_state(now: i64, minutes_left: i64) -> bum_core::limits::LimitState {
    use bum_core::forecast::Sample;
    // 24 %/h; current pct chosen so the limit is `minutes_left` away, reset 4 h out.
    let reset = now + 4 * 3600;
    let last = 100.0 - 24.0 * minutes_left as f64 / 60.0;
    let samples: Vec<Sample> = (0..=6).map(|i| Sample { ts: now - 1800 + i * 300, pct: last - 2.0 * (6 - i) as f64, resets_at: reset }).collect();
    let file = LimitsFile { updated_at: now, five_hour: Some(Window { used_percentage: last, resets_at: reset }), seven_day: None };
    state(Some(file), None, now).with_forecast(&samples, now)
}

fn forecast_only() -> Config {
    Config { quota_thresholds: vec![], ..Default::default() }
}

#[test]
fn forecast_alert_fires_once_when_the_limit_is_within_the_lead_time() {
    let (mut s, mut a, _) = setup(forecast_only());
    let now = 1_789_100_000;
    assert!(a.on_limits(&mut s, &at_risk_state(now, 60)).unwrap().is_empty(), "60 min away, lead is 30");
    let got = a.on_limits(&mut s, &at_risk_state(now, 25)).unwrap();
    assert!(matches!(got.as_slice(), [Alert::Forecast { window: "five_hour", .. }]), "{got:?}");
    assert!(a.on_limits(&mut s, &at_risk_state(now, 20)).unwrap().is_empty(), "once per cycle");
}

#[test]
fn forecast_alert_respects_the_switch_and_lead_minutes() {
    use bum_core::config::Notify;
    let now = 1_789_100_000;
    let off = Config { notify: Notify { forecast: false, ..Default::default() }, ..forecast_only() };
    let (mut s, mut a, _) = setup(off);
    assert!(a.on_limits(&mut s, &at_risk_state(now, 10)).unwrap().is_empty());
    let long = Config { notify: Notify { forecast_lead_minutes: 90, ..Default::default() }, ..forecast_only() };
    let (mut s, mut a, _) = setup(long);
    assert_eq!(a.on_limits(&mut s, &at_risk_state(now, 60)).unwrap().len(), 1);
}

#[test]
fn quota_switch_off_still_allows_forecast_alerts() {
    use bum_core::config::Notify;
    let now = 1_789_100_000;
    let cfg = Config { notify: Notify { quota: false, ..Default::default() }, ..Config::default() };
    let (mut s, mut a, _) = setup(cfg);
    let got = a.on_limits(&mut s, &at_risk_state(now, 10)).unwrap();
    assert!(matches!(got.as_slice(), [Alert::Forecast { .. }]), "{got:?}");
}

#[test]
fn credit_forecast_alerts_once_within_three_days() {
    use bum_core::forecast::Forecast;
    let (mut s, mut a, _) = setup(Config::default());
    let now = t("2026-10-20T00:00:00Z");
    let f = |days: i64| Forecast { rate_per_hour: 1.0, hits_at: Some(now.timestamp() + days * 86_400), resets_at: now.timestamp() + 12 * 86_400, at_risk: true };
    assert!(a.on_credits(&mut s, &[("kiro".into(), "Kiro".into(), f(5))], now).unwrap().is_empty());
    let got = a.on_credits(&mut s, &[("kiro".into(), "Kiro".into(), f(2))], now).unwrap();
    assert!(matches!(got.as_slice(), [Alert::CreditForecast { tool, .. }] if tool == "kiro"));
    assert!(got[0].body(bum_core::i18n::Lang::En).contains("Kiro"));
    assert!(a.on_credits(&mut s, &[("kiro".into(), "Kiro".into(), f(1))], now).unwrap().is_empty());
}
