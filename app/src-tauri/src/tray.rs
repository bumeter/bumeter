//! Icon và menu tray (spec mục 7.2).
use chrono::{DateTime, SecondsFormat, Utc};
use tauri::image::Image;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::menu::{IconMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tk_core::handlers::TodaySummary;
use tk_core::i18n::Lang;
use tk_core::meter::{self, RingTone, ToolQuota, TrayIcon, TrayText};
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
    /// Các dòng theo công cụ (tiêu đề + số liệu), `meter::tray_row_count` dòng mỗi công cụ.
    rows: Vec<IconMenuItem<Wry>>,
    /// Icon đang đặt của từng dòng: chỉ `set_icon` khi đổi.
    row_icons: Mutex<Vec<RowIcon>>,
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

/// Logo 32×32 theo tool id (tạo từ `app/ui/public/tool-icons/*.svg`).
const TOOL_PNGS: [(&str, &[u8]); 2] = [
    ("claude_code", include_bytes!("../icons/tools/claude_code.png")),
    ("kiro", include_bytes!("../icons/tools/kiro.png")),
];

/// Logo công cụ cho dòng tiêu đề; công cụ lạ → không icon.
pub fn tool_logo(tool: &str) -> Option<Image<'static>> {
    TOOL_PNGS.iter().find(|(id, _)| *id == tool).and_then(|(_, b)| Image::from_bytes(b).ok())
}

/// Icon của một dòng menu ở dạng so sánh được (vòng đã lượng tử hoá).
#[derive(Debug, Clone, Copy, PartialEq)]
enum RowIcon {
    Empty,
    Logo,
    Ring(u8, RingTone),
}

fn row_icon(icon: &TrayIcon, threshold: u8) -> RowIcon {
    match *icon {
        TrayIcon::None => RowIcon::Empty,
        TrayIcon::Logo => RowIcon::Logo,
        TrayIcon::Ring { pct, stale } => {
            let (p, tone) = meter::ring_key(pct, stale, threshold);
            RowIcon::Ring(p, tone)
        }
    }
}

/// Ảnh vòng % theo (pct, màu), cache để không vẽ lại.
fn ring_image(pct: u8, tone: RingTone) -> Image<'static> {
    static CACHE: OnceLock<Mutex<HashMap<(u8, RingTone), Image<'static>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry((pct, tone))
        .or_insert_with(|| {
            let n = meter::RING_SIZE as u32;
            Image::new_owned(meter::ring_rgba(pct, tone), n, n)
        })
        .clone()
}

fn icon_image(icon: RowIcon, tool: &str) -> Option<Image<'static>> {
    match icon {
        RowIcon::Empty => None,
        RowIcon::Logo => tool_logo(tool),
        RowIcon::Ring(p, tone) => Some(ring_image(p, tone)),
    }
}

/// Id mục menu của dòng thứ `i` thuộc công cụ `tool`.
fn row_id(tool: &str, i: usize) -> String {
    format!("tool:{tool}:{i}")
}

/// Công cụ của một mục menu dòng công cụ (`tool:<id>:<i>`), None với mục khác.
pub fn tool_of_menu_id(id: &str) -> Option<&str> {
    let rest = id.strip_prefix("tool:")?;
    rest.rsplit_once(':').map(|(tool, _)| tool).filter(|t| !t.is_empty())
}

/// Ngưỡng cảnh báo 1 (đổi màu vòng sang đỏ).
fn first_threshold(cfg: &Config) -> u8 {
    cfg.quota_thresholds.first().copied().unwrap_or(70)
}

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

/// Dòng theo công cụ + dòng "Hôm nay" + title (tk_core::meter). Lỗi truy vấn → chỉ quota, không credit/hoạt động.
pub fn menu_text(store: &Store, cfg: &Config, quotas: &std::collections::BTreeMap<String, ToolQuota>, sources: &[SourceInfo], limits: &LimitState, now: DateTime<Utc>, lang: Lang) -> TrayText {
    let snap = meter::snapshot(store, cfg, quotas, sources, now).unwrap_or_default();
    meter::render_tray(sources, limits, &snap, now.timestamp(), lang)
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
    // Số dòng theo công cụ cố định từ registry; nội dung đặt lại ở `refresh`. Để enabled: GNOME làm xám icon của mục disabled,
    // bấm vào thì mở app ở phạm vi công cụ đó.
    let mut rows = Vec::new();
    let mut row_icons = Vec::new();
    let mut seps = Vec::new();
    for s in &sources {
        for k in 0..meter::tray_row_count(s) {
            let (text, icon) = if k == 0 { (s.label.as_str(), RowIcon::Logo) } else { ("…", RowIcon::Empty) };
            rows.push(IconMenuItem::with_id(app, row_id(&s.id, rows.len()), text, true, icon_image(icon, &s.id), None::<&str>)?);
            row_icons.push(icon);
        }
        seps.push(rows.len());
    }
    let today = MenuItem::with_id(app, "today", "…", false, None::<&str>)?;
    let [open_l, pause_l, quit_l] = action_labels(st.lang());
    let open = MenuItem::with_id(app, "open", open_l, true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", pause_l, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_l, true, None::<&str>)?;
    let menu = Menu::new(app)?;
    for (i, r) in rows.iter().enumerate() {
        if i > 0 && seps.contains(&i) {
            menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
        menu.append(r)?;
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
            id => {
                if let Some(tool) = tool_of_menu_id(id) {
                    crate::window::open_scope(app, tool);
                }
            }
        })
        .build(app)?;
    app.manage(TrayItems { sources, rows, row_icons: Mutex::new(row_icons), today, open, pause, quit });
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
    let threshold = first_threshold(&cfg);
    let mut shown = items.row_icons.lock().unwrap_or_else(|e| e.into_inner());
    for ((item, row), cur) in items.rows.iter().zip(&text.rows).zip(shown.iter_mut()) {
        let _ = item.set_text(&row.text);
        let icon = row_icon(&row.icon, threshold);
        if icon != *cur && item.set_icon(icon_image(icon, &row.tool)).is_ok() {
            *cur = icon;
        }
    }
    drop(shown);
    let _ = items.today.set_text(&text.today);
    let [open_l, pause_l, quit_l] = action_labels(lang);
    let _ = items.open.set_text(open_l);
    let _ = items.pause.set_text(pause_l);
    let _ = items.quit.set_text(quit_l);
    if let Some(tray) = app.tray_by_id("main") {
        let face = face_for(&face_input(store, &st.cfg, limits, today.total, now));
        let _ = tray.set_icon(Some(face_image(face)));
        let _ = tray.set_tooltip(Some(tk_core::limits::render_line(limits, Some(today.total), lang)));
        let _ = tray.set_title(text.title.as_deref());
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
    fn tool_logos_decode_to_32x32_and_unknown_tool_has_none() {
        for (id, _) in TOOL_PNGS {
            let img = tool_logo(id).expect(id);
            assert_eq!((img.width(), img.height()), (32, 32), "{id}");
            assert_eq!(img.rgba()[3], 0, "{id}: góc trong suốt");
        }
        assert!(tool_logo("other").is_none());
    }

    #[test]
    fn row_menu_ids_carry_tool_id() {
        assert_eq!(tool_of_menu_id(&row_id("kiro", 3)), Some("kiro"));
        assert_eq!(tool_of_menu_id(&row_id("claude_code", 0)), Some("claude_code"));
        assert_eq!(tool_of_menu_id("open"), None);
        assert_eq!(tool_of_menu_id("today"), None);
    }

    #[test]
    fn row_icon_quantizes_ring_and_uses_first_threshold() {
        let mut cfg = Config::default();
        assert_eq!(first_threshold(&cfg), 70);
        cfg.quota_thresholds = vec![80, 95];
        assert_eq!(row_icon(&TrayIcon::Ring { pct: Some(75.2), stale: false }, first_threshold(&cfg)), RowIcon::Ring(75, RingTone::Normal));
        assert_eq!(row_icon(&TrayIcon::Ring { pct: Some(75.2), stale: false }, 70), RowIcon::Ring(75, RingTone::Alert));
        cfg.quota_thresholds.clear();
        assert_eq!(first_threshold(&cfg), 70);
        let img = ring_image(21, RingTone::Normal);
        assert_eq!((img.width(), img.height()), (32, 32));
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
        assert_eq!(t.rows.len(), srcs.iter().map(meter::tray_row_count).sum::<usize>());
        assert_eq!(t.rows[0].text, "Claude Code");
        assert!(t.rows[1].text.starts_with("5 giờ · 47% · reset "), "{}", t.rows[1].text);
        assert_eq!(t.rows[2].text, "7 ngày · chưa có dữ liệu");
        assert_eq!(t.rows[3].text, "Kiro");
        assert!(t.rows[4].text.starts_with("Tháng · 0% · 0/1000 credit · reset "), "{}", t.rows[4].text);
        assert_eq!(t.today, "Hôm nay: chưa có hoạt động");
        assert_eq!(t.title.as_deref(), Some("47%"));
        // /usage còn mới thì thắng hạn mức người dùng nhập.
        let mut q = std::collections::BTreeMap::new();
        q.insert("kiro".to_string(), ToolQuota { used: 270.69, limit: 1000.0, resets_on: chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), plan: "KIRO PRO".into(), fetched_at: now.timestamp() });
        let t = menu_text(&store, &cfg, &q, &srcs, &st, now, Lang::Vi);
        assert_eq!(t.rows[4].text, "Tháng · 27% · 271/1000 credit · reset 01/11");
        for r in &t.rows {
            println!("{:?} {}", r.icon, r.text);
        }
        println!("{}", t.today);
    }
}
