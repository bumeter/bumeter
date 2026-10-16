//! The app acts as host: if `bum daemon` is running, ask it to shut down and take over (spec section 3.2.1).
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use bum_core::alerts::{Alert, AlertEngine};
use bum_core::coalesce::Coalescer;
use bum_core::engine::{Engine, IngestReport};
use bum_core::handlers::{limit_state, local_day_bounds, today_summary, HandlerCtx};
use bum_core::host::HostHandler;
use bum_service::host::{start_host_taking_over, HostHandle};

use crate::state::AppState;

pub struct HostSlot(pub Mutex<Option<HostHandle>>);

struct TauriHandler {
    app: AppHandle,
    alerts: AlertEngine,
    changes: Coalescer,
    ticks: u32,
}

impl TauriHandler {
    fn notify(&self, list: Vec<Alert>) {
        if self.app.state::<AppState>().paused() {
            return;
        }
        let lang = self.app.state::<AppState>().lang();
        for a in list {
            let _ = self.app.notification().builder().title(format!("Bumeter · {}", a.title(lang))).body(a.body(lang)).show();
        }
    }

    /// Tell the UI (if the window is open) that there's new data; coalesced to at most once per 250ms.
    fn emit_changes(&mut self) {
        if let Some(change) = self.changes.take(Instant::now()) {
            let _ = self.app.emit("data-changed", change);
        }
    }

    fn refresh(&self, e: &Engine) {
        let st = self.app.state::<AppState>();
        let ctx = HandlerCtx { store: e.store(), pricing: &st.pricing, cfg: &st.cfg, paths: &st.paths };
        let now = chrono::Utc::now();
        let (from, to) = local_day_bounds(now);
        if let (Ok(limits), Ok(today)) = (limit_state(&ctx, now), today_summary(e.store(), &st.pricing, &from, &to)) {
            crate::tray::refresh(&self.app, e.store(), &limits, &today);
        }
    }
}

impl HostHandler for TauriHandler {
    fn on_ingest(&mut self, e: &mut Engine, r: &IngestReport) {
        if let Ok(list) = self.alerts.on_ingest(e.store_mut(), r, Instant::now(), chrono::Utc::now()) {
            self.notify(list);
        }
        self.changes.add(&r.summary.touched_prompts, &r.summary.touched_sessions);
        self.emit_changes();
        self.refresh(e);
    }

    fn on_limits(&mut self, e: &mut Engine) {
        let st = self.app.state::<AppState>();
        let limits = limit_state(&HandlerCtx { store: e.store(), pricing: &st.pricing, cfg: &st.cfg, paths: &st.paths }, chrono::Utc::now());
        if let Ok(limits) = limits {
            if let Ok(list) = self.alerts.on_limits(e.store_mut(), &limits) {
                self.notify(list);
            }
            let _ = self.app.emit("limits-changed", &limits);
        }
        self.refresh(e);
    }

    fn on_tick(&mut self, e: &mut Engine) {
        if let Ok(list) = self.alerts.on_tick(e.store_mut(), Instant::now(), chrono::Utc::now()) {
            self.notify(list);
        }
        self.emit_changes(); // the rest of a burst
        self.ticks += 1;
        if self.ticks % 30 == 0 {
            self.refresh(e); // update "stale" / reset time
        }
    }
}

/// Runs in the background (without blocking the main thread): take over as host; on failure, run in client mode,
/// where the tray still updates from the read-only DB (spec section 3.2.1).
pub fn start(app: &AppHandle) {
    poll_quotas(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let (paths, cfg) = (st.paths.clone(), st.cfg.clone());
        let make = || -> Box<dyn HostHandler + Send> {
            Box::new(TauriHandler {
                app: app.clone(),
                alerts: AlertEngine::new(cfg.clone(), chrono::Utc::now()),
                changes: Coalescer::new(Duration::from_millis(250)),
                ticks: 0,
            })
        };
        match start_host_taking_over(&paths, &cfg, make, Duration::from_secs(5)) {
            Ok(handle) => {
                app.manage(HostSlot(Mutex::new(Some(handle))));
                refresh_from_db(&app);
            }
            Err(e) => {
                eprintln!("bumeter: could not become host ({e}); running in read-only mode");
                loop {
                    refresh_from_db(&app);
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }
    });
}

/// Update the tray from the read-only DB (at startup and in client mode).
fn refresh_from_db(app: &AppHandle) {
    let st = app.state::<AppState>();
    let Ok(store) = bum_core::store::Store::open_readonly(&st.paths.db()) else { return };
    let ctx = HandlerCtx { store: &store, pricing: &st.pricing, cfg: &st.cfg, paths: &st.paths };
    let now = chrono::Utc::now();
    let (from, to) = local_day_bounds(now);
    if let (Ok(limits), Ok(today)) = (limit_state(&ctx, now), today_summary(&store, &st.pricing, &from, &to)) {
        crate::tray::refresh(app, &store, &limits, &today);
    }
}

/// Tool self-reported quotas (Kiro `/usage`): fetched at startup and then every 15 minutes, on a separate thread so it doesn't block the tray.
fn poll_quotas(app: &AppHandle) {
    const EVERY: Duration = Duration::from_secs(15 * 60);
    let app = app.clone();
    std::thread::spawn(move || loop {
        let path = app.state::<AppState>().paths.tool_quotas();
        let fresh = bum_adapters::fetch_quotas(chrono::Utc::now().timestamp());
        if matches!(bum_adapters::store_quotas(&path, fresh), Ok(true)) {
            refresh_from_db(&app);
        }
        std::thread::sleep(EVERY);
    });
}
