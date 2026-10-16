use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bum_adapters::claude_code::ClaudeCode;
use bum_core::engine::{Engine, IngestReport};
use bum_core::host::{run_host, HostHandler, HostMsg};
use bum_core::store::Store;
use bum_core::watcher::spawn_watcher;

enum Seen {
    Ingest(IngestReport),
    Limits,
}

struct Forward(mpsc::Sender<Seen>);

impl HostHandler for Forward {
    fn on_ingest(&mut self, _e: &mut Engine, r: &IngestReport) {
        let _ = self.0.send(Seen::Ingest(r.clone()));
    }
    fn on_limits(&mut self, _e: &mut Engine) {
        let _ = self.0.send(Seen::Limits);
    }
}

const NEW_PROMPT: &str = r#"{"type":"user","sessionId":"s1","promptId":"p9","isSidechain":false,"timestamp":"2026-09-11T05:00:00.000Z","message":{"role":"user","content":"câu hỏi mới"}}"#;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dst = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() { copy_dir(&e.path(), &dst) } else { std::fs::copy(e.path(), dst).unwrap(); }
    }
}

fn wait_for<F: Fn(&Seen) -> bool>(rx: &mpsc::Receiver<Seen>, pred: F) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(s) = rx.recv_timeout(Duration::from_millis(100)) {
            if pred(&s) {
                return Some(start.elapsed());
            }
        }
    }
    None
}

#[test]
fn appended_line_and_limits_change_reach_the_handler() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("projects");
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code"), &root);
    let limits = data.join("limits.json");

    let (tx, rx) = mpsc::channel::<HostMsg>();
    let (_watcher, _) = spawn_watcher(&[root.clone(), data.clone()], &limits, tx.clone()).unwrap();
    let (seen_tx, seen_rx) = mpsc::channel();
    let root_for_thread: PathBuf = root.clone();
    let limits_for_thread = limits.clone();
    let host = std::thread::spawn(move || {
        let mut engine = Engine::new(Store::open_in_memory().unwrap(), vec![Box::new(ClaudeCode::new(&root_for_thread))]);
        engine.full_scan().unwrap();
        run_host(&mut engine, &rx, &limits_for_thread, &mut Forward(seen_tx), Duration::from_secs(60));
    });
    std::thread::sleep(Duration::from_millis(200));

    let mut f = std::fs::OpenOptions::new().append(true).open(root.join("-home-u-proj/s1.jsonl")).unwrap();
    writeln!(f, "{NEW_PROMPT}").unwrap();
    let latency = wait_for(&seen_rx, |s| matches!(s, Seen::Ingest(r) if r.summary.touched_prompts.contains(&"p9".to_string())));
    println!("độ trễ ghi log → handler: {latency:?}");
    assert!(latency.is_some(), "không nhận được lần hỏi mới");

    std::fs::write(&limits, b"{}").unwrap();
    assert!(wait_for(&seen_rx, |s| matches!(s, Seen::Limits)).is_some(), "không nhận được thay đổi limits");

    tx.send(HostMsg::Shutdown).unwrap();
    host.join().unwrap();
}

#[cfg(unix)]
#[test]
fn a_directory_that_cannot_be_watched_does_not_stop_the_others() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad");
    let good = dir.path().join("good");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::create_dir_all(&good).unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&bad).is_ok() {
        return; // root
    }
    let (tx, rx) = mpsc::channel::<HostMsg>();
    let (_w, failed) = spawn_watcher(&[bad.clone(), good.clone()], &dir.path().join("limits.json"), tx).unwrap();
    assert_eq!(failed, vec![bad.clone()]);
    std::fs::write(good.join("a.jsonl"), b"x\n").unwrap();
    assert!(matches!(rx.recv_timeout(Duration::from_secs(2)), Ok(HostMsg::FsChanged(_))));
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
}
