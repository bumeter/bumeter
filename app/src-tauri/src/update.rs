//! In-app updates: the Tauri updater plugin reads the `latest.json` that CI attaches to each
//! GitHub Release (spec: distribution stays GitHub-only, no server of our own).
//!
//! The check runs in Rust rather than in the frontend so the tray can notify and the window
//! keeps its minimal `core:default` capability (the JS commands would need `updater:default`).
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::state::AppState;

/// What Settings shows after a "Check for updates".
#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    /// A newer version is published.
    pub available: bool,
    /// Version offered by `latest.json` (only when `available`).
    pub version: Option<String>,
    /// Version currently running.
    pub current: String,
    /// Release notes carried by `latest.json`, when present.
    pub notes: Option<String>,
}

/// Ask the configured endpoint for `latest.json`. `Err` on network or malformed-JSON problems;
/// `Ok(None)` when this build is already the newest.
pub async fn check(app: &AppHandle) -> Result<Option<Update>, String> {
    app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())
}

/// Settings → "Check for updates".
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let found = check(&app).await?;
    Ok(UpdateInfo {
        available: found.is_some(),
        version: found.as_ref().map(|u| u.version.clone()),
        current,
        notes: found.and_then(|u| u.body),
    })
}

/// Settings → "Install & restart": download, verify the signature, install, then relaunch.
/// On Windows the installer exits the process first, so `restart` is only reached on macOS/Linux.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = check(&app).await?.ok_or("no update available")?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart();
}

/// Tray notification in the current UI language.
fn notify(app: &AppHandle, vi: String, en: String) {
    let lang = app.state::<AppState>().lang();
    let title = lang.pick("Bumeter · Cập nhật", "Bumeter · Update");
    let _ = app.notification().builder().title(title).body(lang.pick(&vi, &en)).show();
}

/// Tray menu → "Check for updates": check in the background and report the result.
pub fn check_now(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || match tauri::async_runtime::block_on(check(&app)) {
        Ok(Some(u)) => notify(
            &app,
            format!("Bản {} đã sẵn sàng — mở Cài đặt để cài.", u.version),
            format!("Version {} is ready — open Settings to install.", u.version),
        ),
        Ok(None) => {
            let v = app.package_info().version.to_string();
            notify(&app, format!("Bạn đang dùng bản mới nhất ({v})."), format!("You're on the latest version ({v})."));
        }
        Err(e) => notify(&app, format!("Không kiểm tra được cập nhật: {e}"), format!("Could not check for updates: {e}")),
    });
}

/// Quietly check a little after launch and notify only when a newer build is published.
pub fn check_on_startup(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        if let Ok(Some(u)) = tauri::async_runtime::block_on(check(&app)) {
            if app.state::<AppState>().paused() {
                return; // "Pause notifications for 1 hour" also silences the update nudge
            }
            notify(
                &app,
                format!("Bản {} đã sẵn sàng — mở Cài đặt để cài.", u.version),
                format!("Version {} is ready — open Settings to install.", u.version),
            );
        }
    });
}
