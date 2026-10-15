//! App làm host: nếu `tk daemon` đang chạy thì yêu cầu nó tắt rồi chiếm quyền (spec mục 3.2.1).
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tk_core::alerts::{Alert, AlertEngine};
use tk_core::coalesce::Coalescer;
use tk_core::engine::{Engine, IngestReport};
use tk_core::handlers::{limit_state, local_day_bounds, today_summary, HandlerCtx};
use tk_core::host::HostHandler;
use tk_service::host::{start_host_taking_over, HostHandle};

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

    /// Báo UI (nếu cửa sổ đang mở) có dữ liệu mới; gộp tối đa 1 lần / 250ms.
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
        self.emit_changes(); // phần còn lại của một đợt dồn dập
        self.ticks += 1;
        if self.ticks % 30 == 0 {
            self.refresh(e); // cập nhật "cũ" / giờ reset
        }
    }
}

/// Chạy nền (không chặn main thread): chiếm quyền host; thất bại thì chạy ở chế độ client,
/// tray vẫn cập nhật từ DB chỉ đọc (spec mục 3.2.1).
pub fn start(app: &AppHandle) {
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
                eprintln!("tk: không làm host được ({e}); chạy ở chế độ chỉ đọc");
                loop {
                    refresh_from_db(&app);
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }
    });
}

/// Cập nhật tray từ DB chỉ đọc (lúc khởi động và ở chế độ client).
fn refresh_from_db(app: &AppHandle) {
    let st = app.state::<AppState>();
    let Ok(store) = tk_core::store::Store::open_readonly(&st.paths.db()) else { return };
    let ctx = HandlerCtx { store: &store, pricing: &st.pricing, cfg: &st.cfg, paths: &st.paths };
    let now = chrono::Utc::now();
    let (from, to) = local_day_bounds(now);
    if let (Ok(limits), Ok(today)) = (limit_state(&ctx, now), today_summary(&store, &st.pricing, &from, &to)) {
        crate::tray::refresh(app, &store, &limits, &today);
    }
}
