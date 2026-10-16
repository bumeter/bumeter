use std::io::Write;
use std::path::{Path, PathBuf};

use bum_adapters::claude_code::ClaudeCode;
use bum_core::engine::Engine;
use bum_core::store::Store;

const LINE_D: &str = r#"{"type":"assistant","sessionId":"s1","timestamp":"2026-09-11T04:50:00.000Z","requestId":"req_D","message":{"id":"msg_D","model":"claude-opus-5","content":[{"type":"text","text":"d"}],"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5}}}"#;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() { copy_dir(&e.path(), &dst) } else { std::fs::copy(e.path(), dst).unwrap(); }
    }
}

fn setup() -> (tempfile::TempDir, PathBuf, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("projects");
    copy_dir(&fixture(), &root);
    let engine = Engine::new(Store::open_in_memory().unwrap(), vec![Box::new(ClaudeCode::new(&root))]);
    (dir, root, engine)
}

fn main_file(root: &Path) -> PathBuf {
    root.join("-home-u-proj/s1.jsonl")
}

#[test]
fn full_scan_ingests_main_and_subagent_files() {
    let (_d, _root, mut e) = setup();
    let report = e.full_scan().unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), 3549);
    assert_eq!(e.store().parse_errors().unwrap(), 1);
    assert_eq!(report.parse_errors, 1);
    assert!(report.summary.touched_prompts.contains(&"p1".to_string()));
}

#[test]
fn partial_line_is_read_only_once_complete() {
    let (_d, root, mut e) = setup();
    e.full_scan().unwrap();
    let path = main_file(&root);
    let (head, tail) = LINE_D.split_at(40);
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(head.as_bytes()).unwrap();
    e.ingest_path(&path).unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), 3549);

    f.write_all(tail.as_bytes()).unwrap();
    f.write_all(b"\n").unwrap();
    e.ingest_path(&path).unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), 3549 + 15);
    assert_eq!(e.store().parse_errors().unwrap(), 1, "a partially written line must not be counted as an error");
}

#[test]
fn truncated_file_is_purged_and_reread() {
    let (_d, root, mut e) = setup();
    e.full_scan().unwrap();
    let path = main_file(&root);
    let first3: String = std::fs::read_to_string(&path).unwrap().lines().take(4).map(|l| format!("{l}\n")).collect();
    std::fs::write(&path, first3).unwrap();
    e.ingest_path(&path).unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), 1152 + 60);
}

#[test]
fn replaced_file_with_same_content_is_not_double_counted() {
    let (_d, root, mut e) = setup();
    e.full_scan().unwrap();
    let path = main_file(&root);
    let content = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, &content).unwrap();
    e.ingest_path(&path).unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), 3549);
}

#[test]
fn rescan_without_changes_reads_nothing() {
    let (_d, _root, mut e) = setup();
    e.full_scan().unwrap();
    let again = e.full_scan().unwrap();
    assert_eq!(again.lines, 0);
    assert!(again.summary.touched_prompts.is_empty());
}

#[test]
fn deleted_file_is_ignored_and_keeps_data() {
    let (_d, root, mut e) = setup();
    e.full_scan().unwrap();
    let path = main_file(&root);
    std::fs::remove_file(&path).unwrap();
    e.ingest_path(&path).unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), 3549);
}

#[cfg(unix)]
#[test]
fn unreadable_file_does_not_stop_the_scan() {
    use std::os::unix::fs::PermissionsExt;
    let (_d, root, mut e) = setup();
    let bad = root.join("-a-first/x.jsonl"); // sorts before the fixture file
    std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
    std::fs::write(&bad, b"{}\n").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&bad).is_ok() {
        return; // running as root: cannot test this
    }
    let report = e.full_scan().unwrap();
    assert_eq!(e.store().usage_totals().unwrap().total(), 3549);
    assert_eq!(report.file_errors, 1);
}
