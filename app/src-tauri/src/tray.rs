//! Icon và menu tray (spec mục 7.2).
use chrono::{DateTime, SecondsFormat, Utc};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tk_core::handlers::TodaySummary;
use tk_core::i18n::Lang;
use tk_core::meter::{self, MenuText, ToolQuota};
use tk_core::source::SourceInfo;
use tk_core::config::Config;
use tk_core::face::{face_for, Face, FaceInput};
use tk_core::handlers::local_day_bounds;
use tk_core::limits::LimitState;
#[cfg(test)]
use tk_core::limits::Window;
use tk_core::store::Store;

pub struct TrayItems {
    sources: Vec<SourceInfo>,
    /// Các dòng theo công cụ, `meter::line_count` dòng mỗi công cụ.
    tools: Vec<MenuItem<Wry>>,
    today: MenuItem<Wry>,
    open: MenuItem<Wry>,
    pause: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

const TRAY_PNGS: [(Face, &[u8]); 5] = [
    (Face::Happy, include_bytes!("../icons/tray/bum-happy.png")),
    (Face::Calm, include_bytes!("../icons/tray/bum-calm.png")),
    (Face::Worry, include_bytes!("../icons/tray/bum-worry.png")),
    (Face::Blaze, include_bytes!("../icons/tray/bum-blaze.png")),
    (Face::Sleep, include_bytes!("../icons/tray/bum-sleep.png")),
];

/// Icon tray 32×32 của một biểu cảm Bum.
pub fn face_image(face: Face) -> Image<'static> {
    let bytes = TRAY_PNGS.iter().find(|(f, _)| *f == face).map(|(_, b)| *b).expect("đủ 5 biểu cảm");
    Image::from_bytes(bytes).expect("PNG tray hợp lệ")
}

fn minutes_since(ts: Option<String>, now: DateTime<Utc>) -> Option<i64> {
    let t = DateTime::parse_from_rfc3339(&ts?).ok()?;
    Some((now - t.with_timezone(&Utc)).num_minutes().max(0))
}

/// Gom đầu vào chọn biểu cảm từ DB + trạng thái quota (spec 3.4). Lỗi truy vấn coi như "không có dữ liệu".
pub fn face_input(store: &Store, cfg: &Config, limits: &LimitState, today_tokens: u64, now: DateTime<Utc>) -> FaceInput {
    let thresholds = match cfg.quota_thresholds.as_slice() {
        [a, b, ..] => (*a, *b),
        _ => (70, 90),
    };
    let offset = chrono::Local::now().offset().local_minus_utc();
    let (today_from, _) = local_day_bounds(now);
    let since = DateTime::parse_from_rfc3339(&today_from).map(|t| (t.with_timezone(&Utc) - chrono::Duration::days(30)).to_rfc3339_opts(SecondsFormat::Millis, true));
    // Trung bình token/ngày trên các ngày có lượt hỏi, trong 30 ngày trước hôm nay.
    let avg_daily_tokens = since.ok().and_then(|since| {
        let days = store.active_days_between(&since, &today_from, offset).ok()?;
        let total = store.model_usage_between(&since, &today_from).ok()?.iter().map(|(_, u)| u.total()).sum::<u64>();
        (days > 0).then(|| total / days)
    });
    FaceInput {
        stale: limits.stale,
        minutes_since_last_turn: minutes_since(store.latest_prompt_ts().ok().flatten(), now),
        quota_pct: limits.five_hour.map(|w| w.used_percentage),
        thresholds,
        minutes_since_anomaly: minutes_since(store.latest_anomaly_ts(None).ok().flatten(), now),
        today_tokens,
        avg_daily_tokens,
    }
}

/// Text các dòng theo công cụ + dòng "Hôm nay" (tk_core::meter). Lỗi truy vấn → chỉ quota, không credit/hoạt động.
pub fn menu_text(store: &Store, cfg: &Config, quotas: &std::collections::BTreeMap<String, ToolQuota>, sources: &[SourceInfo], limits: &LimitState, now: DateTime<Utc>, lang: Lang) -> MenuText {
    let snap = meter::snapshot(store, cfg, quotas, sources, now).unwrap_or_default();
    meter::render(sources, limits, &snap, now.timestamp(), lang)
}

/// Nhãn các mục cố định: mở dashboard, tạm dừng, thoát.
pub fn action_labels(lang: Lang) -> [&'static str; 3] {
    [lang.pick("Mở dashboard", "Open dashboard"), lang.pick("Tạm dừng thông báo 1 giờ", "Pause notifications for 1 hour"), lang.pick("Thoát", "Quit")]
}

/// Danh sách công cụ từ registry (thứ tự hiển thị).
pub fn sources(cfg: &Config) -> Vec<SourceInfo> {
    tk_adapters::registry(&tk_service::host::claude_root(cfg), &tk_service::host::kiro_root(cfg)).into_iter().map(|(i, _)| i).collect()
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let st = app.state::<crate::state::AppState>();
    let sources = sources(&st.cfg);
    // Số dòng theo công cụ cố định từ registry; nội dung đặt lại ở `refresh`.
    let tools = (0..sources.iter().map(meter::line_count).sum::<usize>())
        .map(|i| MenuItem::with_id(app, format!("tool-{i}"), "…", false, None::<&str>))
        .collect::<tauri::Result<Vec<_>>>()?;
    let today = MenuItem::with_id(app, "today", "…", false, None::<&str>)?;
    let [open_l, pause_l, quit_l] = action_labels(st.lang());
    let open = MenuItem::with_id(app, "open", open_l, true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", pause_l, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_l, true, None::<&str>)?;
    let menu = Menu::new(app)?;
    for t in &tools {
        menu.append(t)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&today)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append_items(&[&open, &pause, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(face_image(Face::Calm))
        .tooltip("Bumeter")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| match ev.id.as_ref() {
            "open" => crate::window::open(app),
            "pause" => crate::state::pause_for(app, 3600),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    app.manage(TrayItems { sources, tools, today, open, pause, quit });
    Ok(())
}

pub fn refresh(app: &AppHandle, store: &Store, limits: &LimitState, today: &TodaySummary) {
    let items = app.state::<TrayItems>();
    let st = app.state::<crate::state::AppState>();
    let lang = st.lang();
    let now = Utc::now();
    // Đọc lại config/hạn mức mỗi lần: người dùng sửa ở Cài đặt thì tray theo ngay.
    let cfg = Config::load(&st.paths.config());
    let quotas = meter::read_quotas(&st.paths.tool_quotas());
    let text = menu_text(store, &cfg, &quotas, &items.sources, limits, now, lang);
    for (item, line) in items.tools.iter().zip(&text.tools) {
        let _ = item.set_text(line);
    }
    let _ = items.today.set_text(&text.today);
    let [open_l, pause_l, quit_l] = action_labels(lang);
    let _ = items.open.set_text(open_l);
    let _ = items.pause.set_text(pause_l);
    let _ = items.quit.set_text(quit_l);
    if let Some(tray) = app.tray_by_id("main") {
        let face = face_for(&face_input(store, &st.cfg, limits, today.total, now));
        let _ = tray.set_icon(Some(face_image(face)));
        let _ = tray.set_tooltip(Some(tk_core::limits::render_line(limits, Some(today.total), lang)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tray_png_decodes_to_32x32() {
        for (face, _) in TRAY_PNGS {
            let img = face_image(face);
            assert_eq!((img.width(), img.height()), (32, 32), "{face:?}");
        }
    }

    #[test]
    fn menu_lines_follow_registry_and_match_item_count() {
        let srcs = sources(&Config::default());
        assert_eq!(srcs.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["claude_code", "kiro"]);
        let store = Store::open_in_memory().unwrap();
        let st = tk_core::limits::state(
            Some(tk_core::limits::LimitsFile { updated_at: 0, five_hour: Some(Window { used_percentage: 47.0, resets_at: 4_102_444_800 }), seven_day: None }),
            None,
            0,
        );
        let mut cfg = Config::default();
        cfg.tool_limits.insert("kiro".into(), tk_core::config::ToolLimit { monthly_credits: Some(1000.0), reset_day: 1 });
        let now = Utc::now();
        let t = menu_text(&store, &cfg, &Default::default(), &srcs, &st, now, Lang::Vi);
        assert_eq!(t.tools.len(), srcs.iter().map(meter::line_count).sum::<usize>());
        assert!(t.tools[0].starts_with("Claude Code") && t.tools[0].contains("▰▰▰▰▰▱▱▱▱▱  47% · 5 giờ"), "{}", t.tools[0]);
        assert!(t.tools[1].contains("7 ngày · chưa có dữ liệu"), "{}", t.tools[1]);
        assert!(t.tools[2].starts_with("Kiro") && t.tools[2].contains("▱▱▱▱▱▱▱▱▱▱  0% · 0/1000 credit · reset "), "{}", t.tools[2]);
        assert_eq!(t.today, "Hôm nay: chưa có hoạt động");
        // /usage còn mới thì thắng hạn mức người dùng nhập.
        let mut q = std::collections::BTreeMap::new();
        q.insert("kiro".to_string(), ToolQuota { used: 270.69, limit: 1000.0, resets_on: chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), plan: "KIRO PRO".into(), fetched_at: now.timestamp() });
        let t = menu_text(&store, &cfg, &q, &srcs, &st, now, Lang::Vi);
        assert!(t.tools[2].contains("27% · 271/1000 credit · reset 01/11"), "{}", t.tools[2]);
        for l in t.tools.iter().chain([&t.today]) {
            println!("{l}");
        }
    }
}
