//! Host loop: batch file events, read logs, call handlers (spec sections 5, 14.3).
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::engine::{Engine, IngestReport};

/// Batch events that arrive close together to read them once.
const DEBOUNCE: Duration = Duration::from_millis(30);
const TICK: Duration = Duration::from_secs(1);

pub enum HostMsg {
    FsChanged(Vec<PathBuf>),
    Shutdown,
}

pub trait HostHandler {
    fn on_ingest(&mut self, _engine: &mut Engine, _report: &IngestReport) {}
    fn on_limits(&mut self, _engine: &mut Engine) {}
    fn on_tick(&mut self, _engine: &mut Engine) {}
}

pub fn run_host(engine: &mut Engine, rx: &Receiver<HostMsg>, limits_file: &Path, handler: &mut dyn HostHandler, rescan_every: Duration) {
    let mut last_rescan = Instant::now();
    loop {
        match rx.recv_timeout(TICK) {
            Ok(HostMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(HostMsg::FsChanged(first)) => {
                let mut changed: BTreeSet<PathBuf> = first.into_iter().collect();
                let until = Instant::now() + DEBOUNCE;
                while let Some(left) = until.checked_duration_since(Instant::now()) {
                    match rx.recv_timeout(left) {
                        Ok(HostMsg::FsChanged(more)) => changed.extend(more),
                        Ok(HostMsg::Shutdown) => return,
                        Err(_) => break,
                    }
                }
                let mut report = IngestReport::default();
                let mut limits_changed = false;
                for p in changed {
                    if p == limits_file {
                        limits_changed = true;
                    } else {
                        match engine.ingest_path(&p) {
                            Ok(r) => report.merge(r),
                            Err(e) => eprintln!("bumeter: failed to read {}: {e}", p.display()),
                        }
                    }
                }
                if report.lines > 0 {
                    handler.on_ingest(engine, &report);
                }
                if limits_changed {
                    handler.on_limits(engine);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        handler.on_tick(engine);
        // Reconciliation scan to catch missed events (machine sleep, inotify overflow).
        if last_rescan.elapsed() >= rescan_every {
            last_rescan = Instant::now();
            match engine.full_scan() {
                Ok(r) if r.lines > 0 => handler.on_ingest(engine, &r),
                Ok(_) => {}
                Err(e) => eprintln!("bumeter: scan failed: {e}"),
            }
        }
    }
}
