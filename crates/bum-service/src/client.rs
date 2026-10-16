use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

use interprocess::local_socket::{prelude::*, Stream};
use serde_json::Value;
use bum_core::paths::Paths;

use crate::protocol::{socket_name, Request, Response};

/// Default timeout for a command.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);

/// Call a command on the host. Fails immediately if there is no host, fails after `DEFAULT_TIMEOUT` if the host hangs.
pub fn call(paths: &Paths, method: &str, params: Value) -> io::Result<Value> {
    call_timeout(paths, method, params, DEFAULT_TIMEOUT)
}

/// Like `call` but with a custom timeout (statusline uses a short one).
pub fn call_timeout(paths: &Paths, method: &str, params: Value, timeout: Duration) -> io::Result<Value> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (paths, method) = (paths.clone(), method.to_string());
    // Synchronous sockets have no cross-platform read timeout; run on a separate thread and discard it on timeout.
    std::thread::spawn(move || {
        let _ = tx.send(call_blocking(&paths, &method, params));
    });
    rx.recv_timeout(timeout).unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, "host did not respond")))
}

fn call_blocking(paths: &Paths, method: &str, params: Value) -> io::Result<Value> {
    let conn = Stream::connect(socket_name(paths)?)?;
    let mut reader = BufReader::new(conn);
    let mut req = serde_json::to_string(&Request { id: 1, method: method.into(), params })?;
    req.push('\n');
    reader.get_mut().write_all(req.as_bytes())?;
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let resp: Response = serde_json::from_str(&line)?;
    match resp.error {
        Some(e) => Err(io::Error::other(e)),
        None => Ok(resp.result.unwrap_or(Value::Null)),
    }
}
