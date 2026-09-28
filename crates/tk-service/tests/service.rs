use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};
use tk_core::config::Config;
use tk_core::host::HostHandler;
use tk_core::paths::Paths;
use tk_service::client;
use tk_service::host::{start_host, StartError};
use tk_service::lock::HostLock;

struct Quiet;
impl HostHandler for Quiet {}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code")
}

#[test]
fn second_lock_is_refused_until_first_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("host.lock");
    let first = HostLock::try_acquire(&p).unwrap();
    assert!(first.is_some());
    assert!(HostLock::try_acquire(&p).unwrap().is_none());
    drop(first);
    assert!(HostLock::try_acquire(&p).unwrap().is_some());
}

#[test]
fn host_serves_queries_and_shuts_down_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("data"));
    let cfg = Config { claude_projects_dir: Some(fixture()), kiro_sessions_dir: Some(std::path::PathBuf::from("/nonexistent-kiro")), ..Config::default() };
    let handle = start_host(paths.clone(), cfg.clone(), Box::new(Quiet)).unwrap();

    assert!(matches!(start_host(paths.clone(), cfg, Box::new(Quiet)), Err(StartError::AlreadyRunning)));

    let pong = client::call(&paths, "ping", Value::Null).unwrap();
    assert_eq!(pong, json!("pong"));

    let recent = client::call(&paths, "recent_prompts", json!({"limit": 5})).unwrap();
    assert_eq!(recent.as_array().unwrap().len(), 2);

    let limits = client::call(&paths, "limit_state", Value::Null).unwrap();
    assert!(limits.get("stale").is_some());

    assert!(client::call(&paths, "no_such_method", Value::Null).is_err());

    client::call(&paths, "shutdown", Value::Null).unwrap();
    assert!(handle.wait_timeout(Duration::from_secs(5)), "host không tắt");
    assert!(HostLock::try_acquire(&paths.lock()).unwrap().is_some(), "lock phải được nhả");
}

#[test]
fn client_without_host_errors_fast() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("data"));
    let start = std::time::Instant::now();
    assert!(client::call(&paths, "ping", Value::Null).is_err());
    assert!(start.elapsed() < Duration::from_millis(500));
}
