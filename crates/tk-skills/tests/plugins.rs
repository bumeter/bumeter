#![cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tk_skills::catalog::{SkillState, Source};
use tk_skills::plugins::{parse_details, parse_tokens, scan_plugins, ClaudeCli};

const DETAILS: &str = "superpowers 6.4.1
  Description: x

Projected token cost
  Always-on:   ~840 tok   added to every session

Per-component (rounded)
  component                       always-on  on-invoke
  brainstorming                         ~70      ~6.3k
  writing-plans                         ~30      ~3.3k
  requesting-code-review                ~40        ~1k
  code-reviewer                        ~50        ~2k

  On-invoke cost is paid each time a skill or agent fires.
";

fn fake_claude(dir: &Path, install: &Path, enabled: bool) -> PathBuf {
    std::fs::write(dir.join("list.json"), format!(
        r#"[{{"id":"superpowers@claude-plugins-official","version":"6.4.1","scope":"user","enabled":{enabled},"installPath":"{}"}}]"#,
        install.display())).unwrap();
    std::fs::write(dir.join("details-superpowers.txt"), DETAILS).unwrap();
    let bin = dir.join("claude");
    std::fs::write(&bin, "#!/bin/sh\nD=$(dirname \"$0\")\nif [ \"$1 $2\" = \"plugin list\" ]; then cat \"$D/list.json\"; exit 0; fi\nif [ \"$1 $2\" = \"plugin details\" ]; then cat \"$D/details-$3.txt\"; exit 0; fi\necho \"$@\" >> \"$D/calls.log\"\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

#[test]
fn parses_approximate_token_counts() {
    assert_eq!(parse_tokens("~70"), Some(70));
    assert_eq!(parse_tokens("~6.3k"), Some(6300));
    assert_eq!(parse_tokens("~1k"), Some(1000));
    assert_eq!(parse_tokens("—"), None);
}

#[test]
fn parses_per_component_table() {
    let rows = parse_details(DETAILS);
    assert_eq!(rows, vec![
        ("brainstorming".to_string(), Some(70), Some(6300)),
        ("writing-plans".to_string(), Some(30), Some(3300)),
        ("requesting-code-review".to_string(), Some(40), Some(1000)),
        ("code-reviewer".to_string(), Some(50), Some(2000)),
    ]);
}

#[test]
fn plugin_skills_are_named_plugin_colon_skill_with_real_costs() {
    let dir = tempfile::tempdir().unwrap();
    let install = dir.path().join("install");
    for s in ["brainstorming", "writing-plans", "requesting-code-review"] {
        std::fs::create_dir_all(install.join("skills").join(s)).unwrap();
    }
    std::fs::write(install.join("skills/brainstorming/SKILL.md"), "---\nname: brainstorming\ndescription: Khám phá ý tưởng\n---\n").unwrap();
    let cli = ClaudeCli::new(fake_claude(dir.path(), &install, false));
    let (skills, warnings) = scan_plugins(&cli);
    assert!(warnings.is_empty(), "{warnings:?}");
    let b = skills.iter().find(|s| s.name == "superpowers:brainstorming").unwrap();
    assert_eq!(b.source, Source::Plugin { plugin: "superpowers@claude-plugins-official".into() });
    assert_eq!(b.state, SkillState::Disabled);
    assert_eq!((b.always_on_tokens, b.on_invoke_tokens, b.estimated), (Some(70), Some(6300), false));
    assert_eq!(b.description, "Khám phá ý tưởng");
    assert_eq!(skills.len(), 3, "agent `code-reviewer` không phải skill");
}

#[test]
fn missing_cli_gives_a_warning_not_an_error() {
    let cli = ClaudeCli::new(PathBuf::from("/nonexistent/claude"));
    let (skills, warnings) = scan_plugins(&cli);
    assert!(skills.is_empty());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn garbage_output_gives_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("claude");
    std::fs::write(&bin, "#!/bin/sh\necho 'not json'\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (skills, warnings) = scan_plugins(&ClaudeCli::new(bin));
    assert!(skills.is_empty());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn hanging_cli_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("claude");
    std::fs::write(&bin, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let start = std::time::Instant::now();
    let (_, warnings) = scan_plugins(&ClaudeCli::new(bin).with_timeout(std::time::Duration::from_millis(500)));
    assert_eq!(warnings.len(), 1);
    assert!(start.elapsed() < std::time::Duration::from_secs(3));
}

#[test]
fn large_output_does_not_deadlock() {
    let dir = tempfile::tempdir().unwrap();
    let big = format!("[{}]", (0..3000).map(|i| format!(r#"{{"id":"p{i}@m","version":"1","enabled":true}}"#)).collect::<Vec<_>>().join(","));
    std::fs::write(dir.path().join("big.json"), &big).unwrap();
    let bin = dir.path().join("claude");
    std::fs::write(&bin, "#!/bin/sh\ncat \"$(dirname \"$0\")/big.json\"\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cli = ClaudeCli::new(bin).with_timeout(std::time::Duration::from_secs(3));
    assert_eq!(tk_skills::plugins::list_plugins(&cli).unwrap().len(), 3000);
}

#[test]
fn plugin_details_run_in_parallel() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("list.json"), r#"[{"id":"a@m","enabled":true},{"id":"b@m","enabled":true},{"id":"c@m","enabled":true}]"#).unwrap();
    let bin = dir.path().join("claude");
    std::fs::write(&bin, "#!/bin/sh\nD=$(dirname \"$0\")\nif [ \"$2\" = list ]; then cat \"$D/list.json\"; else sleep 0.4; echo '  component  always-on  on-invoke'; echo \"  s-$3  ~10  ~1k\"; fi\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let start = std::time::Instant::now();
    let (skills, w) = scan_plugins(&ClaudeCli::new(bin));
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(skills.len(), 3);
    assert!(start.elapsed() < std::time::Duration::from_millis(1000), "{:?}", start.elapsed());
}
