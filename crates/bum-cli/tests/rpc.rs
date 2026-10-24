//! `bum rpc` end to end: a real `bum` process talking JSON lines, on the fixture logs.
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use serde_json::{json, Value};

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/claude_code")
}

fn command(args: &[&str], data: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_bum"));
    c.args(args)
        .env("BUM_DATA_DIR", data)
        .env("BUM_CLAUDE_DIR", fixture())
        .env("BUM_KIRO_DIR", "/nonexistent-kiro")
        .env("BUM_OPENCODE_DB", "/nonexistent-opencode/opencode.db");
    c
}

struct Rpc {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Value>,
    next: u64,
}

impl Rpc {
    fn start(data: &Path) -> Self {
        let mut child = command(&["rpc"], data).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    let _ = tx.send(v);
                }
            }
        });
        Rpc { child, stdin, lines, next: 0 }
    }

    /// Next line that matches, skipping the others; panics after `secs`.
    fn wait_for(&self, secs: u64, want: impl Fn(&Value) -> bool) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(secs);
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let v = self.lines.recv_timeout(left).expect("no matching line from bum rpc");
            if want(&v) {
                return v;
            }
        }
    }

    fn send_raw(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send_raw(&json!({ "id": id, "method": method, "params": params }).to_string());
        self.wait_for(30, |v| v["id"] == id)
    }

    fn host_role(&self) -> String {
        self.wait_for(30, |v| v["event"] == "host")["role"].as_str().unwrap().to_string()
    }

    /// Close stdin and wait for the process to exit.
    fn finish(mut self) -> bool {
        drop(self.stdin.take());
        for _ in 0..100 {
            if let Ok(Some(st)) = self.child.try_wait() {
                return st.success();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        false
    }
}

#[test]
fn without_a_host_it_becomes_one_and_answers() {
    let dir = tempfile::tempdir().unwrap();
    let mut rpc = Rpc::start(dir.path());
    assert_eq!(rpc.host_role(), "own");
    assert_eq!(rpc.call("ping", Value::Null)["result"], "pong");
    // The fixture was scanned: there are numbers to report.
    let e = rpc.call("effectiveness", json!({ "days": 3650 }));
    assert!(e["result"]["turns"].as_u64().unwrap() > 0, "{e}");
    // Methods the app answers from the tools' logs work too.
    let sources = rpc.call("sources", Value::Null);
    assert!(sources["result"].as_array().unwrap().iter().any(|s| s["info"]["id"] == "claude_code"), "{sources}");
    // The Skills and Settings screens work too.
    let count = rpc.call("skills:count", Value::Null);
    assert!(count["result"]["total"].is_u64(), "{count}");
    let settings = rpc.call("settings:get", Value::Null);
    assert!(settings["result"]["config"].is_object(), "{settings}");
    assert!(rpc.call("skills:nope", Value::Null)["error"].as_str().unwrap().contains("no such action"));
    // Errors are answers, not crashes.
    assert!(rpc.call("no_such_method", Value::Null)["error"].is_string());
    rpc.send_raw("not json");
    assert!(rpc.wait_for(10, |v| v.get("id") == Some(&Value::Null))["error"].as_str().unwrap().contains("bad request"));
    assert!(rpc.finish(), "bum rpc must exit cleanly when stdin closes");

    // It released the host lock on the way out: the next one becomes the host again.
    let rpc = Rpc::start(dir.path());
    assert_eq!(rpc.host_role(), "own");
    assert!(rpc.finish());
}

#[test]
fn with_a_host_it_forwards_and_takes_over_when_the_host_goes() {
    let dir = tempfile::tempdir().unwrap();
    let mut daemon = command(&["daemon"], dir.path()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    // Wait until the daemon serves its socket.
    let mut up = false;
    for _ in 0..200 {
        let out = command(&["status"], dir.path()).output().unwrap();
        if out.status.success() {
            up = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(up, "bum daemon did not start");

    let mut rpc = Rpc::start(dir.path());
    assert_eq!(rpc.host_role(), "remote");
    assert_eq!(rpc.call("ping", Value::Null)["result"], "pong");

    // The host goes away (like the app quitting): the next request makes `bum rpc` the host.
    daemon.kill().unwrap();
    let _ = daemon.wait();
    assert_eq!(rpc.call("ping", Value::Null)["result"], "pong");
    assert!(rpc.finish());
}
