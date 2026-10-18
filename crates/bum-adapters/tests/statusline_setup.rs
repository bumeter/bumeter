#![cfg(unix)]
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use bum_adapters::claude_code::statusline::{Mode, Status, StatuslineSetup};

/// Fake `bum`: forwards stdin to stdout like `bum statusline --tee`.
fn fake_bin(dir: &Path, name: &str) -> String {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, "#!/bin/sh\ncat\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p.to_string_lossy().to_string()
}

fn setup_with(dir: &Path, bum: String) -> StatuslineSetup {
    StatuslineSetup::new(dir.join(".claude"), dir.join("backups"), bum)
}

fn setup(dir: &Path) -> StatuslineSetup {
    let bum = fake_bin(dir, "bin-a/bum");
    setup_with(dir, bum)
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
    assert!(command(dir.path()).ends_with("bum\" statusline"));
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
fn changed_bum_path_rehooks_from_the_original_and_keeps_first_backup() {
    let dir = tempfile::tempdir().unwrap();
    let original = r#"{"statusLine":{"type":"command","command":"bash sl.sh"}}"#;
    write_settings(dir.path(), original);
    let a = setup(dir.path());
    a.apply(&a.plan().unwrap()).unwrap();

    let b = setup_with(dir.path(), fake_bin(dir.path(), "bin-b/bum"));
    assert!(matches!(b.status().unwrap(), Status::Hooked { .. }), "still recognized as already hooked");
    let plan = b.plan().unwrap();
    assert!(!plan.is_noop(), "a changed bum path must be updated");
    b.apply(&plan).unwrap();
    let cmd = command(dir.path());
    assert!(cmd.contains("bin-b/bum") && !cmd.contains("bin-a/bum"), "{cmd}");
    assert_eq!(cmd.matches("--tee").count(), 1, "{cmd}");

    b.undo().unwrap();
    assert_eq!(std::fs::read_to_string(settings(dir.path())).unwrap(), original);
}

#[test]
fn refuses_when_bum_binary_does_not_exist() {
    let dir = tempfile::tempdir().unwrap();
    for bad in ["bum", "/nonexistent/bum"] {
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
fn missing_binary_error_names_bum_not_tk() {
    let t = tempfile::tempdir().unwrap();
    let s = setup_with(t.path(), "/nonexistent/bum".into());
    let msg = s.plan().unwrap_err().to_string();
    assert!(msg.contains("bum"), "{msg}");
    assert!(!msg.split(|c: char| !c.is_alphanumeric()).any(|w| w == "tk"), "{msg}");
}

#[test]
fn show_mode_runs_the_original_inside_bum_and_switching_keeps_the_first_backup() {
    let dir = tempfile::tempdir().unwrap();
    // A command with single quotes, like a PS1 converted by the statusline-setup agent.
    let original = r#"{"statusLine":{"type":"command","command":"printf '%s' \"$(cat)\" | tr a-z A-Z"}}"#;
    write_settings(dir.path(), original);
    let s = setup(dir.path());

    s.apply(&s.plan_for(Mode::Show).unwrap()).unwrap();
    let shown = command(dir.path());
    assert!(shown.contains("statusline --around '"), "{shown}");
    assert!(matches!(s.status().unwrap(), Status::Hooked { show: true, .. }));
    // The quoting round-trips: the shell hands `bum` exactly the original command.
    assert_eq!(run_sh(&format!("printf '%s' {}", &shown[shown.find("--around ").unwrap() + 9..]), ""), "printf '%s' \"$(cat)\" | tr a-z A-Z");
    assert!(s.plan_for(Mode::Show).unwrap().is_noop());

    // Switch to recording only: rebuilt from the original, not wrapped around the --around form.
    s.apply(&s.plan_for(Mode::Record).unwrap()).unwrap();
    let recorded = command(dir.path());
    assert!(recorded.ends_with("statusline --tee | { printf '%s' \"$(cat)\" | tr a-z A-Z; }"), "{recorded}");
    assert!(matches!(s.status().unwrap(), Status::Hooked { show: false, .. }));

    // And back, then undo: the file is exactly what it was before the first enable.
    s.apply(&s.plan_for(Mode::Show).unwrap()).unwrap();
    s.undo().unwrap();
    assert_eq!(std::fs::read_to_string(settings(dir.path())).unwrap(), original);
}

#[test]
fn a_hand_written_around_hook_is_unwrapped_not_lost() {
    // Set up by hand (no Bumeter backup), with a bare `bum` on PATH.
    let dir = tempfile::tempdir().unwrap();
    write_settings(dir.path(), r#"{"statusLine":{"type":"command","command":"bum statusline --around 'echo hi'\\''there'"}}"#);
    let s = setup(dir.path());
    s.apply(&s.plan_for(Mode::Show).unwrap()).unwrap();
    let cmd = command(dir.path());
    assert!(cmd.ends_with("statusline --around 'echo hi'\\''there'"), "{cmd}");
    assert!(cmd.starts_with('"'), "rewritten with the absolute bum path: {cmd}");
}

#[test]
fn without_a_statusline_both_modes_show_bums_own_line() {
    let dir = tempfile::tempdir().unwrap();
    write_settings(dir.path(), "{}");
    let s = setup(dir.path());
    s.apply(&s.plan_for(Mode::Record).unwrap()).unwrap();
    assert!(command(dir.path()).ends_with("\" statusline"));
    assert!(matches!(s.status().unwrap(), Status::Hooked { show: true, .. }));
    assert!(s.plan_for(Mode::Show).unwrap().is_noop());
}
