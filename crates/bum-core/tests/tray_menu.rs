use chrono::NaiveDate;
use bum_core::i18n::Lang;
use bum_core::limits::{state, LimitsFile, Window};
use bum_core::meter::{render_tray, ring_key, tray_row_count, CreditMeter, RingTone, Snapshot, TrayIcon, TrayRow};
use bum_core::source::SourceInfo;
use bum_core::store::ToolActivity;

fn src(id: &str, label: &str, limits: bool, credits: bool) -> SourceInfo {
    SourceInfo { id: id.into(), label: label.into(), letter: "X".into(), color: "#000".into(), icon: None, real_tokens: !credits, composition: false, limits, credits, transcript: false }
}

fn sources() -> Vec<SourceInfo> {
    vec![src("claude_code", "Claude Code", true, false), src("kiro", "Kiro", false, true)]
}

fn snap(credit: Option<CreditMeter>, today: Vec<ToolActivity>) -> Snapshot {
    Snapshot { credits: credit.into_iter().map(|c| ("kiro".to_string(), c)).collect(), today }
}

fn act(tool: &str, tokens: u64, credits: f64, prompts: u64) -> ToolActivity {
    ToolActivity { tool: tool.into(), tokens, credits, prompts }
}

fn ring(pct: Option<f64>, stale: bool) -> TrayIcon {
    TrayIcon::Ring { pct, stale }
}

#[test]
fn tray_rows_have_header_per_tool_then_metric_lines_without_bars() {
    let st = state(Some(LimitsFile { updated_at: 10_000, five_hour: Some(Window { used_percentage: 21.0, resets_at: 4_102_444_800 }), seven_day: Some(Window { used_percentage: 86.4, resets_at: 4_102_444_800 }) }), None, 10_000);
    let credit = CreditMeter { used: 270.69, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), fetched_at: Some(10_000), forecast: None };
    let t = render_tray(&sources(), &st, &snap(Some(credit), vec![act("claude_code", 16_200_000, 0.0, 30), act("kiro", 9_000, 12.34, 54)]), 10_000, Lang::Vi);
    assert_eq!(t.rows.len(), sources().iter().map(tray_row_count).sum::<usize>());
    assert_eq!(t.rows[0], TrayRow { tool: "claude_code".into(), text: "Claude Code".into(), icon: TrayIcon::Logo });
    assert!(t.rows[1].text.starts_with("5 giờ · 21% · reset "), "{}", t.rows[1].text);
    assert_eq!(t.rows[1].icon, ring(Some(21.0), false));
    assert!(t.rows[2].text.starts_with("7 ngày · 86% · reset "), "{}", t.rows[2].text);
    assert_eq!(t.rows[2].text.chars().rev().nth(2), Some('/'), "7 days shows dd/mm: {}", t.rows[2].text);
    assert_eq!(t.rows[3], TrayRow { tool: "kiro".into(), text: "Kiro".into(), icon: TrayIcon::Logo });
    assert_eq!(t.rows[4].text, "Tháng · 27% · 271/1000 credit · reset 01/11");
    assert_eq!(t.rows[4].tool, "kiro");
    assert!(matches!(t.rows[4].icon, TrayIcon::Ring { pct: Some(p), stale: false } if (p - 27.069).abs() < 1e-9));
    for r in &t.rows {
        assert!(!r.text.contains('▰') && !r.text.contains('▱'), "{}", r.text);
    }
    assert_eq!(t.today, "Hôm nay: Claude Code 16.2M token · Kiro 12.3 credit");
    assert_eq!(t.title.as_deref(), Some("86%"));
    let en = render_tray(&sources(), &st, &snap(Some(CreditMeter { used: 270.69, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), fetched_at: None, forecast: None }), vec![]), 10_000, Lang::En);
    assert!(en.rows[1].text.starts_with("5 hours · 21% · reset "), "{}", en.rows[1].text);
    assert_eq!(en.rows[4].text, "Month · 27% · 271/1000 credits · reset Nov 1");
    for r in &t.rows {
        println!("{:?} {}", r.icon, r.text);
    }
}

#[test]
fn stale_windows_and_old_kiro_usage_are_marked_stale_and_keep_notes() {
    let st = state(Some(LimitsFile { updated_at: 0, five_hour: Some(Window { used_percentage: 78.0, resets_at: 4_102_444_800 }), seven_day: None }), None, 7_200);
    let credit = CreditMeter { used: 100.0, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), fetched_at: Some(7_200 - 25 * 60), forecast: None };
    let t = render_tray(&sources(), &st, &snap(Some(credit), vec![]), 7_200, Lang::Vi);
    assert!(t.rows[1].text.starts_with("5 giờ · 78% · reset ") && t.rows[1].text.ends_with(" · cập nhật 2 giờ trước"), "{}", t.rows[1].text);
    assert_eq!(t.rows[1].icon, ring(Some(78.0), true));
    assert_eq!(t.rows[2].text, "7 ngày · chưa có dữ liệu");
    assert_eq!(t.rows[2].icon, ring(None, false));
    assert_eq!(t.rows[4].text, "Tháng · 10% · 100/1000 credit · reset 01/11 · cập nhật 25 phút trước");
    assert_eq!(t.rows[4].icon, ring(Some(10.0), true));
    assert_eq!(t.title.as_deref(), Some("78%"));
}

#[test]
fn tool_without_percent_gets_one_today_line_without_icon_and_no_title() {
    let st = state(None, None, 0);
    let t = render_tray(&sources(), &st, &snap(None, vec![act("kiro", 9_000, 12.34, 54)]), 0, Lang::Vi);
    assert_eq!(t.rows[4], TrayRow { tool: "kiro".into(), text: "Kiro · hôm nay 12.3 credit · 54 lượt".into(), icon: TrayIcon::None });
    // Claude Code has no limits.json yet: no % → no title.
    assert_eq!(t.title, None);
    let t = render_tray(&[src("other", "Other", false, false)], &st, &snap(None, vec![]), 0, Lang::En);
    assert_eq!(t.rows[1].text, "Other · today 0 tokens · 0 turns");
}

#[test]
fn title_is_highest_percent_rounded_including_over_limit() {
    let st = state(Some(LimitsFile { updated_at: 0, five_hour: Some(Window { used_percentage: 12.6, resets_at: 4_102_444_800 }), seven_day: None }), None, 0);
    let t = render_tray(&sources(), &st, &snap(None, vec![]), 0, Lang::Vi);
    assert_eq!(t.title.as_deref(), Some("13%"));
    let credit = CreditMeter { used: 1234.5, limit: 1000.0, next_reset: NaiveDate::from_ymd_opt(2026, 11, 15).unwrap(), fetched_at: None, forecast: None };
    let t = render_tray(&sources(), &st, &snap(Some(credit), vec![]), 0, Lang::Vi);
    assert_eq!(t.title.as_deref(), Some("123%"));
}

#[test]
fn ring_key_quantizes_and_picks_tone_by_threshold_and_staleness() {
    assert_eq!(ring_key(Some(21.4), false, 70), (21, RingTone::Normal));
    assert_eq!(ring_key(Some(69.6), false, 70), (70, RingTone::Normal), "compares the threshold against the real number");
    assert_eq!(ring_key(Some(70.0), false, 70), (70, RingTone::Alert));
    assert_eq!(ring_key(Some(86.0), false, 70), (86, RingTone::Alert));
    assert_eq!(ring_key(Some(150.0), false, 70), (100, RingTone::Alert));
    assert_eq!(ring_key(Some(-3.0), false, 70), (0, RingTone::Normal));
    assert_eq!(ring_key(Some(f64::NAN), false, 70), (0, RingTone::Normal));
    assert_eq!(ring_key(Some(86.0), true, 70), (86, RingTone::Stale));
    assert_eq!(ring_key(None, false, 70), (0, RingTone::Stale));
}
