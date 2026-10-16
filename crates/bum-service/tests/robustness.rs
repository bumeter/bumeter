use std::time::{Duration, Instant};

use serde_json::Value;
use bum_core::config::Config;
use bum_core::host::HostHandler;
use bum_core::paths::Paths;
use bum_service::client;
use bum_service::host::{start_host_taking_over, StartError};
use bum_service::lock::HostLock;
use bum_service::protocol::socket_name;
use bum_service::server::listen;

struct Quiet;
impl HostHandler for Quiet {}

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code")
}

#[test]
fn client_gives_up_on_a_host_that_never_answers() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("d"));
    paths.ensure().unwrap();
    let _listener = listen(socket_name(&paths).unwrap()).unwrap(); // chấp nhận kết nối nhưng không bao giờ trả lời
    let start = Instant::now();
    assert!(client::call_timeout(&paths, "ping", Value::Null, Duration::from_millis(300)).is_err());
    assert!(start.elapsed() < Duration::from_secs(1), "{:?}", start.elapsed());
}

#[test]
fn takeover_times_out_cleanly_when_lock_holder_has_no_socket() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("d"));
    paths.ensure().unwrap();
    let _held = HostLock::try_acquire(&paths.lock()).unwrap().unwrap(); // giống `bum scan` đang chạy
    let cfg = Config { claude_projects_dir: Some(fixture()), kiro_sessions_dir: Some(std::path::PathBuf::from("/nonexistent-kiro")), ..Config::default() };
    let r = start_host_taking_over(&paths, &cfg, || Box::new(Quiet), Duration::from_millis(600));
    assert!(matches!(r, Err(StartError::AlreadyRunning)));
}

#[test]
fn takeover_replaces_a_running_host() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("d"));
    let cfg = Config { claude_projects_dir: Some(fixture()), kiro_sessions_dir: Some(std::path::PathBuf::from("/nonexistent-kiro")), ..Config::default() };
    let first = start_host_taking_over(&paths, &cfg, || Box::new(Quiet), Duration::from_secs(1)).unwrap();
    let second = start_host_taking_over(&paths, &cfg, || Box::new(Quiet), Duration::from_secs(5)).unwrap();
    assert!(first.wait_timeout(Duration::from_secs(3)), "host cũ phải tắt");
    assert_eq!(client::call(&paths, "ping", Value::Null).unwrap(), Value::from("pong"));
    second.shutdown();
    assert!(second.wait_timeout(Duration::from_secs(3)));
}
