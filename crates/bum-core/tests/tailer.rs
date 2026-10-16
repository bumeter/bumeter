use std::io::Write;

use bum_core::tailer::read_complete_lines;

#[test]
fn stops_before_a_partially_written_line() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.jsonl");
    std::fs::write(&p, b"one\ntwo\nthr").unwrap();
    let (lines, off) = read_complete_lines(&p, 0).unwrap();
    assert_eq!(lines, vec!["one", "two"]);
    assert_eq!(off, 8);

    let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
    f.write_all(b"ee\n").unwrap();
    let (lines, off) = read_complete_lines(&p, off).unwrap();
    assert_eq!(lines, vec!["three"]);
    assert_eq!(off, 14);
}

#[test]
fn nothing_new_returns_same_offset() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.jsonl");
    std::fs::write(&p, b"one\n").unwrap();
    assert_eq!(read_complete_lines(&p, 4).unwrap(), (vec![], 4));
}

#[test]
fn strips_carriage_returns() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.jsonl");
    std::fs::write(&p, b"one\r\n").unwrap();
    assert_eq!(read_complete_lines(&p, 0).unwrap().0, vec!["one"]);
}
