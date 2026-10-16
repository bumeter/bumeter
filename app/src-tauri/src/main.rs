// Không mở cửa sổ console trên Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod runtime;
mod settings;
mod skills;
mod state;
mod tray;
mod window;

use tauri::{Manager, RunEvent};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window::open(app)))
        .plugin(tauri_plugin_notification::init())
        .manage(state::AppState::load())
        .manage(skills::PendingPlans::default())
        .setup(|app| {
            // Máy đã bật theo dõi giới hạn bằng `tk`: trỏ hook sang `bum` (best effort).
            if let Some(state) = app.try_state::<state::AppState>() {
                match settings::setup(&state).migrate_legacy_hook() {
                    Ok(true) => eprintln!("bumeter: đã chuyển hook statusline từ tk sang bum"),
                    Ok(false) => {}
                    Err(e) => eprintln!("bumeter: không chuyển được hook statusline: {e}"),
                }
            }
            tray::create(app.handle())?;
            runtime::start(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::call, settings::settings, skills::skills])
        .build(tauri::generate_context!())
        .expect("không khởi tạo được app")
        .run(|app, event| {
            // Đóng cửa sổ không thoát app; chỉ menu "Thoát" (có exit code) mới thoát.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                } else if let Some(slot) = app.try_state::<runtime::HostSlot>() {
                    if let Some(h) = slot.0.lock().ok().and_then(|mut s| s.take()) {
                        h.shutdown();
                        let _ = h.wait_timeout(std::time::Duration::from_secs(3));
                    }
                }
            }
        });
}
