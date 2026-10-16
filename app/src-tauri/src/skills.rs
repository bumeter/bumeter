//! Commands for the Skill screen (spec section 8). The UI may only `apply` a plan created by the core itself.
use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::State;
use bum_adapters::claude_code::statusline::StatuslineSetup;
use bum_core::handlers::skill_stats;
use bum_core::store::Store;
use bum_skills::catalog::scan_personal;
use bum_skills::ops::{Manager, OpRequest, Plan};
use bum_skills::plugins::{scan_plugins, ClaudeCli};
use bum_skills::report::{build, Usage};

use crate::state::AppState;

#[derive(Default)]
pub struct PendingPlans(pub Mutex<HashMap<String, (String, Plan)>>);

fn kiro_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_default().join(".kiro")
}

/// One Manager per tool, with the same `skills/` + `skills-disabled/` layout; trash/log kept separate.
fn manager(state: &AppState, tool: &str) -> Manager {
    if tool == "kiro" {
        Manager::new(kiro_dir(), state.paths.data_dir.join("kiro"), ClaudeCli::system())
    } else {
        Manager::new(StatuslineSetup::default_claude_dir(), state.paths.data_dir.clone(), ClaudeCli::system())
    }
}

fn usage(state: &AppState) -> Option<Vec<Usage>> {
    let store = Store::open_readonly(&state.paths.db()).ok()?;
    Some(skill_stats(&store)
        .ok()?
        .into_iter()
        .map(|s| Usage { name: s.name, uses: s.uses, projects: s.projects, tokens: s.tokens, avg_tokens: s.avg_tokens, last_used: s.last_used })
        .collect())
}

#[tauri::command(async)]
pub fn skills(state: State<'_, AppState>, pending: State<'_, PendingPlans>, action: String, params: Option<Value>) -> Result<Value, String> {
    let params = params.unwrap_or(Value::Null);
    let tool = params["tool"].as_str().unwrap_or("claude_code").to_string();
    let m = manager(&state, &tool);
    let err = |e: anyhow::Error| format!("{e:#}");
    match action.as_str() {
        "list" => {
            let _ = m.purge_trash(chrono::Utc::now());
            let _ = manager(&state, "kiro").purge_trash(chrono::Utc::now());
            let mut entries = scan_personal(&StatuslineSetup::default_claude_dir());
            entries.extend(bum_skills::catalog::scan_personal_for(&kiro_dir(), "kiro"));
            // `plugins: false`: return personal skills immediately; the UI makes a second call to fetch plugins (the CLI can be slow).
            let mut warnings = Vec::new();
            if params["plugins"].as_bool().unwrap_or(true) {
                let (plugins, w) = scan_plugins(&ClaudeCli::system());
                entries.extend(plugins);
                warnings = w;
            }
            Ok(serde_json::to_value(build(entries, usage(&state), warnings, chrono::Utc::now())).unwrap_or_default())
        }
        "plan" => {
            let req: OpRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
            let plan = m.plan(&req).map_err(err)?;
            let out = json!({ "id": plan.id, "description": plan.description });
            pending.0.lock().map_err(|e| e.to_string())?.insert(plan.id.clone(), (tool.clone(), plan));
            Ok(out)
        }
        "apply" => {
            let id = params["id"].as_str().ok_or("missing 'id'")?;
            let (plan_tool, plan) = pending.0.lock().map_err(|e| e.to_string())?.remove(id).ok_or("plan does not exist or was already used")?;
            manager(&state, &plan_tool).apply(&plan).map(|e| serde_json::to_value(e).unwrap_or_default()).map_err(err)
        }
        "undo" => {
            let id = params["id"].as_str().ok_or("missing 'id'")?;
            m.undo(id).map(|_| json!(true)).map_err(err)
        }
        "search" => {
            // skills.sh is only a catalog; installing clones directly from GitHub (op `install_market`).
            let q = params["q"].as_str().unwrap_or_default();
            let limit = params["limit"].as_u64().unwrap_or(20) as u32;
            bum_skills::market::search(q, limit).map(|h| serde_json::to_value(h).unwrap_or_default()).map_err(err)
        }
        "log" => m.log().map(|l| serde_json::to_value(l).unwrap_or_default()).map_err(err),
        other => Err(format!("no such action '{other}'")),
    }
}
