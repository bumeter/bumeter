//! Icon và menu tray (spec mục 7.2).
use chrono::{DateTime, SecondsFormat, Utc};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tk_core::handlers::TodaySummary;
use tk_core::i18n::Lang;
use tk_core::insight::fmt_tokens;
use tk_core::config::Config;
use tk_core::face::{face_for, Face, FaceInput};
use tk_core::handlers::local_day_bounds;
use tk_core::limits::{LimitState, Window};
use tk_core::store::Store;

pub struct TrayItems {
    five: MenuItem<Wry>,
    seven: MenuItem<Wry>,
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
        minutes_since_anomaly: minutes_since(store.latest_alert_ts("anomaly:").ok().flatten(), now),
        today_tokens,
        avg_daily_tokens,
    }
}

fn ago(secs: i64, lang: Lang) -> String {
    let (n, vi, en) = match secs.max(0) {
        s if s < 60 => (s, "giây", "s"),
        s if s < 3600 => (s / 60, "phút", "min"),
        s if s < 86_400 => (s / 3600, "giờ", "h"),
        s => (s / 86_400, "ngày", "d"),
    };
    match lang {
        Lang::Vi => format!("{n} {vi} trước"),
        Lang::En => format!("{n}{en} ago"),
    }
}

fn window_label(name: &str, w: Option<Window>, limits: &LimitState, now: i64, lang: Lang) -> String {
    match w {
        Some(w) => {
            let when = chrono::DateTime::from_timestamp(w.resets_at, 0)
                .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
                .unwrap_or_default();
            let freshness = match (limits.stale, limits.updated_at) {
                (true, Some(t)) => format!(" · {} {}", lang.pick("cập nhật", "updated"), ago(now - t, lang)),
                _ => String::new(),
            };
            format!("{name}: {:.0}% · reset {when}{freshness}", w.used_percentage)
        }
        None => format!("{name}: {}", lang.pick("chưa có dữ liệu", "no data yet")),
    }
}

pub fn labels(limits: &LimitState, today: &TodaySummary, now: i64, lang: Lang) -> [String; 3] {
    [
        window_label(lang.pick("5 giờ", "5 hours"), limits.five_hour, limits, now, lang),
        window_label(lang.pick("7 ngày", "7 days"), limits.seven_day, limits, now, lang),
        format!("{}: {} token · {} {}", lang.pick("Hôm nay", "Today"), fmt_tokens(today.total), today.prompts, lang.pick("lượt", "turns")),
    ]
}

/// Nhãn các mục cố định: mở dashboard, tạm dừng, thoát.
pub fn action_labels(lang: Lang) -> [&'static str; 3] {
    [lang.pick("Mở dashboard", "Open dashboard"), lang.pick("Tạm dừng thông báo 1 giờ", "Pause notifications for 1 hour"), lang.pick("Thoát", "Quit")]
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let five = MenuItem::with_id(app, "five", "5 giờ: …", false, None::<&str>)?;
    let seven = MenuItem::with_id(app, "seven", "7 ngày: …", false, None::<&str>)?;
    let today = MenuItem::with_id(app, "today", "Hôm nay: …", false, None::<&str>)?;
    let [open_l, pause_l, quit_l] = action_labels(app.state::<crate::state::AppState>().lang());
    let open = MenuItem::with_id(app, "open", open_l, true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", pause_l, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_l, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&five, &seven, &today, &sep1, &open, &pause, &quit])?;
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
    app.manage(TrayItems { five, seven, today, open, pause, quit });
    Ok(())
}

pub fn refresh(app: &AppHandle, store: &Store, limits: &LimitState, today: &TodaySummary) {
    let items = app.state::<TrayItems>();
    let lang = app.state::<crate::state::AppState>().lang();
    let [five, seven, today_label] = labels(limits, today, chrono::Utc::now().timestamp(), lang);
    let [open_l, pause_l, quit_l] = action_labels(lang);
    let _ = items.open.set_text(open_l);
    let _ = items.pause.set_text(pause_l);
    let _ = items.quit.set_text(quit_l);
    let _ = items.five.set_text(&five);
    let _ = items.seven.set_text(&seven);
    let _ = items.today.set_text(&today_label);
    if let Some(tray) = app.tray_by_id("main") {
        let now = Utc::now();
        let face = face_for(&face_input(store, &app.state::<crate::state::AppState>().cfg, limits, today.total, now));
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
    fn labels_mark_stale_data_and_missing_windows() {
        let st = tk_core::limits::state(
            Some(tk_core::limits::LimitsFile { updated_at: 0, five_hour: Some(Window { used_percentage: 78.0, resets_at: 4_102_444_800 }), seven_day: None }),
            None,
            10_000,
        );
        let today = TodaySummary { prompts: 1, usage: Default::default(), total: 2_100_000, cost_usd: 3.2, unpriced_models: vec![] };
        let [five, seven, t] = labels(&st, &today, 10_000, Lang::Vi);
        assert!(five.contains("78%") && five.contains("cập nhật 2 giờ trước"), "{five}");
        assert!(seven.contains("chưa có dữ liệu"), "{seven}");
        assert_eq!(t, "Hôm nay: 2.1M token · 1 lượt");
        let [five, seven, t] = labels(&st, &today, 10_000, Lang::En);
        assert!(five.starts_with("5 hours: 78%") && five.contains("updated 2h ago"), "{five}");
        assert!(seven.contains("no data yet"), "{seven}");
        assert_eq!(t, "Today: 2.1M token · 1 turns");
    }
}
