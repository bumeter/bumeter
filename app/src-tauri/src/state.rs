use std::sync::atomic::{AtomicI64, Ordering};

use tauri::{AppHandle, Manager};
use bum_core::config::Config;
use bum_core::paths::Paths;
use bum_core::pricing::Pricing;

pub struct AppState {
    pub paths: Paths,
    pub cfg: Config,
    pub pricing: Pricing,
    pub paused_until: AtomicI64,
    /// Ngôn ngữ hiện tại; đổi được khi đang chạy (Cài đặt), khác với `cfg` chỉ nạp lúc mở app.
    lang: std::sync::RwLock<bum_core::i18n::Lang>,
}

impl AppState {
    pub fn load() -> Self {
        let paths = Paths::resolve();
        let _ = paths.ensure();
        let cfg = Config::load(&paths.config());
        let pricing = Pricing::load_or_init(&paths.pricing());
        let lang = std::sync::RwLock::new(cfg.language);
        Self { paths, cfg, pricing, paused_until: AtomicI64::new(0), lang }
    }

    pub fn lang(&self) -> bum_core::i18n::Lang {
        self.lang.read().map(|l| *l).unwrap_or_default()
    }

    pub fn set_lang(&self, lang: bum_core::i18n::Lang) {
        if let Ok(mut l) = self.lang.write() {
            *l = lang;
        }
    }

    pub fn paused(&self) -> bool {
        chrono::Utc::now().timestamp() < self.paused_until.load(Ordering::Relaxed)
    }
}

pub fn pause_for(app: &AppHandle, secs: i64) {
    app.state::<AppState>().paused_until.store(chrono::Utc::now().timestamp() + secs, Ordering::Relaxed);
}
