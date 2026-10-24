// Don't open a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod runtime;
mod settings;
mod skills;
mod state;
mod tray;
mod update;
mod window;

use tauri::{Manager, RunEvent};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| window::open(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state::AppState::load())
        .manage(bum_service::ops::Ops::new(bum_core::paths::Paths::resolve(), settings::bum_bin(), env!("CARGO_PKG_VERSION")))
        .setup(|app| {
            tray::create(app.handle())?;
            runtime::start(app.handle());
            update::check_on_startup(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::call, settings::settings, skills::skills, update::check_update, update::install_update])
        .build(tauri::generate_context!())
        .expect("failed to initialize app")
        .run(|app, event| {
            // Closing the window doesn't quit the app; only the "Quit" menu item (with an exit code) quits.
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
