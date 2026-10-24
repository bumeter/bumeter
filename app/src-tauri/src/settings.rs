//! Commands for the Settings screen: read/write config, enable/remove limit tracking via statusline (spec section 7.1).
use serde_json::Value;
use tauri::State;
use bum_core::config::Config;

use crate::state::AppState;

/// Path to `bum`: `BUM_BIN`, or a file next to the app, or searched in PATH.
pub fn bum_bin() -> String {
    if let Ok(p) = std::env::var("BUM_BIN") {
        return p;
    }
    let name = if cfg!(windows) { "bum.exe" } else { "bum" };
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(name)))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string())
}

#[tauri::command(async)]
pub fn settings(state: State<'_, AppState>, ops: State<'_, bum_service::ops::Ops>, action: String, params: Option<Value>) -> Result<Value, String> {
    // Shared with `bum rpc` (bum_service::ops); the app also switches its own language on save.
    let out = ops.settings(&action, &params.unwrap_or(Value::Null))?;
    if action == "save_config" {
        if let Ok(cfg) = serde_json::from_value::<Config>(out.clone()) {
            state.set_lang(cfg.language);
        }
        return Ok(serde_json::json!(true));
    }
    Ok(out)
}
