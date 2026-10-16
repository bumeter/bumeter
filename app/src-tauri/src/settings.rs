//! Lệnh cho màn Cài đặt: đọc/ghi config, bật/gỡ theo dõi giới hạn qua statusline (spec mục 7.1).
use serde_json::{json, Value};
use tauri::State;
use tk_adapters::claude_code::statusline::StatuslineSetup;
use tk_core::config::Config;

use crate::state::AppState;

/// Đường dẫn `bum`: `BUM_BIN` (hoặc `TK_BIN` cũ), hoặc file nằm cạnh app, hoặc tìm trong PATH.
pub fn bum_bin() -> String {
    if let Ok(p) = std::env::var("BUM_BIN").or_else(|_| std::env::var("TK_BIN")) {
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

pub fn setup(state: &AppState) -> StatuslineSetup {
    StatuslineSetup::new(StatuslineSetup::default_claude_dir(), state.paths.data_dir.join("backups"), bum_bin())
}

#[tauri::command(async)]
pub fn settings(state: State<'_, AppState>, action: String, params: Option<Value>) -> Result<Value, String> {
    let err = |e: anyhow::Error| e.to_string();
    let s = setup(&state);
    match action.as_str() {
        "get" => {
            let parse_errors = tk_core::store::Store::open_readonly(&state.paths.db()).ok().and_then(|st| st.parse_errors().ok()).unwrap_or(0);
            Ok(json!({
                "config": Config::load(&state.paths.config()),
                "statusline": s.status().map_err(err)?,
                "data_dir": state.paths.data_dir,
                "parse_errors": parse_errors,
                "tool_quotas": tk_core::meter::read_quotas(&state.paths.tool_quotas()),
            }))
        }
        "statusline_plan" => {
            let plan = s.plan().map_err(err)?;
            Ok(json!({ "description": plan.description, "noop": plan.is_noop() }))
        }
        "statusline_apply" => s.plan().and_then(|p| s.apply(&p)).map(|_| json!(true)).map_err(err),
        "statusline_undo" => s.undo().map(|_| json!(true)).map_err(err),
        "save_config" => {
            let cfg: Config = serde_json::from_value(params.unwrap_or_default()["config"].clone()).map_err(|e| e.to_string())?;
            cfg.save(&state.paths.config()).map_err(err)?;
            state.set_lang(cfg.language);
            Ok(json!(true))
        }
        other => Err(format!("không có thao tác '{other}'")),
    }
}
