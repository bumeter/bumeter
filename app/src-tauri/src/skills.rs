//! Lệnh cho màn Skill (spec mục 8). UI chỉ được `apply` plan do chính lõi tạo ra.
use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::State;
use tk_adapters::claude_code::statusline::StatuslineSetup;
use tk_core::handlers::skill_stats;
use tk_core::store::Store;
use tk_skills::catalog::scan_personal;
use tk_skills::ops::{Manager, OpRequest, Plan};
use tk_skills::plugins::{scan_plugins, ClaudeCli};
use tk_skills::report::{build, Usage};

use crate::state::AppState;

#[derive(Default)]
pub struct PendingPlans(pub Mutex<HashMap<String, (String, Plan)>>);

fn kiro_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_default().join(".kiro")
}

/// Mỗi công cụ một Manager, cùng bố cục `skills/` + `skills-disabled/`; thùng rác/nhật ký tách riêng.
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
            entries.extend(tk_skills::catalog::scan_personal_for(&kiro_dir(), "kiro"));
            // `plugins: false`: trả skill cá nhân ngay; UI gọi lần hai để lấy plugin (CLI có thể chậm).
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
            let id = params["id"].as_str().ok_or("thiếu 'id'")?;
            let (plan_tool, plan) = pending.0.lock().map_err(|e| e.to_string())?.remove(id).ok_or("plan không tồn tại hoặc đã dùng")?;
            manager(&state, &plan_tool).apply(&plan).map(|e| serde_json::to_value(e).unwrap_or_default()).map_err(err)
        }
        "undo" => {
            let id = params["id"].as_str().ok_or("thiếu 'id'")?;
            m.undo(id).map(|_| json!(true)).map_err(err)
        }
        "search" => {
            // skills.sh chỉ là danh mục; cài thì clone thẳng từ GitHub (op `install_market`).
            let q = params["q"].as_str().unwrap_or_default();
            let limit = params["limit"].as_u64().unwrap_or(20) as u32;
            tk_skills::market::search(q, limit).map(|h| serde_json::to_value(h).unwrap_or_default()).map_err(err)
        }
        "log" => m.log().map(|l| serde_json::to_value(l).unwrap_or_default()).map_err(err),
        other => Err(format!("không có thao tác '{other}'")),
    }
}
