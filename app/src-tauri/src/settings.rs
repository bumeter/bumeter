//! Commands for the Settings screen: read/write config, enable/remove limit tracking via statusline (spec section 7.1).
use serde_json::{json, Value};
use tauri::State;
use bum_adapters::claude_code::statusline::{Mode, StatuslineSetup, SHOW_SUPPORTED};
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

/// `params.mode`: `"show"` puts Bumeter's part on the status line, `"record"` (the default) only records the numbers.
fn mode(params: &Option<Value>) -> Mode {
    params.as_ref().and_then(|p| serde_json::from_value(p["mode"].clone()).ok()).unwrap_or(Mode::Record)
}

pub fn setup(state: &AppState) -> StatuslineSetup {
    StatuslineSetup::new(StatuslineSetup::default_claude_dir(), state.paths.data_dir.join("backups"), bum_bin())
}

#[tauri::command(async)]
pub fn settings(app: tauri::AppHandle, state: State<'_, AppState>, action: String, params: Option<Value>) -> Result<Value, String> {
    let err = |e: anyhow::Error| e.to_string();
    let s = setup(&state);
    match action.as_str() {
        "get" => {
            let parse_errors = bum_core::store::Store::open_readonly(&state.paths.db()).ok().and_then(|st| st.parse_errors().ok()).unwrap_or(0);
            Ok(json!({
                "config": Config::load(&state.paths.config()),
                "statusline": s.status().map_err(err)?,
                "statusline_show": SHOW_SUPPORTED,
                "data_dir": state.paths.data_dir,
                "parse_errors": parse_errors,
                "tool_quotas": bum_core::meter::read_quotas(&state.paths.tool_quotas()),
                "version": app.package_info().version.to_string(),
            }))
        }
        "statusline_plan" => {
            let plan = s.plan_for(mode(&params)).map_err(err)?;
            Ok(json!({ "description": plan.description, "noop": plan.is_noop() }))
        }
        "statusline_apply" => s.plan_for(mode(&params)).and_then(|p| s.apply(&p)).map(|_| json!(true)).map_err(err),
        "statusline_undo" => s.undo().map(|_| json!(true)).map_err(err),
        "save_config" => {
            let cfg: Config = serde_json::from_value(params.unwrap_or_default()["config"].clone()).map_err(|e| e.to_string())?;
            cfg.save(&state.paths.config()).map_err(err)?;
            state.set_lang(cfg.language);
            Ok(json!(true))
        }
        other => Err(format!("no such action '{other}'")),
    }
}
