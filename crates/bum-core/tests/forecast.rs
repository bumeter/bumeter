use bum_core::forecast::{five_hour, paced, seven_day, Sample, CREDIT_MIN_ELAPSED};

const NOW: i64 = 1_000_000;

/// 7 samples over the last 30 min, +2 points every 5 min (24 %/h), last one 52%.
fn steady(resets_at: i64) -> Vec<Sample> {
    (0..=6).map(|i| Sample { ts: NOW - 1800 + i * 300, pct: 40.0 + 2.0 * i as f64, resets_at }).collect()
}

#[test]
fn steady_rise_gives_rate_and_eta() {
    let f = five_hour(&steady(NOW + 3 * 3600), NOW).unwrap();
    assert!((f.rate_per_hour - 24.0).abs() < 1e-6, "{}", f.rate_per_hour);
    assert_eq!(f.hits_at, Some(NOW + 7200)); // 48 points left at 24 %/h
    assert_eq!(f.resets_at, NOW + 3 * 3600);
    assert!(f.at_risk);
}

#[test]
fn eta_after_reset_is_not_at_risk() {
    let f = five_hour(&steady(NOW + 3600), NOW).unwrap();
    assert_eq!(f.hits_at, Some(NOW + 7200));
    assert!(!f.at_risk);
}

#[test]
fn not_enough_evidence_gives_none() {
    let r = NOW + 3600;
    let two = vec![Sample { ts: NOW - 900, pct: 40.0, resets_at: r }, Sample { ts: NOW, pct: 50.0, resets_at: r }];
    assert_eq!(five_hour(&two, NOW), None, "fewer than 3 samples");
    let short: Vec<_> = (0..3).map(|i| Sample { ts: NOW - 300 + i * 150, pct: 40.0 + 5.0 * i as f64, resets_at: r }).collect();
    assert_eq!(five_hour(&short, NOW), None, "span under 10 min");
    let flat: Vec<_> = (0..3).map(|i| Sample { ts: NOW - 1200 + i * 600, pct: 40.0 + 0.5 * i as f64, resets_at: r }).collect();
    assert_eq!(five_hour(&flat, NOW), None, "rise under 2 points");
    assert_eq!(five_hour(&[], NOW), None);
}

#[test]
fn samples_of_another_window_or_too_old_are_ignored() {
    let r = NOW + 3 * 3600;
    let mut s = steady(r);
    s.drain(..4); // keep 3 current samples: NOW-600, NOW-300, NOW (span 10 min, +4 points)
    s.push(Sample { ts: NOW - 1500, pct: 90.0, resets_at: NOW - 100 }); // previous window
    s.push(Sample { ts: NOW - 4000, pct: 10.0, resets_at: r }); // older than 30 min
    let f = five_hour(&s, NOW).unwrap();
    assert!((f.rate_per_hour - 24.0).abs() < 1e-6);
}

#[test]
fn only_samples_after_the_last_drop_count() {
    let r = NOW + 6 * 3600;
    let mut s = vec![
        Sample { ts: NOW - 1700, pct: 80.0, resets_at: r },
        Sample { ts: NOW - 1400, pct: 85.0, resets_at: r },
        Sample { ts: NOW - 1100, pct: 90.0, resets_at: r },
    ];
    s.extend((0..4).map(|i| Sample { ts: NOW - 900 + i * 300, pct: 10.0 + 2.0 * i as f64, resets_at: r }));
    let f = five_hour(&s, NOW).unwrap();
    assert!((f.rate_per_hour - 24.0).abs() < 1e-6);
    assert_eq!(f.hits_at, Some(NOW + 12_600)); // 84 points left
}

#[test]
fn future_samples_are_ignored() {
    let r = NOW + 3 * 3600;
    let mut s = steady(r);
    s.push(Sample { ts: NOW + 600, pct: 99.0, resets_at: r });
    assert_eq!(five_hour(&s, NOW).unwrap().hits_at, Some(NOW + 7200));
}

#[test]
fn seven_day_uses_average_pace_after_12_hours() {
    assert_eq!(seven_day(5.0, NOW + 7 * 86_400 - 3600, NOW), None, "only 1 h into the window");
    let f = seven_day(80.0, NOW + 3 * 86_400, NOW).unwrap(); // 4 days elapsed
    assert!((f.rate_per_hour - 80.0 / 96.0).abs() < 1e-9);
    assert_eq!(f.hits_at, Some(NOW + 86_400));
    assert!(f.at_risk);
}

#[test]
fn paced_credit_forecast_and_guards() {
    let f = paced(482.0, 1000.0, NOW - 10 * 86_400, NOW + 20 * 86_400, NOW, CREDIT_MIN_ELAPSED).unwrap();
    assert_eq!(f.hits_at, Some(NOW + 928_531));
    assert!(f.at_risk);
    assert_eq!(paced(482.0, 0.0, NOW - 10 * 86_400, NOW + 86_400, NOW, 0), None, "no limit");
    assert_eq!(paced(0.0, 1000.0, NOW - 10 * 86_400, NOW + 86_400, NOW, 0), None, "nothing used");
    assert_eq!(paced(10.0, 1000.0, NOW - 86_400, NOW + 86_400, NOW, CREDIT_MIN_ELAPSED), None, "under 2 days");
}
