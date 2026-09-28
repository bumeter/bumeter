use std::path::PathBuf;

use tk_skills::catalog::{SkillEntry, SkillState, Source};
use tk_skills::report::{build, Usage};

fn entry(name: &str, source: Source, state: SkillState, always_on: u64) -> SkillEntry {
    SkillEntry { name: name.into(), description: String::new(), source, state, path: Some(PathBuf::from("/x")), symlink: false, installed_at: None, tool: "claude_code".into(), always_on_tokens: Some(always_on), on_invoke_tokens: Some(4000), estimated: true, managed: true }
}

fn usage(name: &str, uses: u64, last: &str) -> Usage {
    Usage { name: name.into(), uses, projects: 1, tokens: uses * 1000, avg_tokens: Some(1000), last_used: Some(last.into()) }
}

#[test]
fn joins_usage_and_flags_unused_enabled_skills() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap().to_utc();
    let entries = vec![
        entry("pj-review", Source::Personal, SkillState::Enabled, 90),
        entry("pj-debug", Source::Personal, SkillState::Enabled, 75),
        entry("pj-old", Source::Personal, SkillState::Enabled, 60),
        entry("pj-off", Source::Personal, SkillState::Disabled, 50),
        entry("superpowers:brainstorming", Source::Plugin { plugin: "superpowers@x".into() }, SkillState::Enabled, 70),
    ];
    let used = vec![
        usage("pj-review", 5, "2026-09-16T00:00:00Z"),
        usage("pj-old", 2, "2026-07-01T00:00:00Z"),
        usage("superpowers:brainstorming", 4, "2026-09-24T00:00:00Z"),
        usage("/model", 14, "2026-09-23T00:00:00Z"),
        usage("removed-skill", 1, "2026-09-01T00:00:00Z"),
    ];
    let r = build(entries, Some(used), vec!["cảnh báo".into()], now);

    let row = |n: &str| r.rows.iter().find(|x| x.entry.name == n).unwrap();
    assert_eq!(row("pj-review").uses, 5);
    assert!(!row("pj-review").unused_30d);
    assert!(row("pj-debug").unused_30d, "chưa dùng bao giờ");
    assert!(row("pj-old").unused_30d, "dùng lần cuối quá 30 ngày");
    assert!(!row("pj-off").unused_30d, "đã tắt thì không gợi ý tắt nữa");
    assert_eq!(row("superpowers:brainstorming").uses, 4);

    assert_eq!(r.total_always_on, 90 + 75 + 60 + 70, "chỉ tính skill đang bật");
    assert_eq!(r.unused, vec!["pj-debug".to_string(), "pj-old".to_string()]);
    assert_eq!(r.unused_always_on, 75 + 60);

    let cmds: Vec<_> = r.commands.iter().map(|c| c.entry.name.as_str()).collect();
    assert_eq!(cmds, vec!["/model", "removed-skill"]);
    assert!(r.commands.iter().all(|c| !c.entry.managed));
    assert_eq!(r.warnings, vec!["cảnh báo".to_string()]);
}

#[test]
fn slash_command_of_an_installed_skill_counts_toward_that_skill() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap().to_utc();
    let entries = vec![entry("superpowers:brainstorming", Source::Plugin { plugin: "superpowers@x".into() }, SkillState::Enabled, 70)];
    let used = vec![
        usage("superpowers:brainstorming", 3, "2026-09-20T00:00:00Z"),
        usage("/superpowers:brainstorming", 2, "2026-09-23T00:00:00Z"),
        usage("/model", 14, "2026-09-23T00:00:00Z"),
    ];
    let r = build(entries, Some(used), vec![], now);
    let b = &r.rows[0];
    assert_eq!(b.uses, 5);
    assert_eq!(b.last_used.as_deref(), Some("2026-09-23T00:00:00Z"));
    assert_eq!(r.commands.iter().map(|c| c.entry.name.as_str()).collect::<Vec<_>>(), vec!["/model"]);
}

#[test]
fn without_usage_data_nothing_is_flagged_unused() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap().to_utc();
    let r = build(vec![entry("pj-debug", Source::Personal, SkillState::Enabled, 75)], None, vec![], now);
    assert!(r.unused.is_empty() && !r.rows[0].unused_30d);
    assert!(r.warnings.iter().any(|w| w.contains("mức dùng")), "{:?}", r.warnings);
}

#[test]
fn recently_installed_skill_is_not_flagged() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap().to_utc();
    let mut fresh = entry("pj-new", Source::Personal, SkillState::Enabled, 75);
    fresh.installed_at = Some("2026-09-20T00:00:00Z".into());
    let r = build(vec![fresh], Some(vec![]), vec![], now);
    assert!(!r.rows[0].unused_30d);
}

#[test]
fn kiro_skills_do_not_borrow_claude_usage_and_are_never_flagged_unused() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-24T00:00:00Z").unwrap().to_utc();
    let mut k = entry("pj-review", Source::Personal, SkillState::Enabled, 90);
    k.tool = "kiro".into();
    let c = entry("pj-review", Source::Personal, SkillState::Enabled, 90);
    let r = build(vec![c, k], Some(vec![usage("pj-review", 5, "2026-09-20T00:00:00Z")]), vec![], now);
    let claude = r.rows.iter().find(|x| x.entry.tool == "claude_code").unwrap();
    let kiro = r.rows.iter().find(|x| x.entry.tool == "kiro").unwrap();
    assert_eq!(claude.uses, 5);
    assert_eq!(kiro.uses, 0);
    assert!(!kiro.usage_known && !kiro.unused_30d);
    assert_eq!(r.total_always_on, 90, "chi phí nền chỉ tính cho Claude Code");
}
