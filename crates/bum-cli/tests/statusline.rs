use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], data_dir: &std::path::Path, stdin: &str) -> (String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bum"))
        .args(args)
        .env("TK_DATA_DIR", data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (String::from_utf8_lossy(&out.stdout).to_string(), out.status.success())
}

const INPUT: &str = r#"{"session_id":"s1","rate_limits":{"five_hour":{"used_percentage":78,"resets_at":4102444800},"seven_day":{"used_percentage":41,"resets_at":4102444800}}}"#;

#[test]
fn statusline_writes_limits_and_prints_line() {
    let dir = tempfile::tempdir().unwrap();
    let (out, ok) = run(&["statusline"], dir.path(), INPUT);
    assert!(ok);
    assert!(out.contains("5h 78%"), "{out}");
    assert!(dir.path().join("limits.json").exists());
}

#[test]
fn quiet_mode_prints_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (out, ok) = run(&["statusline", "--quiet"], dir.path(), INPUT);
    assert!(ok);
    assert!(out.is_empty(), "{out}");
    assert!(dir.path().join("limits.json").exists());
}

#[test]
fn garbage_input_never_fails_the_statusline() {
    let dir = tempfile::tempdir().unwrap();
    let (out, ok) = run(&["statusline"], dir.path(), "not json");
    assert!(ok, "statusline must not break Claude Code");
    assert!(out.contains("5h —"), "{out}");
}

#[test]
fn tee_mode_passes_input_through_unchanged_and_records_limits() {
    let dir = tempfile::tempdir().unwrap();
    let (out, ok) = run(&["statusline", "--tee"], dir.path(), INPUT);
    assert!(ok);
    assert_eq!(out, INPUT);
    assert!(dir.path().join("limits.json").exists());
}
