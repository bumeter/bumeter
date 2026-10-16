//! Cửa sổ chỉ tồn tại khi mở; đóng là hủy WebView để giữ RAM thấp (spec mục 3.1).
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open(app: &AppHandle) {
    show_or_build(app, "index.html");
}

/// Mở app ở phạm vi một công cụ (bấm dòng công cụ trong menu tray): cửa sổ đang có thì báo UI qua event `tray-scope`,
/// chưa có thì mở với `?tray_scope=` để UI đặt phạm vi khi khởi động.
pub fn open_scope(app: &AppHandle, tool: &str) {
    if !show_or_build(app, &format!("index.html?tray_scope={tool}")) {
        let _ = app.emit_to("main", "tray-scope", tool);
    }
}

/// Hiện cửa sổ "main" nếu có (trả false), không thì tạo mới từ `url` (trả true).
fn show_or_build(app: &AppHandle, url: &str) -> bool {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        return false;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(url.into()))
        .title("Bumeter")
        .inner_size(1100.0, 720.0)
        .build();
    true
}
