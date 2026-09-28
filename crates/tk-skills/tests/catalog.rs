use std::path::Path;

use tk_skills::catalog::{scan_personal, SkillState, Source};
use tk_skills::frontmatter::parse;

fn skill(root: &Path, dir: &str, body: &str) {
    let d = root.join(dir);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("SKILL.md"), body).unwrap();
}

#[test]
fn frontmatter_handles_quotes_folded_text_and_missing_fields() {
    let f = parse("---\nname: pj-review\ndescription: \"Review code: đối chiếu yêu cầu\"\n---\n# Body");
    assert_eq!(f.name.as_deref(), Some("pj-review"));
    assert_eq!(f.description.as_deref(), Some("Review code: đối chiếu yêu cầu"));
    let folded = parse("---\nname: x\ndescription: >\n  dòng một\n  dòng hai\nother: 1\n---\n");
    assert_eq!(folded.description.as_deref(), Some("dòng một dòng hai"));
    let none = parse("# không có frontmatter");
    assert!(none.name.is_none() && none.description.is_none());
}

#[test]
fn scans_enabled_disabled_synced_and_symlinks() {
    let home = tempfile::tempdir().unwrap();
    let claude = home.path().join(".claude");
    skill(&claude.join("skills"), "pj-review", "---\nname: pj-review\ndescription: Review code theo khung chuẩn\n---\nnội dung dài");
    skill(&claude.join("skills-disabled"), "pj-debug", "---\nname: pj-debug\ndescription: Debug có hệ thống\n---\n");
    skill(&claude.join("skills/synced/abc-uuid"), "docs", "---\nname: docs\ndescription: tài liệu\n---\n");
    std::fs::create_dir_all(claude.join("skills/empty-dir")).unwrap();
    let external = home.path().join("repo/pj-clarify");
    skill(&home.path().join("repo"), "pj-clarify", "---\nname: pj-clarify\ndescription: Sinh câu hỏi\n---\n");
    std::os::unix::fs::symlink(&external, claude.join("skills/pj-clarify")).unwrap();

    let mut all = scan_personal(&claude);
    all.sort_by(|a, b| a.name.cmp(&b.name));
    let names: Vec<_> = all.iter().map(|s| (s.name.as_str(), s.state, s.source.clone(), s.symlink, s.managed)).collect();
    assert_eq!(names, vec![
        ("docs", SkillState::Enabled, Source::Synced, false, false),
        ("pj-clarify", SkillState::Enabled, Source::Personal, true, true),
        ("pj-debug", SkillState::Disabled, Source::Personal, false, true),
        ("pj-review", SkillState::Enabled, Source::Personal, false, true),
    ]);
}

#[test]
fn estimates_token_costs_from_text_length() {
    let home = tempfile::tempdir().unwrap();
    let claude = home.path().join(".claude");
    let desc = "a".repeat(70);
    let body = format!("---\nname: s\ndescription: {desc}\n---\n{}", "b".repeat(700));
    skill(&claude.join("skills"), "s", &body);
    let s = &scan_personal(&claude)[0];
    assert_eq!(s.always_on_tokens, Some(20));
    assert_eq!(s.on_invoke_tokens, Some((body.chars().count() as f64 / 3.5).ceil() as u64));
    assert!(s.estimated);
}

#[test]
fn missing_skills_dir_is_empty_not_an_error() {
    let home = tempfile::tempdir().unwrap();
    assert!(scan_personal(&home.path().join(".claude")).is_empty());
}

#[test]
fn scan_for_another_tool_tags_entries() {
    let home = tempfile::tempdir().unwrap();
    skill(&home.path().join(".kiro/skills"), "pj", "---\nname: pj\ndescription: x\n---\n");
    let e = tk_skills::catalog::scan_personal_for(&home.path().join(".kiro"), "kiro");
    assert_eq!((e[0].name.as_str(), e[0].tool.as_str()), ("pj", "kiro"));
    assert_eq!(scan_personal(&home.path().join(".kiro"))[0].tool, "claude_code");
}
