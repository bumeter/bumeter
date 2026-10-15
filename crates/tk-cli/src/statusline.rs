//! `bum statusline`: không bao giờ thất bại, vì lỗi ở đây sẽ làm hỏng statusline của Claude Code.
use std::io::Read;

use tk_core::limits;
use tk_core::paths::Paths;

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
    let st = limits::state(limits::read(&paths.limits()), None, now);
    // Số token hôm nay chỉ có khi host đang chạy; không có thì bỏ qua.
    let today = tk_service::client::call(&paths, "today_summary", serde_json::Value::Null)
        .ok()
        .and_then(|v| v["total"].as_u64());
    let lang = tk_core::config::Config::load(&paths.config()).language;
    println!("{}", limits::render_line(&st, today, lang));
}
