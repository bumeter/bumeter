//! Khởi động host: lock → quét lần đầu → watcher → service → vòng lặp (spec mục 3.2.1).
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tk_adapters::claude_code::ClaudeCode;
use tk_core::config::Config;
use tk_core::engine::Engine;
use tk_core::handlers::{dispatch, HandlerCtx};
use tk_core::host::{run_host, HostHandler, HostMsg};
use tk_core::paths::Paths;
use tk_core::pricing::Pricing;
use tk_core::store::Store;
use tk_core::watcher::spawn_watcher;

use crate::client;
use crate::lock::HostLock;
use crate::protocol::{socket_name, Response};
use crate::server::{listen, serve, Handler};

const RESCAN: Duration = Duration::from_secs(60);
const DEGRADED_RESCAN: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("đã có host khác đang chạy")]
    AlreadyRunning,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct HostHandle {
    tx: Sender<HostMsg>,
    done: mpsc::Receiver<()>,
    join: Option<JoinHandle<()>>,
}

impl HostHandle {
    pub fn shutdown(&self) {
        let _ = self.tx.send(HostMsg::Shutdown);
    }

    /// Chờ host dừng hẳn; `false` nếu quá thời gian.
    pub fn wait_timeout(mut self, timeout: Duration) -> bool {
        let stopped = self.done.recv_timeout(timeout).is_ok();
        if stopped {
            if let Some(j) = self.join.take() {
                let _ = j.join();
            }
        }
        stopped
    }

    pub fn wait(mut self) {
        let _ = self.done.recv();
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub fn kiro_root(cfg: &Config) -> std::path::PathBuf {
    cfg.kiro_sessions_dir.clone().unwrap_or_else(tk_adapters::kiro::Kiro::default_root)
}

pub fn claude_root(cfg: &Config) -> std::path::PathBuf {
    cfg.claude_projects_dir.clone().unwrap_or_else(ClaudeCode::default_root)
}

pub fn start_host(paths: Paths, cfg: Config, mut handler: Box<dyn HostHandler + Send>) -> Result<HostHandle, StartError> {
    paths.ensure().map_err(anyhow::Error::from)?;
    let lock = HostLock::try_acquire(&paths.lock()).map_err(anyhow::Error::from)?.ok_or(StartError::AlreadyRunning)?;

    let root = claude_root(&cfg);
    let mut store = Store::open(&paths.db()).map_err(anyhow::Error::from)?;
    // DB mới tạo lại: khôi phục ⭐/📝 từ bản sao lưu.
    let _ = tk_core::store::notes::import_notes(&mut store, &paths.notes());
    let mut engine = Engine::new(store, vec![Box::new(ClaudeCode::new(&root)), Box::new(tk_adapters::kiro::Kiro::new(kiro_root(&cfg)))]);
    let started = Instant::now();
    let report = engine.full_scan()?;
    eprintln!("bumeter: quét lần đầu {} dòng trong {:?}", report.lines, started.elapsed());

    let (tx, rx) = mpsc::channel::<HostMsg>();
    let mut dirs = engine.watch_roots();
    dirs.push(paths.data_dir.clone());
    let (watcher, unwatched) = spawn_watcher(&dirs, &paths.limits(), tx.clone()).map_err(anyhow::Error::from)?;
    // Không theo dõi được thì quét dày hơn để vẫn gần realtime.
    let rescan = if unwatched.is_empty() { RESCAN } else { DEGRADED_RESCAN };

    #[cfg(unix)]
    let _ = std::fs::remove_file(paths.socket()); // socket cũ sau crash; đã giữ lock nên an toàn
    let listener = listen(socket_name(&paths).map_err(anyhow::Error::from)?).map_err(anyhow::Error::from)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(paths.socket(), std::fs::Permissions::from_mode(0o600));
    }

    let stop = Arc::new(AtomicBool::new(false));
    let service_handler = request_handler(paths.clone(), cfg.clone(), tx.clone());
    let service_stop = stop.clone();
    let service = std::thread::spawn(move || serve(listener, service_handler, service_stop));

    let (done_tx, done_rx) = mpsc::channel();
    let limits = paths.limits();
    let join = std::thread::spawn(move || {
        let _lock = lock;
        let _watcher = watcher;
        run_host(&mut engine, &rx, &limits, handler.as_mut(), rescan);
        stop.store(true, Ordering::SeqCst);
        let _ = client::call_timeout(&paths, "ping", Value::Null, Duration::from_millis(300)); // đánh thức accept()
        let _ = service.join();
        drop(_lock);
        let _ = done_tx.send(());
    });
    Ok(HostHandle { tx, done: done_rx, join: Some(join) })
}

/// Khởi động host; nếu đã có host khác thì yêu cầu nó tắt (gửi lại mỗi lần thử) cho tới `timeout`.
/// Trả `AlreadyRunning` nếu bên giữ lock không chịu nhả (ví dụ `bum scan` không có socket).
pub fn start_host_taking_over(
    paths: &Paths,
    cfg: &Config,
    make_handler: impl Fn() -> Box<dyn HostHandler + Send>,
    timeout: Duration,
) -> Result<HostHandle, StartError> {
    let deadline = Instant::now() + timeout;
    loop {
        match start_host(paths.clone(), cfg.clone(), make_handler()) {
            Err(StartError::AlreadyRunning) if Instant::now() < deadline => {
                let _ = client::call_timeout(paths, "shutdown", Value::Null, Duration::from_millis(300));
                std::thread::sleep(Duration::from_millis(100));
            }
            other => return other,
        }
    }
}

fn request_handler(paths: Paths, cfg: Config, tx: Sender<HostMsg>) -> Handler {
    Arc::new(move |req| {
        if req.method == "shutdown" {
            let _ = tx.send(HostMsg::Shutdown);
            return Response::ok(req.id, json!(true));
        }
        let store = match Store::open_readonly(&paths.db()) {
            Ok(s) => s,
            Err(e) => return Response::err(req.id, e.to_string()),
        };
        let pricing = Pricing::load_or_init(&paths.pricing());
        let ctx = HandlerCtx { store: &store, pricing: &pricing, cfg: &cfg, paths: &paths };
        match dispatch(&ctx, &req.method, &req.params, chrono::Utc::now()) {
            Ok(v) => Response::ok(req.id, v),
            Err(e) => Response::err(req.id, e),
        }
    })
}
