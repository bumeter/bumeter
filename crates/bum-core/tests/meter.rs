use chrono::NaiveDate;
use bum_core::i18n::Lang;
use bum_core::limits::{state, LimitsFile, Window};
use bum_core::meter::{bar, render, CreditMeter, Snapshot, NAME_PAD};
use bum_core::source::SourceInfo;
use bum_core::store::ToolActivity;

fn src(id: &str, label: &str, limits: bool, credits: bool) -> SourceInfo {
    SourceInfo { id: id.into(), label: label.into(), letter: "X".into(), color: "#000".into(), icon: None, real_tokens: !credits, composition: false, limits, credits, transcript: false }
}

fn sources() -> Vec<SourceInfo> {
    vec![src("claude_code", "Claude Code", true, false), src("kiro", "Kiro", false, true)]
}

/// Tool names are padded (with digit-width spaces) to the longest name, then two spaces.
fn col(label: &str) -> String {
    format!("{label}{}  ", NAME_PAD.to_string().repeat(11 - label.chars().count()))
}

#[test]
fn bar_has_10_cells_rounded_and_clamped() {
    assert_eq!(bar(0.0), "▱▱▱▱▱▱▱▱▱▱");
    assert_eq!(bar(47.0), "▰▰▰▰▰▱▱▱▱▱");
    assert_eq!(bar(79.0), "▰▰▰▰▰▰▰▰▱▱");
    assert_eq!(bar(21.2), "▰▰▱▱▱▱▱▱▱▱");
    // If used, at least 1 cell; below 100% it is not full.
    assert_eq!(bar(0.4), "▰▱▱▱▱▱▱▱▱▱");
    assert_eq!(bar(99.0), "▰▰▰▰▰▰▰▰▰▱");
    assert_eq!(bar(100.0), "▰▰▰▰▰▰▰▰▰▰");
    assert_eq!(bar(180.0), "▰▰▰▰▰▰▰▰▰▰");
    assert_eq!(bar(-5.0), "▱▱▱▱▱▱▱▱▱▱");
    assert_eq!(bar(f64::NAN), "▱▱▱▱▱▱▱▱▱▱");
}

fn snapshot(credit: Option<CreditMeter>, today: Vec<ToolActivity>) -> Snapshot {
    Snapshot { credits: credit.into_iter().map(|c| ("kiro".to_string(), c)).collect(), today }
}

fn act(tool: &str, tokens: u64, credits: f64, prompts: u64) -> ToolActivity {
    ToolActivity { tool: tool.into(), tokens, credits, prompts }
}

#[test]
fn menu_has_two_window_lines_for_limits_tool_and_credit_bar_for_kiro() {
    let st = state(Some(LimitsFile { updated_at: 10_000, five_hour: Some(Window { used_percentage: 47.0, resets_at: 4_102_444_800 }), seven_day: Some(Window { used_percentage: 79.0, resets_at: 4_102_444_800 }) }), None, 10_000);
    let credit = CreditMeter { used: 212.0, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), fetched_at: None, forecast: None };
    let snap = snapshot(Some(credit), vec![act("claude_code", 114_000_000, 0.0, 30), act("kiro", 9_000, 12.34, 54)]);
    let m = render(&sources(), &st, &snap, 10_000, Lang::Vi);
    assert_eq!(m.tools.len(), 3);
    assert!(m.tools[0].starts_with(&format!("{}▰▰▰▰▰▱▱▱▱▱  47% · 5 giờ · reset ", col("Claude Code"))), "{}", m.tools[0]);
    assert!(!m.tools[0].contains("cập nhật"), "fresh data has no note: {}", m.tools[0]);
    assert!(m.tools[1].starts_with(&format!("{}▰▰▰▰▰▰▰▰▱▱  79% · 7 ngày · reset ", NAME_PAD.to_string().repeat(11) + "  ")), "{}", m.tools[1]);
    let tail: Vec<char> = m.tools[1].chars().rev().take(5).collect();
    assert_eq!(tail[2], '/', "7 days shows the reset date dd/mm: {}", m.tools[1]);
    assert_eq!(m.tools[2], format!("{}▰▰▱▱▱▱▱▱▱▱  21% · 212/1000 credit · reset 01/11", col("Kiro")));
    assert_eq!(m.today, "Hôm nay: Claude Code 114.0M token · Kiro 12.3 credit");
    for line in m.tools.iter().chain([&m.today]) {
        println!("{line}");
    }
    let en = render(&sources(), &st, &snap, 10_000, Lang::En);
    assert!(en.tools[0].contains("47% · 5 hours · reset "), "{}", en.tools[0]);
    assert_eq!(en.tools[2], format!("{}▰▰▱▱▱▱▱▱▱▱  21% · 212/1000 credits · reset Nov 1", col("Kiro")));
    assert_eq!(en.today, "Today: Claude Code 114.0M tokens · Kiro 12.3 credits");
}

#[test]
fn stale_and_missing_windows_keep_their_notes() {
    let st = state(Some(LimitsFile { updated_at: 0, five_hour: Some(Window { used_percentage: 78.0, resets_at: 4_102_444_800 }), seven_day: None }), None, 10_000);
    let m = render(&sources(), &st, &snapshot(None, vec![]), 10_000, Lang::Vi);
    assert!(m.tools[0].contains("78% · 5 giờ") && m.tools[0].contains("cập nhật 2 giờ trước"), "{}", m.tools[0]);
    assert!(m.tools[1].ends_with("7 ngày · chưa có dữ liệu"), "{}", m.tools[1]);
    let m = render(&sources(), &st, &snapshot(None, vec![]), 10_000, Lang::En);
    assert!(m.tools[0].contains("updated 2h ago"), "{}", m.tools[0]);
    assert!(m.tools[1].ends_with("7 days · no data yet"), "{}", m.tools[1]);
}

#[test]
fn over_limit_shows_real_percent_with_full_bar() {
    let st = state(None, None, 0);
    let credit = CreditMeter { used: 1234.5, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 15).unwrap(), fetched_at: None, forecast: None };
    let m = render(&sources(), &st, &snapshot(Some(credit), vec![]), 0, Lang::Vi);
    assert_eq!(m.tools[2], format!("{}▰▰▰▰▰▰▰▰▰▰  123% · 1234/1000 credit · reset 15/11", col("Kiro")));
}

#[test]
fn tool_without_percent_shows_today_usage_and_today_line_skips_idle_tools() {
    let st = state(None, None, 0);
    let m = render(&sources(), &st, &snapshot(None, vec![act("kiro", 9_000, 12.34, 54)]), 0, Lang::Vi);
    assert_eq!(m.tools[2], format!("{}hôm nay 12.3 credit · 54 lượt", col("Kiro")));
    assert_eq!(m.today, "Hôm nay: Kiro 12.3 credit");
    // A token-based tool (no quota) shows tokens.
    let srcs = vec![src("other", "Other", false, false)];
    let m = render(&srcs, &st, &snapshot(None, vec![act("other", 2_100_000, 0.0, 3)]), 0, Lang::En);
    assert_eq!(m.tools, vec!["Other  today 2.1M tokens · 3 turns".to_string()]);
    let m = render(&srcs, &st, &snapshot(None, vec![]), 0, Lang::Vi);
    assert_eq!(m.tools, vec!["Other  hôm nay 0 token · 0 lượt".to_string()]);
    assert_eq!(m.today, "Hôm nay: chưa có hoạt động");
}

#[test]
fn line_count_depends_only_on_source_info() {
    use bum_core::meter::line_count;
    assert_eq!(sources().iter().map(line_count).collect::<Vec<_>>(), vec![2, 1]);
}

#[test]
fn credit_from_kiro_usage_notes_age_only_after_20_minutes() {
    let st = state(None, None, 0);
    let mk = |fetched_at| CreditMeter { used: 270.69, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), fetched_at: Some(fetched_at), forecast: None };
    let now = 100_000;
    let m = render(&sources(), &st, &snapshot(Some(mk(now - 19 * 60)), vec![]), now, Lang::Vi);
    assert_eq!(m.tools[2], format!("{}▰▰▰▱▱▱▱▱▱▱  27% · 271/1000 credit · reset 01/11", col("Kiro")));
    let m = render(&sources(), &st, &snapshot(Some(mk(now - 25 * 60)), vec![]), now, Lang::Vi);
    assert!(m.tools[2].ends_with("reset 01/11 · cập nhật 25 phút trước"), "{}", m.tools[2]);
}

#[test]
fn credit_meter_prefers_fresh_kiro_usage_then_user_limit() {
    use bum_core::config::ToolLimit;
    use bum_core::meter::{pick_credit, ToolQuota};
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-16T05:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let q = |age_min: i64| ToolQuota { used: 270.69, limit: 1000.0, resets_on: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), plan: "KIRO PRO".into(), fetched_at: now.timestamp() - age_min * 60 };
    let user = ToolLimit { monthly_credits: Some(500.0), reset_day: 20 };
    let log_used = || Ok::<f64, rusqlite::Error>(42.0);
    // Newer /usage (< 1 hour) wins.
    let c = pick_credit(Some(&q(10)), Some(&user), now, |_, _| log_used()).unwrap().unwrap();
    assert_eq!((c.used, c.limit, c.fetched_at), (270.69, 1000.0, Some(now.timestamp() - 600)));
    // Older /usage → user-entered limit + credits from the log, period based on the reset day.
    let c = pick_credit(Some(&q(61)), Some(&user), now, |_, _| log_used()).unwrap().unwrap();
    assert_eq!((c.used, c.limit, c.next_reset, c.fetched_at), (42.0, 500.0, NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(), None));
    // Nothing / empty limit → None.
    assert!(pick_credit(None, None, now, |_, _| log_used()).unwrap().is_none());
    assert!(pick_credit(Some(&q(61)), Some(&ToolLimit::default()), now, |_, _| log_used()).unwrap().is_none());
}

#[test]
fn tool_quotas_file_round_trips_and_missing_is_empty() {
    use bum_core::meter::{read_quotas, write_quotas, ToolQuota};
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("tool_quotas.json");
    assert!(read_quotas(&p).is_empty());
    let mut m = std::collections::BTreeMap::new();
    m.insert("kiro".to_string(), ToolQuota { used: 1.5, limit: 1000.0, resets_on: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), plan: "KIRO PRO".into(), fetched_at: 7 });
    write_quotas(&p, &m).unwrap();
    assert_eq!(read_quotas(&p), m);
}
