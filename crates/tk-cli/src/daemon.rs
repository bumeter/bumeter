//! `bum daemon`: host không giao diện; cảnh báo in ra stderr.
use std::time::Instant;

use tk_core::alerts::{Alert, AlertEngine};
use tk_core::config::Config;
use tk_core::engine::{Engine, IngestReport};
use tk_core::handlers::{limit_state, HandlerCtx};
use tk_core::host::HostHandler;
use tk_core::paths::Paths;
use tk_core::pricing::Pricing;
use tk_service::host::{start_host, StartError};

struct LogAlerts {
    alerts: AlertEngine,
    cfg: Config,
    paths: Paths,
}

impl LogAlerts {
    fn print(list: Vec<Alert>, lang: tk_core::i18n::Lang) {
        for a in list {
            eprintln!("[{}] {}", a.title(lang), a.body(lang));
        }
    }
}

impl HostHandler for LogAlerts {
    fn on_ingest(&mut self, e: &mut Engine, r: &IngestReport) {
        if let Ok(list) = self.alerts.on_ingest(e.store_mut(), r, Instant::now(), chrono::Utc::now()) {
            Self::print(list, self.cfg.language);
        }
    }

    fn on_limits(&mut self, e: &mut Engine) {
        let pricing = Pricing::default_table();
        let st = limit_state(&HandlerCtx { store: e.store(), pricing: &pricing, cfg: &self.cfg, paths: &self.paths }, chrono::Utc::now());
        if let Ok(st) = st {
            if let Ok(list) = self.alerts.on_limits(e.store_mut(), &st) {
                Self::print(list, self.cfg.language);
            }
        }
    }

    fn on_tick(&mut self, e: &mut Engine) {
        if let Ok(list) = self.alerts.on_tick(e.store_mut(), Instant::now(), chrono::Utc::now()) {
            Self::print(list, self.cfg.language);
        }
    }
}

pub fn run(paths: Paths, cfg: Config) -> anyhow::Result<()> {
    let handler = LogAlerts { alerts: AlertEngine::new(cfg.clone(), chrono::Utc::now()), cfg: cfg.clone(), paths: paths.clone() };
    match start_host(paths, cfg, Box::new(handler)) {
        Ok(h) => {
            eprintln!("bum: host đang chạy (Ctrl+C để dừng)");
            h.wait();
            Ok(())
        }
        Err(StartError::AlreadyRunning) => anyhow::bail!("đã có host khác đang chạy (app hoặc bum daemon)"),
        Err(StartError::Other(e)) => Err(e),
    }
}
