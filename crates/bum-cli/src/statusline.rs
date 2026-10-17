//! `bum statusline`: never fails, because an error here would break Claude Code's statusline.
use std::io::Read;

use bum_core::limits;
use bum_core::paths::Paths;

pub fn run(quiet: bool, tee: bool) {
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let now = chrono::Utc::now().timestamp();
    let paths = Paths::resolve();

    if let Some(file) = serde_json::from_str(&raw).ok().and_then(|v| limits::from_statusline(&v, now)) {
        if paths.ensure().is_ok() {
            let _ = limits::write_atomic(&paths.limits(), &file);
        }
    }
    if tee {
        use std::io::Write;
        let _ = std::io::stdout().write_all(raw.as_bytes());
        return;
    }
    if quiet {
        return;
    }
    let mut st = limits::state(limits::read(&paths.limits()), None, now);
    (st.forecast_five_hour, st.forecast_seven_day) = host_forecasts(&paths);
    // Today's token count is only available when the host is running; skip it otherwise.
    let today = bum_service::client::call(&paths, "today_summary", serde_json::Value::Null)
        .ok()
        .and_then(|v| v["total"].as_u64());
    let lang = bum_core::config::Config::load(&paths.config()).language;
    println!("{}", limits::render_line(&st, today, lang));
}

/// Forecasts from the running host (they need the reading history in the DB). No host → (None, None); never fails.
pub fn host_forecasts(paths: &Paths) -> (Option<bum_core::forecast::Forecast>, Option<bum_core::forecast::Forecast>) {
    let Ok(v) = bum_service::client::call(paths, "limit_state", serde_json::Value::Null) else { return (None, None) };
    let f = |k: &str| serde_json::from_value(v[k].clone()).ok();
    (f("forecast_five_hour"), f("forecast_seven_day"))
}
