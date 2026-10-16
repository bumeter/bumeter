//! The window exists only while open; closing destroys the WebView to keep RAM low (spec section 3.1).
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open(app: &AppHandle) {
    show_or_build(app, "index.html");
}

/// Open the app scoped to a tool (clicking a tool row in the tray menu): if a window already exists, notify the UI via the `tray-scope` event,
/// otherwise open with `?tray_scope=` so the UI sets the scope at startup.
pub fn open_scope(app: &AppHandle, tool: &str) {
    if !show_or_build(app, &format!("index.html?tray_scope={tool}")) {
        let _ = app.emit_to("main", "tray-scope", tool);
    }
}

/// Show the "main" window if it exists (returns false), otherwise create it from `url` (returns true).
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
