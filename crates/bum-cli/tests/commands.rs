use std::path::Path;
use std::process::Command;

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code")
}

fn bum(args: &[&str], data: &Path) -> (String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_bum"))
        .args(args)
        .env("BUM_DATA_DIR", data)
        .env("BUM_CLAUDE_DIR", fixture())
        .env("BUM_KIRO_DIR", "/nonexistent-kiro")
        .output()
        .unwrap();
    (format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)), out.status.success())
}

#[test]
fn audit_matches_independent_count_on_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let (out, ok) = bum(&["audit"], dir.path());
    assert!(ok, "{out}");
    assert!(out.contains("OK"), "{out}");
    assert!(out.contains("3549"), "{out}");
}

#[test]
fn scan_fills_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let (out, ok) = bum(&["scan"], dir.path());
    assert!(ok, "{out}");
    assert!(dir.path().join("bumeter.db").exists());
}

#[test]
fn status_without_host_explains_and_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (out, ok) = bum(&["status"], dir.path());
    assert!(!ok);
    assert!(out.contains("host"), "{out}");
}
