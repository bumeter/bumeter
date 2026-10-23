//! `bum rpc`: the host's JSON-RPC over stdin/stdout, for editor extensions.
//!
//! Each stdin line is a request `{"id", "method", "params"}`; each answer is one stdout line
//! `{"id", "result"}` or `{"id", "error"}`. Requests go to the running host (the app or `bum daemon`)
//! over its socket. When there is none, this process becomes the host itself, and then also writes
//! events without an id: `{"event":"data_changed"}` when new log data was read, `{"event":"alert",
//! "title", "body"}` for a notification. If the app takes the host over later (it asks the current one
//! to shut down), requests simply go to the app from then on.
//!
//! An extension does not talk to the socket directly because the Windows pipe name is derived with
//! Rust's hasher, and because it would then have to start a host itself.
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use bum_core::alerts::{Alert, AlertEngine};
use bum_core::config::Config;
use bum_core::engine::{Engine, IngestReport};
use bum_core::handlers::{limit_state, HandlerCtx};
use bum_core::host::HostHandler;
use bum_core::paths::Paths;
use bum_core::pricing::Pricing;
use bum_service::client;
use bum_service::host::{start_host, HostHandle, StartError};

/// How long one request may take on the host.
const TIMEOUT: Duration = Duration::from_secs(15);
/// How long to look for a running host at start.
const PROBE: Duration = Duration::from_millis(800);

/// Writes whole lines to stdout from any thread.
#[derive(Clone)]
struct Out(Arc<Mutex<()>>);

impl Out {
    fn line(&self, v: &Value) {
        let _guard = self.0.lock();
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{v}");
        let _ = out.flush();
    }
}

/// Host handler of a `bum rpc` that became the host: alerts and data changes go to stdout as events.
struct Events {
    out: Out,
    alerts: AlertEngine,
    cfg: Config,
    paths: Paths,
}

impl Events {
    fn alerts(&self, list: Vec<Alert>) {
        for a in list {
            self.out.line(&json!({ "event": "alert", "title": a.title(self.cfg.language), "body": a.body(self.cfg.language) }));
        }
    }
}

impl HostHandler for Events {
    fn on_ingest(&mut self, e: &mut Engine, r: &IngestReport) {
        if r.lines > 0 {
            self.out.line(&json!({ "event": "data_changed" }));
        }
        if let Ok(list) = self.alerts.on_ingest(e.store_mut(), r, Instant::now(), chrono::Utc::now()) {
            self.alerts(list);
        }
    }

    fn on_limits(&mut self, e: &mut Engine) {
        self.out.line(&json!({ "event": "data_changed" }));
        let pricing = Pricing::default_table();
        if let Ok(st) = limit_state(&HandlerCtx { store: e.store(), pricing: &pricing, cfg: &self.cfg, paths: &self.paths }, chrono::Utc::now()) {
            if let Ok(list) = self.alerts.on_limits(e.store_mut(), &st) {
                self.alerts(list);
            }
        }
    }

    fn on_tick(&mut self, e: &mut Engine) {
        if let Ok(list) = self.alerts.on_tick(e.store_mut(), Instant::now(), chrono::Utc::now()) {
            self.alerts(list);
        }
    }
}

struct Rpc {
    paths: Paths,
    cfg: Config,
    out: Out,
    own: Option<HostHandle>,
}

impl Rpc {
    fn announce(&self) {
        let role = if self.own.is_some() { "own" } else { "remote" };
        self.out.line(&json!({ "event": "host", "role": role, "version": env!("CARGO_PKG_VERSION") }));
    }

    /// Become the host if nobody is. A first scan of a large history takes a while, so say so first.
    fn become_host(&mut self) {
        if let Some(h) = self.own.take() {
            // Our host was asked to shut down (the app took over) or died: let it finish.
            h.shutdown();
            let _ = h.wait_timeout(Duration::from_secs(2));
        }
        self.out.line(&json!({ "event": "scanning" }));
        let handler = Events { out: self.out.clone(), alerts: AlertEngine::new(self.cfg.clone(), chrono::Utc::now()), cfg: self.cfg.clone(), paths: self.paths.clone() };
        match start_host(self.paths.clone(), self.cfg.clone(), Box::new(handler)) {
            Ok(h) => self.own = Some(h),
            Err(StartError::AlreadyRunning) => {}
            Err(StartError::Other(e)) => eprintln!("bum rpc: could not start the host: {e}"),
        }
        self.announce();
    }

    fn handle(&mut self, method: &str, params: Value) -> Result<Value, String> {
        // Methods that read the tools' own logs are answered here, from the database the host keeps.
        if let Ok(store) = bum_core::store::Store::open_readonly(&self.paths.db()) {
            if let Some(r) = bum_service::extra::call(&store, &self.cfg, method, &params) {
                return r;
            }
        }
        for attempt in 0..2 {
            match client::call_timeout(&self.paths, method, params.clone(), TIMEOUT) {
                Ok(v) => return Ok(v),
                // The host answered with an error (unknown method, bad params…).
                Err(e) if e.kind() == io::ErrorKind::Other => return Err(e.to_string()),
                Err(e) if e.kind() == io::ErrorKind::TimedOut => return Err(e.to_string()),
                // Nobody listening: no host yet, or the one we used went away.
                Err(e) if attempt == 1 => return Err(format!("no Bumeter host: {e}")),
                Err(_) => self.become_host(),
            }
        }
        unreachable!()
    }
}

pub fn run(paths: Paths, cfg: Config) -> anyhow::Result<()> {
    paths.ensure()?;
    let mut rpc = Rpc { paths, cfg, out: Out(Arc::new(Mutex::new(()))), own: None };
    if client::call_timeout(&rpc.paths, "ping", Value::Null, PROBE).is_ok() {
        rpc.announce();
    } else {
        rpc.become_host();
    }
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                rpc.out.line(&json!({ "id": Value::Null, "error": format!("bad request: {e}") }));
                continue;
            }
        };
        let id = req["id"].clone();
        let Some(method) = req["method"].as_str() else {
            rpc.out.line(&json!({ "id": id, "error": "missing 'method'" }));
            continue;
        };
        let answer = match rpc.handle(method, req["params"].clone()) {
            Ok(result) => json!({ "id": id, "result": result }),
            Err(error) => json!({ "id": id, "error": error }),
        };
        rpc.out.line(&answer);
    }
    // stdin closed: the editor went away. Stop our host, if any, so the lock is released.
    if let Some(h) = rpc.own.take() {
        h.shutdown();
        let _ = h.wait_timeout(Duration::from_secs(3));
    }
    Ok(())
}
