//! Tray icon and menu (spec section 7.2).
use chrono::{DateTime, SecondsFormat, Utc};
use tauri::image::Image;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
#[cfg(not(target_os = "linux"))]
use tauri::menu::IconMenuItem;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use bum_core::handlers::TodaySummary;
use bum_core::i18n::Lang;
use bum_core::meter::{self, RingTone, ToolQuota, TrayIcon, TrayText};
use bum_core::source::SourceInfo;
use bum_core::config::Config;
use bum_core::face::{face_for, Face, FaceInput};
use bum_core::handlers::local_day_bounds;
use bum_core::limits::LimitState;
#[cfg(test)]
use bum_core::limits::Window;
use bum_core::store::Store;

pub struct TrayItems {
    sources: Vec<SourceInfo>,
    /// Per-tool rows (title + figures), `meter::tray_row_count` rows per tool.
    rows: Vec<RowItem>,
    /// Currently set icon of each row: only `set_icon` when it changes.
    row_icons: Mutex<Vec<RowIcon>>,
    today: MenuItem<Wry>,
    open: MenuItem<Wry>,
    pause: MenuItem<Wry>,
    update: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

const TRAY_PNGS: [(Face, &[u8]); 5] = [
    (Face::Happy, include_bytes!("../icons/tray/bum-happy.png")),
    (Face::Calm, include_bytes!("../icons/tray/bum-calm.png")),
    (Face::Worry, include_bytes!("../icons/tray/bum-worry.png")),
    (Face::Blaze, include_bytes!("../icons/tray/bum-blaze.png")),
    (Face::Sleep, include_bytes!("../icons/tray/bum-sleep.png")),
];

/// 32×32 logo by tool id (generated from `public/tool-icons/*.svg` in bumeter/ui).
#[cfg_attr(target_os = "linux", allow(dead_code))] // menu item icons are only used outside Linux
const TOOL_PNGS: [(&str, &[u8]); 3] = [
    ("claude_code", include_bytes!("../icons/tools/claude_code.png")),
    ("kiro", include_bytes!("../icons/tools/kiro.png")),
    ("opencode", include_bytes!("../icons/tools/opencode.png")),
];

/// Tool logo for the title row; unknown tool → no icon.
#[cfg_attr(target_os = "linux", allow(dead_code))] // menu item icons are only used outside Linux
pub fn tool_logo(tool: &str) -> Option<Image<'static>> {
    TOOL_PNGS.iter().find(|(id, _)| *id == tool).and_then(|(_, b)| Image::from_bytes(b).ok())
}

/// Icon of a menu row in a comparable form (ring quantized).
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
        TrayIcon::Ring { pct, stale, at_risk } => {
            let (p, tone) = meter::ring_key(pct, stale, threshold, at_risk);
            RowIcon::Ring(p, tone)
        }
    }
}

/// Ring image by (pct, color), cached to avoid redrawing.
#[cfg_attr(target_os = "linux", allow(dead_code))] // menu item icons are only used outside Linux
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

#[cfg_attr(target_os = "linux", allow(dead_code))] // menu item icons are only used outside Linux
fn icon_image(icon: RowIcon, tool: &str) -> Option<Image<'static>> {
    match icon {
        RowIcon::Empty => None,
        RowIcon::Logo => tool_logo(tool),
        RowIcon::Ring(p, tone) => Some(ring_image(p, tone)),
    }
}

/// Menu item id of row `i` for tool `tool`.
fn row_id(tool: &str, i: usize) -> String {
    format!("tool:{tool}:{i}")
}

/// Tool of a tool-row menu item (`tool:<id>:<i>`), None for other items.
pub fn tool_of_menu_id(id: &str) -> Option<&str> {
    let rest = id.strip_prefix("tool:")?;
    rest.rsplit_once(':').map(|(tool, _)| tool).filter(|t| !t.is_empty())
}

/// Alert threshold 1 (turns the ring red).
fn first_threshold(cfg: &Config) -> u8 {
    cfg.quota_thresholds.first().copied().unwrap_or(70)
}

/// The two user-set quota thresholds (default 70/90).
fn thresholds(cfg: &Config) -> [u8; 2] {
    [first_threshold(cfg), cfg.quota_thresholds.get(1).copied().unwrap_or(90)]
}

/// 32×32 tray icon of a Bum expression.
pub fn face_image(face: Face) -> Image<'static> {
    let bytes = TRAY_PNGS.iter().find(|(f, _)| *f == face).map(|(_, b)| *b).expect("all 5 expressions present");
    Image::from_bytes(bytes).expect("valid tray PNG")
}

fn minutes_since(ts: Option<String>, now: DateTime<Utc>) -> Option<i64> {
    let t = DateTime::parse_from_rfc3339(&ts?).ok()?;
    Some((now - t.with_timezone(&Utc)).num_minutes().max(0))
}

/// Gathers the input for choosing an expression from the DB + quota state (spec 3.4). A query error is treated as "no data".
pub fn face_input(store: &Store, cfg: &Config, limits: &LimitState, today_tokens: u64, now: DateTime<Utc>) -> FaceInput {
    let thresholds = match cfg.quota_thresholds.as_slice() {
        [a, b, ..] => (*a, *b),
        _ => (70, 90),
    };
    let offset = chrono::Local::now().offset().local_minus_utc();
    let (today_from, _) = local_day_bounds(now);
    let since = DateTime::parse_from_rfc3339(&today_from).map(|t| (t.with_timezone(&Utc) - chrono::Duration::days(30)).to_rfc3339_opts(SecondsFormat::Millis, true));
    // Average tokens/day over days with prompts, in the 30 days before today.
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
        quota_at_risk: limits.at_risk(),
    }
}

/// Per-tool rows + the "Today" row + title (bum_core::meter). A query error → quota only, no credit/activity.
pub fn menu_text(store: &Store, cfg: &Config, quotas: &std::collections::BTreeMap<String, ToolQuota>, sources: &[SourceInfo], limits: &LimitState, now: DateTime<Utc>, lang: Lang) -> TrayText {
    let snap = meter::snapshot(store, cfg, quotas, sources, now).unwrap_or_default();
    meter::render_tray(sources, limits, &snap, now.timestamp(), lang)
}

/// Labels of the fixed items: open Bumeter, pause, check for updates, quit.
pub fn action_labels(lang: Lang) -> [&'static str; 4] {
    [lang.pick("Mở Bumeter", "Open Bumeter"), lang.pick("Tạm dừng thông báo 1 giờ", "Pause notifications for 1 hour"), lang.pick("Kiểm tra cập nhật", "Check for updates"), lang.pick("Thoát", "Quit")]
}

/// Tools shown on the tray: in registry order, only tools that have a log directory on the machine.
pub fn sources(cfg: &Config) -> Vec<SourceInfo> {
    bum_adapters::installed(&bum_service::host::roots(cfg))
}

/// Linux (AppIndicator/dbusmenu): menu item icons are not exported to the GNOME bar and `IconMenuItem::set_text`
/// cannot change the label (muda builds a separate image box + label) → use plain text items, with the figure row indented under the tool name.
#[cfg(target_os = "linux")]
type RowItem = MenuItem<Wry>;
#[cfg(not(target_os = "linux"))]
type RowItem = IconMenuItem<Wry>;

#[cfg(target_os = "linux")]
fn new_row(app: &AppHandle, id: String, text: &str, _icon: RowIcon, _tool: &str) -> tauri::Result<RowItem> {
    MenuItem::with_id(app, id, text, true, None::<&str>)
}
#[cfg(not(target_os = "linux"))]
fn new_row(app: &AppHandle, id: String, text: &str, icon: RowIcon, tool: &str) -> tauri::Result<RowItem> {
    IconMenuItem::with_id(app, id, text, true, icon_image(icon, tool), None::<&str>)
}

/// 10-cell parallelogram % bar ▰▱: at least 1 cell when used, full only at 100%.
fn block_bar(pct: Option<f64>) -> String {
    const N: usize = 10;
    let p = pct.unwrap_or(0.0).max(0.0);
    let on = if p >= 100.0 { N } else if p > 0.0 { ((p / 100.0 * N as f64).round() as usize).clamp(1, N - 1) } else { 0 };
    format!("{}{}", "▰".repeat(on), "▱".repeat(N - on))
}

/// Color dot by usage level (menu text can't be colored, emoji can): below threshold 1 green, from threshold 1 or at risk
/// (forecast limit before the reset) orange, from threshold 2 red; stale or missing data is white.
fn level_dot(pct: Option<f64>, stale: bool, at_risk: bool, [t1, t2]: [u8; 2]) -> &'static str {
    match pct {
        Some(_) if stale => "⚪",
        None => "⚪",
        Some(p) if p >= f64::from(t2) => "🔴",
        Some(p) if p >= f64::from(t1) || at_risk => "🟠",
        Some(_) => "🟢",
    }
}

/// Display label of a row. Linux doesn't show menu item icons: every row is flush left (no indent),
/// with the % bar + color dot at the start of the line so the bars line up (the menu font is proportional).
fn row_label(text: &str, icon: &TrayIcon, thresholds: [u8; 2]) -> String {
    if !cfg!(target_os = "linux") {
        return text.to_string();
    }
    match icon {
        TrayIcon::Logo | TrayIcon::None => text.to_string(),
        TrayIcon::Ring { pct, stale, at_risk } => format!("{} {}  {text}", block_bar(*pct), level_dot(*pct, *stale, *at_risk, thresholds)),
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let st = app.state::<crate::state::AppState>();
    let sources = sources(&st.cfg);
    // The number of rows per tool is fixed by the registry; contents are reset in `refresh`. Kept enabled: GNOME grays out the icon of a disabled item,
    // and clicking it opens the app scoped to that tool.
    let mut rows = Vec::new();
    let mut row_icons = Vec::new();
    let mut seps = Vec::new();
    for s in &sources {
        for k in 0..meter::tray_row_count(s) {
            let (text, icon) = if k == 0 { (s.label.as_str(), RowIcon::Logo) } else { ("…", RowIcon::Empty) };
            rows.push(new_row(app, row_id(&s.id, rows.len()), text, icon, &s.id)?);
            row_icons.push(icon);
        }
        seps.push(rows.len());
    }
    let today = MenuItem::with_id(app, "today", "…", false, None::<&str>)?;
    let [open_l, pause_l, update_l, quit_l] = action_labels(st.lang());
    let open = MenuItem::with_id(app, "open", open_l, true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", pause_l, true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", update_l, true, None::<&str>)?;
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
    menu.append_items(&[&open, &pause, &update, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(face_image(Face::Calm))
        .tooltip("Bumeter")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| match ev.id.as_ref() {
            "open" => crate::window::open(app),
            "pause" => crate::state::pause_for(app, 3600),
            "update" => crate::update::check_now(app),
            "quit" => app.exit(0),
            id => {
                if let Some(tool) = tool_of_menu_id(id) {
                    crate::window::open_scope(app, tool);
                }
            }
        })
        .build(app)?;
    app.manage(TrayItems { sources, rows, row_icons: Mutex::new(row_icons), today, open, pause, update, quit });
    Ok(())
}

/// Some fresh % row (any tool) is forecast to hit its limit before the reset.
fn any_row_at_risk(rows: &[meter::TrayRow]) -> bool {
    rows.iter().any(|r| matches!(r.icon, TrayIcon::Ring { at_risk: true, stale: false, .. }))
}

pub fn refresh(app: &AppHandle, store: &Store, limits: &LimitState, today: &TodaySummary) {
    let items = app.state::<TrayItems>();
    let st = app.state::<crate::state::AppState>();
    let lang = st.lang();
    let now = Utc::now();
    // Re-read config/limits every time: if the user changes them in Settings, the tray follows immediately.
    let cfg = Config::load(&st.paths.config());
    let quotas = meter::read_quotas(&st.paths.tool_quotas());
    let text = menu_text(store, &cfg, &quotas, &items.sources, limits, now, lang);
    let threshold = first_threshold(&cfg);
    let mut shown = items.row_icons.lock().unwrap_or_else(|e| e.into_inner());
    for ((item, row), cur) in items.rows.iter().zip(&text.rows).zip(shown.iter_mut()) {
        let _ = item.set_text(row_label(&row.text, &row.icon, thresholds(&cfg)));
        let icon = row_icon(&row.icon, threshold);
        #[cfg(not(target_os = "linux"))]
        if icon != *cur && item.set_icon(icon_image(icon, &row.tool)).is_ok() {
            *cur = icon;
        }
        #[cfg(target_os = "linux")]
        {
            *cur = icon;
        }
    }
    drop(shown);
    let _ = items.today.set_text(&text.today);
    let [open_l, pause_l, update_l, quit_l] = action_labels(lang);
    let _ = items.open.set_text(open_l);
    let _ = items.pause.set_text(pause_l);
    let _ = items.update.set_text(update_l);
    let _ = items.quit.set_text(quit_l);
    if let Some(tray) = app.tray_by_id("main") {
        let mut input = face_input(store, &st.cfg, limits, today.total, now);
        input.quota_at_risk |= any_row_at_risk(&text.rows); // credit tools too (Kiro), not only the Claude windows
        let face = face_for(&input);
        let _ = tray.set_icon(Some(face_image(face)));
        let _ = tray.set_tooltip(Some(bum_core::limits::render_line(limits, Some(today.total), lang)));
        let _ = tray.set_title(text.title.as_deref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_row_label_has_bar_first_so_bars_line_up() {
        if !cfg!(target_os = "linux") {
            return;
        }
        let ring = |p: f64| TrayIcon::Ring { pct: Some(p), stale: false, at_risk: false };
        let t = [70, 90];
        assert_eq!(row_label("5 hours · 42% · reset 13:10", &ring(42.0), t), "▰▰▰▰▱▱▱▱▱▱ 🟢  5 hours · 42% · reset 13:10");
        // Color by threshold: < 70 green, 70–89 orange, ≥ 90 red.
        assert_eq!(row_label("x", &ring(69.9), t), "▰▰▰▰▰▰▰▱▱▱ 🟢  x");
        assert_eq!(row_label("x", &ring(70.0), t), "▰▰▰▰▰▰▰▱▱▱ 🟠  x");
        assert_eq!(row_label("x", &ring(89.0), t), "▰▰▰▰▰▰▰▰▰▱ 🟠  x");
        assert_eq!(row_label("x", &ring(90.0), t), "▰▰▰▰▰▰▰▰▰▱ 🔴  x");
        assert_eq!(row_label("x", &ring(130.0), t), "▰▰▰▰▰▰▰▰▰▰ 🔴  x");
        // Different user-set thresholds.
        assert_eq!(row_label("x", &ring(60.0), [50, 80]), "▰▰▰▰▰▰▱▱▱▱ 🟠  x");
        // When used, at least 1 cell; full only at 100%.
        assert_eq!(row_label("x", &ring(0.4), t), "▰▱▱▱▱▱▱▱▱▱ 🟢  x");
        assert_eq!(row_label("x", &ring(99.6), t), "▰▰▰▰▰▰▰▰▰▱ 🔴  x");
        // Stale or missing data: white dot.
        assert_eq!(row_label("x", &TrayIcon::Ring { pct: Some(95.0), stale: true, at_risk: false }, t), "▰▰▰▰▰▰▰▰▰▱ ⚪  x");
        assert_eq!(row_label("x", &TrayIcon::Ring { pct: None, stale: false, at_risk: false }, t), "▱▱▱▱▱▱▱▱▱▱ ⚪  x");
        // Rows without a % and tool titles: kept as-is.
        assert_eq!(row_label("Kiro · today", &TrayIcon::None, t), "Kiro · today");
        assert_eq!(row_label("Kiro", &TrayIcon::Logo, t), "Kiro");
    }

    #[test]
    fn any_tool_at_risk_counts_for_the_face_unless_stale() {
        let row = |tool: &str, stale, at_risk| meter::TrayRow { tool: tool.into(), text: String::new(), icon: TrayIcon::Ring { pct: Some(40.0), stale, at_risk } };
        assert!(!any_row_at_risk(&[row("claude_code", false, false), row("kiro", false, false)]));
        assert!(any_row_at_risk(&[row("claude_code", false, false), row("kiro", false, true)]), "Kiro credits at risk");
        assert!(!any_row_at_risk(&[row("kiro", true, true)]), "stale data does not worry Bum");
    }

    #[test]
    fn at_risk_row_is_orange_below_the_first_threshold() {
        if !cfg!(target_os = "linux") {
            return;
        }
        let t = [70, 90];
        assert_eq!(row_label("x", &TrayIcon::Ring { pct: Some(40.0), stale: false, at_risk: true }, t), "▰▰▰▰▱▱▱▱▱▱ 🟠  x");
        assert_eq!(row_label("x", &TrayIcon::Ring { pct: Some(95.0), stale: false, at_risk: true }, t), "▰▰▰▰▰▰▰▰▰▱ 🔴  x");
        assert_eq!(row_label("x", &TrayIcon::Ring { pct: Some(40.0), stale: true, at_risk: true }, t), "▰▰▰▰▱▱▱▱▱▱ ⚪  x");
    }

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
            assert_eq!(img.rgba()[3], 0, "{id}: transparent corner");
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
        assert_eq!(row_icon(&TrayIcon::Ring { pct: Some(75.2), stale: false, at_risk: false }, first_threshold(&cfg)), RowIcon::Ring(75, RingTone::Normal));
        assert_eq!(row_icon(&TrayIcon::Ring { pct: Some(75.2), stale: false, at_risk: false }, 70), RowIcon::Ring(75, RingTone::Alert));
        cfg.quota_thresholds.clear();
        assert_eq!(first_threshold(&cfg), 70);
        let img = ring_image(21, RingTone::Normal);
        assert_eq!((img.width(), img.height()), (32, 32));
    }

    #[test]
    fn menu_lines_follow_registry_and_match_item_count() {
        // Point both tools at existing empty dirs so the test does not depend on real logs in $HOME.
        let base = std::env::temp_dir().join(format!("bumeter-tray-test-{}", std::process::id()));
        let (claude, kiro) = (base.join("claude"), base.join("kiro"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::create_dir_all(&kiro).unwrap();
        let mut tool_limits = std::collections::BTreeMap::new();
        tool_limits.insert("kiro".into(), bum_core::config::ToolLimit { monthly_credits: Some(1000.0), reset_day: 1 });
        let cfg = Config { claude_projects_dir: Some(claude), opencode_db: Some(std::path::PathBuf::from("/nonexistent-opencode/opencode.db")), kiro_sessions_dir: Some(kiro), tool_limits, ..Default::default() };
        let srcs = sources(&cfg);
        assert_eq!(srcs.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["claude_code", "kiro"]);
        let store = Store::open_in_memory().unwrap();
        let st = bum_core::limits::state(
            Some(bum_core::limits::LimitsFile { updated_at: 0, five_hour: Some(Window { used_percentage: 47.0, resets_at: 4_102_444_800 }), seven_day: None }),
            None,
            0,
        );
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
        // A fresh /usage wins over user-entered limits.
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
