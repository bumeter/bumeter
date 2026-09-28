//! Cửa sổ chỉ tồn tại khi mở; đóng là hủy WebView để giữ RAM thấp (spec mục 3.1).
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("thongke")
        .inner_size(1100.0, 720.0)
        .build();
}
