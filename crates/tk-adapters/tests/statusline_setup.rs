#![cfg(unix)]
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use tk_adapters::claude_code::statusline::{Status, StatuslineSetup};

/// `tk` giả: chuyển stdin ra stdout như `tk statusline --tee`.
fn fake_tk(dir: &Path, name: &str) -> String {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, "#!/bin/sh\ncat\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p.to_string_lossy().to_string()
}

fn setup_with(dir: &Path, tk: String) -> StatuslineSetup {
    StatuslineSetup::new(dir.join(".claude"), dir.join("backups"), tk)
}

fn setup(dir: &Path) -> StatuslineSetup {
    let tk = fake_tk(dir, "bin-a/tk");
    setup_with(dir, tk)
}

fn settings(dir: &Path) -> PathBuf {
    dir.join(".claude/settings.json")
}

fn write_settings(dir: &Path, content: &str) {
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    std::fs::write(settings(dir), content).unwrap();
}

fn read(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn command(dir: &Path) -> String {
    read(&settings(dir))["statusLine"]["command"].as_str().unwrap().to_string()
}

fn run_sh(cmd: &str, input: &str) -> String {
    let mut child = Command::new("sh").arg("-c").arg(cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}

#[test]
fn no_statusline_gets_our_command_and_undo_restores_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    let original = "{\n  \"theme\": \"dark\",\n  \"model\": \"opus\"\n}\n";
    write_settings(dir.path(), original);
    assert_eq!(s.status().unwrap(), Status::NotConfigured);
    s.apply(&s.plan().unwrap()).unwrap();
    assert!(command(dir.path()).ends_with("tk\" statusline"));
    assert_eq!(read(&settings(dir.path()))["theme"], "dark");
    assert!(matches!(s.status().unwrap(), Status::Hooked { .. }));
    s.undo().unwrap();
    assert_eq!(std::fs::read_to_string(settings(dir.path())).unwrap(), original);
}

#[test]
fn existing_command_is_grouped_and_wrapped_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    write_settings(dir.path(), r#"{"statusLine":{"type":"command","command":"bash ~/.claude/statusline-command.sh"}}"#);
    s.apply(&s.plan().unwrap()).unwrap();
    let hooked = command(dir.path());
    assert!(hooked.ends_with("statusline --tee | { bash ~/.claude/statusline-command.sh; }"), "{hooked}");
    let again = s.plan().unwrap();
    assert!(again.is_noop());
    s.apply(&again).unwrap();
    assert_eq!(command(dir.path()), hooked);
}

#[test]
fn compound_user_command_still_receives_stdin() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    write_settings(dir.path(), r#"{"statusLine":{"type":"command","command":"cd / && cat"}}"#);
    assert_eq!(run_sh("cd / && cat", "json-input"), "json-input");
    s.apply(&s.plan().unwrap()).unwrap();
    assert_eq!(run_sh(&command(dir.path()), "json-input"), "json-input", "{}", command(dir.path()));
}

#[test]
fn changed_tk_path_rehooks_from_the_original_and_keeps_first_backup() {
    let dir = tempfile::tempdir().unwrap();
    let original = r#"{"statusLine":{"type":"command","command":"bash sl.sh"}}"#;
    write_settings(dir.path(), original);
    let a = setup(dir.path());
    a.apply(&a.plan().unwrap()).unwrap();

    let b = setup_with(dir.path(), fake_tk(dir.path(), "bin-b/tk"));
    assert!(matches!(b.status().unwrap(), Status::Hooked { .. }), "vẫn nhận ra là đã hook");
    let plan = b.plan().unwrap();
    assert!(!plan.is_noop(), "đường dẫn tk đổi thì phải cập nhật");
    b.apply(&plan).unwrap();
    let cmd = command(dir.path());
    assert!(cmd.contains("bin-b/tk") && !cmd.contains("bin-a/tk"), "{cmd}");
    assert_eq!(cmd.matches("--tee").count(), 1, "{cmd}");

    b.undo().unwrap();
    assert_eq!(std::fs::read_to_string(settings(dir.path())).unwrap(), original);
}

#[test]
fn refuses_when_tk_binary_does_not_exist() {
    let dir = tempfile::tempdir().unwrap();
    for bad in ["tk", "/nonexistent/tk"] {
        let s = setup_with(dir.path(), bad.into());
        assert!(s.plan().is_err(), "{bad}");
    }
}

#[test]
fn undo_keeps_edits_made_after_enabling() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    write_settings(dir.path(), r#"{"statusLine":{"type":"command","command":"bash sl.sh"},"theme":"dark"}"#);
    s.apply(&s.plan().unwrap()).unwrap();
    let mut v = read(&settings(dir.path()));
    v["permissions"] = json!({"allow": ["Bash(ls)"]});
    std::fs::write(settings(dir.path()), serde_json::to_string_pretty(&v).unwrap()).unwrap();

    s.undo().unwrap();
    let after = read(&settings(dir.path()));
    assert_eq!(after["statusLine"]["command"], "bash sl.sh");
    assert_eq!(after["permissions"]["allow"][0], "Bash(ls)");
}

#[test]
fn undo_refuses_when_user_changed_the_statusline_since() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    write_settings(dir.path(), r#"{"statusLine":{"type":"command","command":"bash sl.sh"}}"#);
    s.apply(&s.plan().unwrap()).unwrap();
    write_settings(dir.path(), r#"{"statusLine":{"type":"command","command":"my-new-line"}}"#);
    assert!(s.undo().is_err());
    assert_eq!(command(dir.path()), "my-new-line");
}

#[test]
fn symlinked_settings_stays_a_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    let real = dir.path().join("dotfiles/settings.json");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::write(&real, "{}").unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::os::unix::fs::symlink(&real, settings(dir.path())).unwrap();
    s.apply(&s.plan().unwrap()).unwrap();
    assert!(std::fs::symlink_metadata(settings(dir.path())).unwrap().file_type().is_symlink());
    assert!(read(&real)["statusLine"]["command"].is_string());
}

#[test]
fn missing_settings_file_is_created_and_undo_removes_it() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    s.apply(&s.plan().unwrap()).unwrap();
    assert!(settings(dir.path()).exists());
    s.undo().unwrap();
    assert!(!settings(dir.path()).exists());
}

#[test]
fn invalid_settings_json_is_refused_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup(dir.path());
    write_settings(dir.path(), "{ not json");
    assert!(s.plan().is_err());
    assert_eq!(std::fs::read_to_string(settings(dir.path())).unwrap(), "{ not json");
}

#[test]
fn undo_without_backup_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(setup(dir.path()).undo().is_err());
}

#[test]
fn legacy_tk_hook_is_rewritten_to_bum_with_backup_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let bum = fake_tk(dir.path(), "bin-a/bum");
    let s = setup_with(dir.path(), bum);
    let old = format!("\"{}/bin-a/tk\" statusline --tee | {{ bash ~/.claude/sl.sh; }}", dir.path().display());
    write_settings(dir.path(), &json!({"model": "x", "statusLine": {"type": "command", "command": old}}).to_string());

    assert!(s.migrate_legacy_hook().unwrap());
    let now = command(dir.path());
    assert_eq!(now, format!("\"{}/bin-a/bum\" statusline --tee | {{ bash ~/.claude/sl.sh; }}", dir.path().display()));
    assert_eq!(read(&settings(dir.path()))["model"], "x");
    assert!(dir.path().join("backups/statusline-backup.json").exists(), "phải có bản sao lưu");

    let before = std::fs::read(settings(dir.path())).unwrap();
    assert!(!s.migrate_legacy_hook().unwrap());
    assert_eq!(std::fs::read(settings(dir.path())).unwrap(), before);
}

#[test]
fn legacy_unquoted_tk_path_is_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup_with(dir.path(), fake_tk(dir.path(), "bin-a/bum"));
    write_settings(dir.path(), &json!({"statusLine": {"type": "command", "command": "/usr/bin/tk statusline --tee"}}).to_string());
    assert!(s.migrate_legacy_hook().unwrap());
    assert_eq!(command(dir.path()), "/usr/bin/bum statusline --tee");
    assert!(!s.migrate_legacy_hook().unwrap());
}

#[test]
fn legacy_migration_ignores_custom_and_missing_settings() {
    let dir = tempfile::tempdir().unwrap();
    let s = setup_with(dir.path(), fake_tk(dir.path(), "bin-a/bum"));
    assert!(!s.migrate_legacy_hook().unwrap());
    write_settings(dir.path(), &json!({"statusLine": {"type": "command", "command": "bash ~/.claude/mytk.sh"}}).to_string());
    assert!(!s.migrate_legacy_hook().unwrap());
    assert_eq!(command(dir.path()), "bash ~/.claude/mytk.sh");
    assert!(!dir.path().join("backups").exists());
}
